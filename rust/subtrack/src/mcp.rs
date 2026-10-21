//! MCP (Model Context Protocol) stdio server — port of
//! `packages/subtrack/src/mcp/*` (protocol, transport, tools, security,
//! handlers). Implements JSON-RPC 2.0 over a newline-delimited (NDJSON) or
//! `Content-Length:`-framed byte stream read from stdin; one single-line
//! response per request is written to stdout.

use std::collections::HashMap;

use crate::db::{self, SqlValue, Subscription};
use crate::json::{number, quote_string as js_str};
use crate::logger;
use crate::parser::ParsedArgs;

// ── protocol constants ───────────────────────────────────

const SERVER_NAME: &str = "subtrack-mcp";
const SERVER_VERSION: &str = "1.0.0";
const DEFAULT_PROTOCOL_VERSION: &str = "2024-11-05";

const ERR_PARSE: i64 = -32700;
const ERR_INVALID_REQUEST: i64 = -32600;
const ERR_METHOD_NOT_FOUND: i64 = -32601;
const ERR_INVALID_PARAMS: i64 = -32602;

// ── security limits ──────────────────────────────────────

const MAX_REQUEST_SIZE: usize = 1024 * 100;
/// Default max string length; mirrors `MAX_STRING_LENGTH` in security.ts
/// (every per-tool schema passes an explicit `maxLength`, so it is unused).
#[allow(dead_code)]
const MAX_STRING_LENGTH: usize = 500;
const MAX_TAG_COUNT: usize = 20;
const MAX_NEW_TAGS: usize = 10;
const MAX_BULK_AFFECTED: usize = 1_000;

// ── JSON helpers (JS JSON.stringify semantics) ───────────

fn opt_str(v: &Option<String>) -> String {
    v.as_deref()
        .map(js_str)
        .unwrap_or_else(|| "null".to_string())
}

fn opt_num(v: Option<f64>) -> String {
    v.map(number).unwrap_or_else(|| "null".to_string())
}

fn opt_i64(v: Option<i64>) -> String {
    v.map(|x| x.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn arr_strings(v: &[String]) -> String {
    let items: Vec<String> = v.iter().map(|s| js_str(s)).collect();
    format!("[{}]", items.join(","))
}

/// Compact `JSON.stringify(subscription)` (key order = SUB_COLUMNS + tags).
fn sub_compact(s: &Subscription) -> String {
    let f = [
        format!("\"id\":{}", s.id),
        format!("\"name\":{}", js_str(&s.name)),
        format!("\"price\":{}", number(s.price)),
        format!("\"currency\":{}", js_str(&s.currency)),
        format!("\"cycle\":{}", js_str(&s.cycle)),
        format!("\"status\":{}", js_str(&s.status)),
        format!("\"billingDay\":{}", opt_i64(s.billing_day)),
        format!("\"createdAt\":{}", js_str(&s.created_at)),
        format!("\"notes\":{}", opt_str(&s.notes)),
        format!("\"paymentMethod\":{}", opt_str(&s.payment_method)),
        format!("\"contractStart\":{}", opt_str(&s.contract_start)),
        format!("\"contractEnd\":{}", opt_str(&s.contract_end)),
        format!("\"autoRenewal\":{}", s.auto_renewal),
        format!("\"vendorName\":{}", opt_str(&s.vendor_name)),
        format!("\"vendorUrl\":{}", opt_str(&s.vendor_url)),
        format!("\"planTier\":{}", opt_str(&s.plan_tier)),
        format!("\"discountAmount\":{}", opt_num(s.discount_amount)),
        format!("\"discountType\":{}", opt_str(&s.discount_type)),
        format!("\"tags\":{}", arr_strings(&s.tags)),
    ];
    format!("{{{}}}", f.join(","))
}

fn subs_compact(v: &[Subscription]) -> String {
    let items: Vec<String> = v.iter().map(sub_compact).collect();
    format!("[{}]", items.join(","))
}

fn trial_compact(t: &db::Trial) -> String {
    format!(
        "{{\"id\":{},\"name\":{},\"expiresAt\":{},\"price\":{},\"currency\":{},\"cycle\":{},\"notes\":{},\"createdAt\":{}}}",
        t.id,
        js_str(&t.name),
        js_str(&t.expires_at),
        opt_num(t.price),
        opt_str(&t.currency),
        opt_str(&t.cycle),
        opt_str(&t.notes),
        js_str(&t.created_at),
    )
}

fn price_change_compact(c: &db::PriceChange) -> String {
    format!(
        "{{\"id\":{},\"subscriptionId\":{},\"subscriptionName\":{},\"oldPrice\":{},\"newPrice\":{},\"oldCurrency\":{},\"newCurrency\":{},\"changedAt\":{}}}",
        c.id,
        c.subscription_id,
        js_str(&c.name),
        opt_num(c.old_price),
        number(c.new_price),
        opt_str(&c.old_currency),
        opt_str(&c.new_currency),
        js_str(&c.changed_at),
    )
}

fn llm_usage_compact(u: &db::LlmUsage) -> String {
    format!(
        "{{\"id\":{},\"provider\":{},\"model\":{},\"input_tokens\":{},\"output_tokens\":{},\"cost\":{},\"date\":{},\"description\":{}}}",
        u.id,
        js_str(&u.provider),
        js_str(&u.model),
        u.input_tokens,
        u.output_tokens,
        number(u.cost),
        js_str(&u.date),
        opt_str(&u.description),
    )
}

// ── tool registry (port of tools.ts) ─────────────────────

struct ToolDef {
    name: &'static str,
    description: &'static str,
    /// (property name, JSON type, description)
    props: &'static [(&'static str, &'static str, &'static str)],
    required: &'static [&'static str],
}

const TOOLS: &[ToolDef] = &[
    ToolDef {
        name: "list_subscriptions",
        description: "List all subscriptions",
        props: &[
            (
                "sort",
                "string",
                "Sort field: name, price, currency, cycle, status",
            ),
            ("desc", "boolean", "Sort descending"),
            ("limit", "number", "Max entries to return (default: all)"),
            ("offset", "number", "Skip the first N entries (for paging)"),
        ],
        required: &[],
    },
    ToolDef {
        name: "get_subscription",
        description: "Get subscription by ID",
        props: &[("id", "number", "Subscription ID")],
        required: &["id"],
    },
    ToolDef {
        name: "search_subscriptions",
        description: "Search subscriptions by name, notes, or tags",
        props: &[
            ("query", "string", "Search query"),
            ("names", "boolean", "Search in names"),
            ("notes", "boolean", "Search in notes"),
            ("tags", "boolean", "Search in tags"),
        ],
        required: &["query"],
    },
    ToolDef {
        name: "add_subscription",
        description: "Add a new subscription",
        props: &[
            ("name", "string", "Subscription name"),
            (
                "price",
                "number",
                "Price in major currency units (e.g. 14.99 USD)",
            ),
            ("currency", "string", "Currency code (e.g. USD, JPY)"),
            (
                "cycle",
                "string",
                "Billing cycle: weekly, bi-weekly, monthly, quarterly, semi-annual, yearly",
            ),
            ("tags", "string", "Comma-separated tags"),
            ("billingDay", "number", "Billing day of month (1-31)"),
            ("status", "string", "Status: active, paused, cancelled"),
            ("paymentMethod", "string", "Payment method"),
            ("notes", "string", "Notes"),
        ],
        required: &["name", "price", "currency", "cycle"],
    },
    ToolDef {
        name: "delete_subscription",
        description: "Delete subscription by ID",
        props: &[("id", "number", "Subscription ID")],
        required: &["id"],
    },
    ToolDef {
        name: "get_summary",
        description: "Get subscription summary statistics",
        props: &[],
        required: &[],
    },
    ToolDef {
        name: "get_upcoming",
        description: "Get upcoming bills within a number of days",
        props: &[("days", "number", "Number of days (default: 7)")],
        required: &[],
    },
    ToolDef {
        name: "get_calendar",
        description: "Get calendar entries for a month",
        props: &[
            ("month", "number", "Month (1-12)"),
            ("year", "number", "Year"),
        ],
        required: &[],
    },
    ToolDef {
        name: "export_data",
        description: "Export subscriptions in a format",
        props: &[("format", "string", "Export format: csv, json, md")],
        required: &["format"],
    },
    ToolDef {
        name: "edit_subscription",
        description: "Edit an existing subscription",
        props: &[
            ("id", "number", "Subscription ID to edit"),
            ("name", "string", "New name"),
            (
                "price",
                "number",
                "New price in major currency units (e.g. 14.99 USD)",
            ),
            ("currency", "string", "New currency code"),
            ("cycle", "string", "New billing cycle"),
            ("status", "string", "New status: active, paused, cancelled"),
            ("billingDay", "number", "New billing day of month (1-31)"),
            ("tags", "string", "Comma-separated tags (replaces all)"),
            ("paymentMethod", "string", "New payment method"),
            ("notes", "string", "New notes"),
        ],
        required: &["id"],
    },
    ToolDef {
        name: "get_history",
        description: "Get price change history for subscriptions",
        props: &[
            ("id", "number", "Filter by subscription ID"),
            ("days", "number", "Recent days to include (default: all)"),
        ],
        required: &[],
    },
    ToolDef {
        name: "get_analytics",
        description: "Get subscription analytics and statistics",
        props: &[],
        required: &[],
    },
    ToolDef {
        name: "get_forecast",
        description: "Get spending forecast for upcoming months",
        props: &[
            (
                "months",
                "number",
                "Number of months to forecast (default: 12)",
            ),
            ("currency", "string", "Convert all to target currency"),
            (
                "cancel",
                "string",
                "Comma-separated subscription names to exclude from forecast",
            ),
        ],
        required: &[],
    },
    ToolDef {
        name: "compare",
        description: "Compare subscription spending between current and previous period",
        props: &[
            (
                "period",
                "string",
                "Period to compare: monthly (default), yearly, quarterly",
            ),
            ("currency", "string", "Convert all to target currency"),
        ],
        required: &[],
    },
    ToolDef {
        name: "bulk_operations",
        description: "Perform bulk operations on subscriptions matching filters",
        props: &[
            (
                "action",
                "string",
                "Action: status, delete, tag_add, tag_remove",
            ),
            (
                "status",
                "string",
                "Target status for 'status' action: active, paused, cancelled",
            ),
            (
                "tag_name",
                "string",
                "Tag name for tag_add / tag_remove actions",
            ),
            (
                "filter_tag",
                "string",
                "Only affect subscriptions with this tag",
            ),
            (
                "filter_status",
                "string",
                "Only affect subscriptions with this status",
            ),
            (
                "filter_name",
                "string",
                "Only affect subscriptions whose name contains this",
            ),
            (
                "confirm",
                "boolean",
                "Must be true to perform a destructive bulk action",
            ),
        ],
        required: &["action"],
    },
    ToolDef {
        name: "get_trials",
        description: "Get trial periods",
        props: &[(
            "expiring_soon",
            "number",
            "Filter trials expiring within N days",
        )],
        required: &[],
    },
    ToolDef {
        name: "list_tags",
        description: "List all tags with their subscription counts",
        props: &[],
        required: &[],
    },
    ToolDef {
        name: "get_tag_subscriptions",
        description: "Get subscriptions matching one or more tags (AND logic)",
        props: &[(
            "tag",
            "string",
            "Comma-separated tag names (all must match)",
        )],
        required: &["tag"],
    },
    ToolDef {
        name: "get_usage_total",
        description: "Get aggregated LLM API usage for a date range (cost, tokens, provider/model breakdown)",
        props: &[
            (
                "from",
                "string",
                "Start date YYYY-MM-DD (default: current month)",
            ),
            (
                "to",
                "string",
                "End date YYYY-MM-DD (default: current month)",
            ),
        ],
        required: &[],
    },
    ToolDef {
        name: "list_usage",
        description: "List LLM API usage entries",
        props: &[
            ("provider", "string", "Filter by provider"),
            ("from", "string", "Start date YYYY-MM-DD"),
            ("to", "string", "End date YYYY-MM-DD"),
            ("limit", "number", "Max entries to return (default: 100)"),
        ],
        required: &[],
    },
];

fn tools_json() -> String {
    let mut out = String::from("[");
    for (i, t) in TOOLS.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let mut props = String::from("{");
        for (j, (pk, pt, pd)) in t.props.iter().enumerate() {
            if j > 0 {
                props.push(',');
            }
            props.push_str(&format!(
                "{}:{{\"type\":{},\"description\":{}}}",
                js_str(pk),
                js_str(pt),
                js_str(pd)
            ));
        }
        props.push('}');
        out.push_str(&format!(
            "{{\"name\":{},\"description\":{},\"inputSchema\":{{\"type\":\"object\",\"properties\":{}",
            js_str(t.name),
            js_str(t.description),
            props
        ));
        if !t.required.is_empty() {
            let req: Vec<String> = t.required.iter().map(|r| js_str(r)).collect();
            out.push_str(&format!(",\"required\":[{}]", req.join(",")));
        }
        out.push_str("}}");
    }
    out.push(']');
    out
}

// ── security: validation schemas (port of security.ts) ───

#[derive(Clone, Copy)]
enum Rule {
    Str {
        max_len: usize,
        enum_vals: &'static [&'static str],
    },
    Num {
        min: f64,
        max: f64,
        integer: bool,
    },
    Bool,
}

impl Rule {
    const fn s(max_len: usize) -> Self {
        Rule::Str {
            max_len,
            enum_vals: &[],
        }
    }
    const fn se(max_len: usize, enum_vals: &'static [&'static str]) -> Self {
        Rule::Str { max_len, enum_vals }
    }
    const fn n(min: f64, max: f64, integer: bool) -> Self {
        Rule::Num { min, max, integer }
    }
    const fn b() -> Self {
        Rule::Bool
    }
}

const CYCLE_ENUM: &[&str] = &[
    "weekly",
    "bi-weekly",
    "monthly",
    "quarterly",
    "semi-annual",
    "yearly",
];
const STATUS_ENUM: &[&str] = &["active", "paused", "cancelled", "archived"];
const BULK_ACTION_ENUM: &[&str] = &["status", "delete", "tag_add", "tag_remove"];

fn schema_for(name: &str) -> Vec<(&'static str, Rule)> {
    match name {
        "add_subscription" => vec![
            ("name", Rule::s(100)),
            ("price", Rule::n(0.0, 99_999_999.0, false)),
            ("currency", Rule::s(3)),
            ("cycle", Rule::s(20)),
            ("tags", Rule::s(500)),
            ("billingDay", Rule::n(1.0, 31.0, true)),
            ("status", Rule::s(10)),
            ("paymentMethod", Rule::s(50)),
            ("notes", Rule::s(500)),
        ],
        "edit_subscription" => vec![
            ("id", Rule::n(1.0, 1_000_000_000.0, true)),
            ("name", Rule::s(100)),
            ("price", Rule::n(0.0, 99_999_999.0, false)),
            ("currency", Rule::s(3)),
            ("cycle", Rule::s(20)),
            ("status", Rule::s(10)),
            ("tags", Rule::s(500)),
            ("paymentMethod", Rule::s(50)),
            ("notes", Rule::s(500)),
            ("billingDay", Rule::n(1.0, 31.0, true)),
        ],
        "delete_subscription" => vec![("id", Rule::n(1.0, 1_000_000_000.0, true))],
        "search_subscriptions" => vec![
            ("query", Rule::s(200)),
            ("names", Rule::b()),
            ("notes", Rule::b()),
            ("tags", Rule::b()),
        ],
        "list_subscriptions" => vec![
            ("sort", Rule::s(20)),
            ("desc", Rule::b()),
            ("limit", Rule::n(1.0, 10_000.0, true)),
            ("offset", Rule::n(0.0, 1_000_000_000.0, true)),
        ],
        "get_subscription" => vec![("id", Rule::n(1.0, 1_000_000_000.0, true))],
        "get_summary" => vec![],
        "get_upcoming" => vec![("days", Rule::n(0.0, 3_650.0, true))],
        "get_calendar" => vec![
            ("month", Rule::n(1.0, 12.0, true)),
            ("year", Rule::n(1.0, 9_999.0, true)),
        ],
        "export_data" => vec![("format", Rule::se(10, &["csv", "json", "md"]))],
        "get_history" => vec![
            ("id", Rule::n(1.0, 1_000_000_000.0, true)),
            ("days", Rule::n(0.0, 3_650.0, true)),
        ],
        "get_analytics" => vec![],
        "get_forecast" => vec![
            ("months", Rule::n(1.0, 120.0, true)),
            ("currency", Rule::s(3)),
            ("cancel", Rule::s(500)),
        ],
        "compare" => vec![
            ("period", Rule::se(20, CYCLE_ENUM)),
            ("currency", Rule::s(3)),
        ],
        "bulk_operations" => vec![
            ("action", Rule::se(20, BULK_ACTION_ENUM)),
            ("status", Rule::se(10, STATUS_ENUM)),
            ("tag_name", Rule::s(50)),
            ("filter_tag", Rule::s(50)),
            ("filter_status", Rule::s(10)),
            ("filter_name", Rule::s(200)),
            ("confirm", Rule::b()),
        ],
        "get_trials" => vec![("expiring_soon", Rule::n(0.0, 3_650.0, true))],
        "list_tags" => vec![],
        "get_tag_subscriptions" => vec![("tag", Rule::s(200))],
        "get_usage_total" => vec![("from", Rule::s(10)), ("to", Rule::s(10))],
        "list_usage" => vec![
            ("provider", Rule::s(50)),
            ("from", Rule::s(10)),
            ("to", Rule::s(10)),
            ("limit", Rule::n(1.0, 1_000.0, true)),
        ],
        _ => vec![],
    }
}

fn validate_args(
    args: &serde_json::Map<String, serde_json::Value>,
    schema: &[(&str, Rule)],
) -> Option<String> {
    for (key, rules) in schema {
        let value = match args.get(*key) {
            Some(v) if !v.is_null() => v,
            _ => continue,
        };
        match rules {
            Rule::Str { max_len, enum_vals } => {
                let s = match value.as_str() {
                    Some(s) => s,
                    None => return Some(format!("{} must be a string", key)),
                };
                if !enum_vals.is_empty() && !enum_vals.contains(&s) {
                    return Some(format!("{} has an unsupported value", key));
                }
                let max_len = *max_len;
                if s.chars().count() > max_len {
                    return Some(format!("{} too long (max {} chars)", key, max_len));
                }
            }
            Rule::Num { min, max, integer } => {
                let n = match value.as_f64() {
                    Some(n) if n.is_finite() => n,
                    _ => return Some(format!("{} must be a finite number", key)),
                };
                if *integer && n.fract() != 0.0 {
                    return Some(format!("{} must be an integer", key));
                }
                if n < *min {
                    return Some(format!("{} must be at least {}", key, num_arg(*min)));
                }
                if n > *max {
                    return Some(format!("{} must be at most {}", key, num_arg(*max)));
                }
            }
            Rule::Bool => {
                if !value.is_boolean() {
                    return Some(format!("{} must be a boolean", key));
                }
            }
        }
    }
    None
}

/// JS prints 1000 for 1000.0 but 1000000000 too; reuse json::number.
fn num_arg(v: f64) -> String {
    number(v)
}

fn validate_tool_call(
    name: &str,
    args: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Option<String> {
    let empty = serde_json::Map::new();
    let obj = args.unwrap_or(&empty);
    let raw = serde_json::to_string(&serde_json::Value::Object(obj.clone()))
        .unwrap_or_else(|_| "{}".to_string());
    if raw.chars().count() > MAX_REQUEST_SIZE {
        return Some(format!(
            "Request too large ({} bytes, max {})",
            raw.chars().count(),
            MAX_REQUEST_SIZE
        ));
    }
    let schema = schema_for(name);
    if !schema.is_empty()
        && let Some(err) = validate_args(obj, &schema)
    {
        return Some(format!("Validation error: {}", err));
    }
    None
}

// ── responses ────────────────────────────────────────────

struct Resp {
    text: String,
    is_error: bool,
}

impl Resp {
    fn ok(text: impl Into<String>) -> Self {
        Resp {
            text: text.into(),
            is_error: false,
        }
    }
    fn err(text: impl Into<String>) -> Self {
        Resp {
            text: text.into(),
            is_error: true,
        }
    }
    fn to_json(&self) -> String {
        let content = format!(
            "{{\"content\":[{{\"type\":\"text\",\"text\":{}}}]",
            js_str(&self.text)
        );
        if self.is_error {
            format!("{},\"isError\":true}}", content)
        } else {
            format!("{}}}", content)
        }
    }
}

// ── argument accessors ───────────────────────────────────

fn arg_str<'a>(
    args: Option<&'a serde_json::Map<String, serde_json::Value>>,
    k: &str,
) -> Option<&'a str> {
    args.and_then(|m| m.get(k)).and_then(|v| v.as_str())
}

fn arg_num(args: Option<&serde_json::Map<String, serde_json::Value>>, k: &str) -> Option<f64> {
    args.and_then(|m| m.get(k)).and_then(|v| v.as_f64())
}

fn arg_bool(args: Option<&serde_json::Map<String, serde_json::Value>>, k: &str) -> Option<bool> {
    args.and_then(|m| m.get(k)).and_then(|v| v.as_bool())
}

fn arg_present(args: Option<&serde_json::Map<String, serde_json::Value>>, k: &str) -> bool {
    args.map(|m| m.contains_key(k)).unwrap_or(false)
}

// ── handlers (port of handlers.ts) ───────────────────────

fn handle_list_subscriptions(args: Option<&serde_json::Map<String, serde_json::Value>>) -> Resp {
    let opts = db::QueryOptions {
        sort: arg_str(args, "sort").map(|s| s.to_string()),
        desc: arg_bool(args, "desc").unwrap_or(false),
        limit: arg_num(args, "limit").map(|n| n as i64),
        offset: arg_num(args, "offset").map(|n| n as i64),
        ..Default::default()
    };
    match db::get_subscriptions(&opts) {
        Ok(subs) => Resp::ok(subs_compact(&subs)),
        Err(e) => Resp::err(e.to_string()),
    }
}

fn handle_get_subscription(args: Option<&serde_json::Map<String, serde_json::Value>>) -> Resp {
    if !arg_present(args, "id") {
        return Resp::err("id is required");
    }
    let raw = arg_num(args, "id").unwrap_or(f64::NAN);
    if !raw.is_finite() || raw.fract() != 0.0 || raw < 1.0 {
        return Resp::err("id must be a positive integer");
    }
    match db::get_subscription_by_id(raw as i64) {
        Ok(Some(sub)) => Resp::ok(sub_compact(&sub)),
        Ok(None) => Resp::ok("null"),
        Err(e) => Resp::err(e.to_string()),
    }
}

fn handle_search_subscriptions(args: Option<&serde_json::Map<String, serde_json::Value>>) -> Resp {
    let query = match arg_str(args, "query") {
        Some(q) if !q.is_empty() => q.to_string(),
        _ => return Resp::err("query is required"),
    };
    for key in ["names", "notes", "tags"] {
        if arg_present(args, key) && arg_bool(args, key).is_none() {
            return Resp::err(format!("{} must be a boolean", key));
        }
    }
    match db::search_subscriptions(
        &query,
        arg_bool(args, "names").unwrap_or(false),
        arg_bool(args, "notes").unwrap_or(false),
        arg_bool(args, "tags").unwrap_or(false),
    ) {
        Ok(subs) => Resp::ok(subs_compact(&subs)),
        Err(e) => Resp::err(e.to_string()),
    }
}

fn handle_add_subscription(args: Option<&serde_json::Map<String, serde_json::Value>>) -> Resp {
    let (name, price, currency, cycle) = match (
        arg_str(args, "name"),
        arg_present(args, "price"),
        arg_str(args, "currency"),
        arg_str(args, "cycle"),
    ) {
        (Some(n), true, Some(c), Some(cy)) if !n.is_empty() => (
            n.to_string(),
            arg_num(args, "price").unwrap_or(f64::NAN),
            c.to_string(),
            cy.to_string(),
        ),
        _ => return Resp::err("name, price, currency, and cycle are required"),
    };

    if let Err(e) = crate::validation::validate_name(&name) {
        return Resp::err(format!("Invalid name: {}", e));
    }
    let price_str = number(price);
    if let Err(e) = crate::validation::validate_price(&price_str) {
        return Resp::err(format!("Invalid price: {}", e));
    }
    if !crate::validation::is_valid_currency(&currency) {
        return Resp::err(format!(
            "Invalid currency \"{}\". Use a supported 3-letter ISO code (e.g. USD, JPY)",
            currency
        ));
    }
    if !crate::validation::is_valid_cycle(&cycle) {
        return Resp::err(format!(
            "Invalid cycle \"{}\". Use: weekly, bi-weekly, monthly, quarterly, semi-annual, yearly",
            cycle
        ));
    }
    let status = arg_str(args, "status").unwrap_or("active").to_string();
    if !crate::validation::is_valid_status(&status) {
        return Resp::err(format!(
            "Invalid status \"{}\". Use: active, paused, cancelled, archived",
            status
        ));
    }

    let tags: Vec<String> = arg_str(args, "tags")
        .map(|t| {
            t.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    if tags.len() > MAX_NEW_TAGS {
        return Resp::err(format!("Maximum {} tags allowed", MAX_NEW_TAGS));
    }
    if let Err(e) = crate::validation::validate_tags(&tags.join(",")) {
        return Resp::err(format!("Invalid tags: {}", e));
    }

    if arg_present(args, "billingDay") {
        let bd = arg_num(args, "billingDay").unwrap_or(f64::NAN);
        if let Err(e) = crate::validation::validate_billing_day(&number(bd)) {
            return Resp::err(format!("Invalid billing day: {}", e));
        }
    }
    if arg_present(args, "paymentMethod") {
        let pm = arg_str(args, "paymentMethod").unwrap_or("");
        if let Err(e) = crate::validation::validate_payment_method(pm) {
            return Resp::err(format!("Invalid payment method: {}", e));
        }
    }
    if arg_present(args, "notes") {
        let notes = arg_str(args, "notes").unwrap_or("");
        if let Err(e) = crate::validation::validate_notes(notes) {
            return Resp::err(format!("Invalid notes: {}", e));
        }
    }

    let conn = match db::open_db() {
        Ok(c) => c,
        Err(e) => return Resp::err(e.to_string()),
    };
    let new = db::NewSubscription {
        name,
        price,
        currency,
        cycle,
        status,
        billing_day: arg_present(args, "billingDay")
            .then(|| arg_num(args, "billingDay").unwrap_or(0.0) as i64),
        notes: arg_str(args, "notes").map(|s| s.to_string()),
        payment_method: arg_str(args, "paymentMethod").map(|s| s.to_string()),
        auto_renewal: true,
        tags,
        ..Default::default()
    };
    match db::write_subscription(&conn, &new, true) {
        Ok(id) => Resp::ok(format!("{{\"id\":{}}}", id)),
        Err(e) => Resp::err(e.to_string()),
    }
}

fn handle_delete_subscription(args: Option<&serde_json::Map<String, serde_json::Value>>) -> Resp {
    if !arg_present(args, "id") {
        return Resp::err("id is required");
    }
    let raw = arg_num(args, "id").unwrap_or(f64::NAN);
    if !raw.is_finite() || raw.fract() != 0.0 || raw < 1.0 {
        return Resp::err("id must be a positive integer");
    }
    let conn = match db::open_db() {
        Ok(c) => c,
        Err(e) => return Resp::err(e.to_string()),
    };
    match db::remove_subscription(&conn, raw as i64, true) {
        Ok(success) => Resp::ok(format!("{{\"success\":{}}}", success)),
        Err(e) => Resp::err(e.to_string()),
    }
}

/// Per-tag aggregate: `name → (count, [(currency, monthly), ...])`.
type McpMonthlyByTag = Vec<(String, i64, Vec<(String, f64)>)>;

fn mcp_summary_json(subs: &[Subscription]) -> String {
    let mut monthly_by_currency: Vec<(String, f64)> = Vec::new();
    let mut monthly_by_tag: McpMonthlyByTag = Vec::new();
    for sub in subs {
        let monthly = sub.price * crate::date::period_factor(&sub.cycle, "monthly");
        match monthly_by_currency
            .iter_mut()
            .find(|(c, _)| *c == sub.currency)
        {
            Some((_, v)) => *v += monthly,
            None => monthly_by_currency.push((sub.currency.clone(), monthly)),
        }
        for tag in &sub.tags {
            if let Some(entry) = monthly_by_tag.iter_mut().find(|(t, _, _)| t == tag) {
                entry.1 += 1;
                match entry.2.iter_mut().find(|(c, _)| c == &sub.currency) {
                    Some((_, v)) => *v += monthly,
                    None => entry.2.push((sub.currency.clone(), monthly)),
                }
            } else {
                monthly_by_tag.push((tag.clone(), 1, vec![(sub.currency.clone(), monthly)]));
            }
        }
    }

    let mut f: Vec<String> = Vec::new();
    f.push(format!("\"totalCount\":{}", subs.len()));
    let cc: Vec<String> = monthly_by_currency
        .iter()
        .map(|(c, v)| format!("{}:{}", js_str(c), number(*v)))
        .collect();
    f.push(format!("\"monthlyByCurrency\":{{{}}}", cc.join(",")));
    let tt: Vec<String> = monthly_by_tag
        .iter()
        .map(|(tag, count, monthly)| {
            let mm: Vec<String> = monthly
                .iter()
                .map(|(c, v)| format!("{}:{}", js_str(c), number(*v)))
                .collect();
            format!(
                "{}:{{\"count\":{},\"monthly\":{{{}}}}}",
                js_str(tag),
                count,
                mm.join(",")
            )
        })
        .collect();
    f.push(format!("\"monthlyByTag\":{{{}}}", tt.join(",")));

    let mut most_expensive: Vec<(String, &Subscription)> = Vec::new();
    for sub in subs {
        match most_expensive
            .iter_mut()
            .find(|(currency, _)| *currency == sub.currency)
        {
            Some((_, current)) => {
                if sub.price > current.price {
                    *current = sub;
                }
            }
            None => most_expensive.push((sub.currency.clone(), sub)),
        }
    }
    most_expensive.sort_by(|a, b| a.0.cmp(&b.0));
    match most_expensive.as_slice() {
        [] => {}
        [(_, me)] => f.push(format!("\"mostExpensive\":{}", sub_compact(me))),
        most => {
            let entries: Vec<String> = most
                .iter()
                .map(|(currency, me)| format!("{}:{}", js_str(currency), sub_compact(me)))
                .collect();
            f.push(format!(
                "\"mostExpensiveByCurrency\":{{{}}}",
                entries.join(",")
            ));
        }
    }
    format!("{{{}}}", f.join(","))
}

fn non_cancelled() -> std::io::Result<Vec<Subscription>> {
    Ok(db::get_subscriptions(&db::QueryOptions::default())?
        .into_iter()
        .filter(|s| s.status != "cancelled")
        .collect())
}

fn handle_get_summary(_args: Option<&serde_json::Map<String, serde_json::Value>>) -> Resp {
    match non_cancelled() {
        Ok(subs) => Resp::ok(mcp_summary_json(&subs)),
        Err(e) => Resp::err(e.to_string()),
    }
}

fn handle_get_upcoming(args: Option<&serde_json::Map<String, serde_json::Value>>) -> Resp {
    let days = arg_num(args, "days").unwrap_or(7.0) as i64;
    let subs = match non_cancelled() {
        Ok(s) => s,
        Err(e) => return Resp::err(e.to_string()),
    };
    let entries = crate::upcoming::upcoming_within_days(&subs, days);
    let items: Vec<String> = entries
        .iter()
        .map(|e| {
            let sub = &subs[e.idx];
            format!(
                "{{\"sub\":{},\"nextDate\":{},\"amount\":{}}}",
                sub_compact(sub),
                js_str(&format!("{}T00:00:00.000Z", e.next_date.format("%Y-%m-%d"))),
                number(e.amount),
            )
        })
        .collect();
    Resp::ok(format!("[{}]", items.join(",")))
}

fn handle_get_calendar(args: Option<&serde_json::Map<String, serde_json::Value>>) -> Resp {
    let today = crate::date::today();
    let month = arg_num(args, "month").map(|n| n as u32).unwrap_or_else(|| {
        use chrono::Datelike;
        today.month()
    });
    let year = arg_num(args, "year").map(|n| n as i32).unwrap_or_else(|| {
        use chrono::Datelike;
        today.year()
    });
    let entries = crate::calendar::calc_calendar_entries(month, year);
    let items: Vec<String> = entries
        .iter()
        .map(|e| {
            let subs: Vec<String> = e
                .subs
                .iter()
                .map(|s| {
                    format!(
                        "{{\"name\":{},\"price\":{},\"currency\":{},\"status\":{},\"id\":{}}}",
                        js_str(&s.name),
                        number(s.price),
                        js_str(&s.currency),
                        js_str(&s.status),
                        s.id
                    )
                })
                .collect();
            format!("{{\"day\":{},\"subs\":[{}]}}", e.day, subs.join(","))
        })
        .collect();
    Resp::ok(format!("[{}]", items.join(",")))
}

fn handle_export_data(args: Option<&serde_json::Map<String, serde_json::Value>>) -> Resp {
    let format = arg_str(args, "format").unwrap_or("json").to_string();
    let subs = match db::get_subscriptions(&db::QueryOptions::default()) {
        Ok(s) => s,
        Err(e) => return Resp::err(e.to_string()),
    };
    match format.as_str() {
        "csv" => Resp::ok(crate::export::export_csv(&subs)),
        "json" => Resp::ok(format!("{}\n", crate::json::list_json(&subs))),
        "md" => Resp::ok(crate::export::export_md(&subs)),
        _ => Resp::err(format!(
            "Unsupported format: {}. Supported formats: csv, json, md",
            format
        )),
    }
}

fn handle_edit_subscription(args: Option<&serde_json::Map<String, serde_json::Value>>) -> Resp {
    if !arg_present(args, "id") {
        return Resp::err("id is required");
    }
    let raw = arg_num(args, "id").unwrap_or(f64::NAN);
    if !raw.is_finite() || raw.fract() != 0.0 || raw < 1.0 {
        return Resp::err("id must be a positive integer");
    }
    let id = raw as i64;

    if arg_present(args, "name") {
        let name = arg_str(args, "name").unwrap_or("");
        if let Err(e) = crate::validation::validate_name(name) {
            return Resp::err(format!("Invalid name: {}", e));
        }
    }
    if arg_present(args, "price") {
        let price = arg_num(args, "price").unwrap_or(f64::NAN);
        if let Err(e) = crate::validation::validate_price(&number(price)) {
            return Resp::err(format!("Invalid price: {}", e));
        }
    }
    if arg_present(args, "currency") {
        let c = arg_str(args, "currency").unwrap_or("");
        if !crate::validation::is_valid_currency(c) {
            return Resp::err(format!(
                "Invalid currency \"{}\". Use a supported 3-letter ISO code (e.g. USD, JPY)",
                c
            ));
        }
    }
    if arg_present(args, "cycle") {
        let c = arg_str(args, "cycle").unwrap_or("");
        if !crate::validation::is_valid_cycle(c) {
            return Resp::err(format!(
                "Invalid cycle \"{}\". Use: weekly, bi-weekly, monthly, quarterly, semi-annual, yearly",
                c
            ));
        }
    }
    if arg_present(args, "status") {
        let s = arg_str(args, "status").unwrap_or("");
        if !crate::validation::is_valid_status(s) {
            return Resp::err(format!(
                "Invalid status \"{}\". Use: active, paused, cancelled, archived",
                s
            ));
        }
    }
    if arg_present(args, "billingDay") {
        let bd = arg_num(args, "billingDay").unwrap_or(f64::NAN);
        if let Err(e) = crate::validation::validate_billing_day(&number(bd)) {
            return Resp::err(format!("Invalid billing day: {}", e));
        }
    }
    if arg_present(args, "paymentMethod") {
        let pm = arg_str(args, "paymentMethod").unwrap_or("");
        if let Err(e) = crate::validation::validate_payment_method(pm) {
            return Resp::err(format!("Invalid payment method: {}", e));
        }
    }
    if arg_present(args, "notes") {
        let notes = arg_str(args, "notes").unwrap_or("");
        if let Err(e) = crate::validation::validate_notes(notes) {
            return Resp::err(format!("Invalid notes: {}", e));
        }
    }

    let mut sets: Vec<(&str, SqlValue)> = Vec::new();
    if arg_present(args, "name") {
        sets.push((
            "name",
            SqlValue::Text(arg_str(args, "name").unwrap_or("").to_string()),
        ));
    }
    if arg_present(args, "price") {
        sets.push((
            "price",
            SqlValue::Real(arg_num(args, "price").unwrap_or(0.0)),
        ));
    }
    if arg_present(args, "currency") {
        sets.push((
            "currency",
            SqlValue::Text(arg_str(args, "currency").unwrap_or("").to_string()),
        ));
    }
    if arg_present(args, "cycle") {
        sets.push((
            "cycle",
            SqlValue::Text(arg_str(args, "cycle").unwrap_or("").to_string()),
        ));
    }
    if arg_present(args, "status") {
        sets.push((
            "status",
            SqlValue::Text(arg_str(args, "status").unwrap_or("").to_string()),
        ));
    }
    if arg_present(args, "billingDay") {
        sets.push((
            "billing_day",
            SqlValue::Int(arg_num(args, "billingDay").unwrap_or(0.0) as i64),
        ));
    }
    if arg_present(args, "paymentMethod") {
        let pm = arg_str(args, "paymentMethod").unwrap_or("");
        sets.push((
            "payment_method",
            if pm.is_empty() {
                SqlValue::Null
            } else {
                SqlValue::Text(pm.to_string())
            },
        ));
    }
    if arg_present(args, "notes") {
        let n = arg_str(args, "notes").unwrap_or("");
        sets.push((
            "notes",
            if n.is_empty() {
                SqlValue::Null
            } else {
                SqlValue::Text(n.to_string())
            },
        ));
    }

    let tags_arg = if arg_present(args, "tags") {
        let tags: Vec<String> = arg_str(args, "tags")
            .map(|t| {
                t.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        if tags.len() > MAX_NEW_TAGS {
            return Resp::err(format!("Maximum {} tags allowed", MAX_NEW_TAGS));
        }
        if let Err(e) = crate::validation::validate_tags(&tags.join(",")) {
            return Resp::err(format!("Invalid tags: {}", e));
        }
        Some(tags)
    } else {
        None
    };

    let conn = match db::open_db() {
        Ok(c) => c,
        Err(e) => return Resp::err(e.to_string()),
    };
    let success = match db::update_subscription(&conn, id, &sets, false) {
        Ok(s) => s,
        Err(e) => return Resp::err(e.to_string()),
    };
    if !success {
        return Resp::err(format!("Subscription with id {} not found", id));
    }
    if let Some(tags) = tags_arg
        && let Err(e) = db::set_subscription_tags(&conn, id, &tags, false)
    {
        return Resp::err(e.to_string());
    }
    if let Err(e) = db::save_db(&conn) {
        return Resp::err(e.to_string());
    }
    Resp::ok("{\"success\":true}")
}

fn handle_get_history(args: Option<&serde_json::Map<String, serde_json::Value>>) -> Resp {
    let id = arg_num(args, "id").map(|n| n as i64);
    let days = arg_num(args, "days").map(|n| n as i64);
    let entries = if let Some(id) = id {
        db::get_price_history(id)
    } else {
        db::get_all_price_changes()
    };
    let mut entries = match entries {
        Ok(e) => e,
        Err(e) => return Resp::err(e.to_string()),
    };
    if id.is_none()
        && let Some(days) = days
        && days > 0
    {
        let cutoff = crate::date::today_string_days_ago(days);
        entries.retain(|c| c.changed_at.as_str() >= cutoff.as_str());
    }
    let items: Vec<String> = entries.iter().map(price_change_compact).collect();
    Resp::ok(format!("[{}]", items.join(",")))
}

fn handle_get_analytics(_args: Option<&serde_json::Map<String, serde_json::Value>>) -> Resp {
    let all = match db::get_subscriptions(&db::QueryOptions {
        include_archived: true,
        ..Default::default()
    }) {
        Ok(s) => s,
        Err(e) => return Resp::err(e.to_string()),
    };
    let active: Vec<Subscription> = all
        .iter()
        .filter(|s| s.status != "cancelled" && s.status != "archived")
        .cloned()
        .collect();
    let count = |st: &str| all.iter().filter(|s| s.status == st).count();
    let summary = mcp_summary_json(&active);
    // Splice statusBreakdown onto the end of the summary object.
    let breakdown = format!(
        "\"statusBreakdown\":{{\"active\":{},\"paused\":{},\"cancelled\":{},\"archived\":{}}}",
        count("active"),
        count("paused"),
        count("cancelled"),
        count("archived"),
    );
    let inner = &summary[1..summary.len() - 1];
    let body = if inner.is_empty() {
        breakdown
    } else {
        format!("{},{}", inner, breakdown)
    };
    Resp::ok(format!("{{{}}}", body))
}

fn handle_get_forecast(args: Option<&serde_json::Map<String, serde_json::Value>>) -> Resp {
    let months = arg_num(args, "months").unwrap_or(12.0) as i64;
    let target = arg_str(args, "currency").map(|s| s.to_string());
    let cancel_names: Vec<String> = arg_str(args, "cancel")
        .map(|t| {
            t.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let rates = target.as_ref().map(|_| crate::fx::fetch_rates());

    let subs = match db::get_subscriptions(&db::QueryOptions::default()) {
        Ok(s) => s,
        Err(e) => return Resp::err(e.to_string()),
    };
    let active: Vec<&Subscription> = subs
        .iter()
        .filter(|s| s.status != "cancelled" && !cancel_names.contains(&s.name))
        .collect();

    let mut entries: Vec<String> = Vec::new();
    let mut raw_total = 0.0f64;
    for sub in &active {
        let monthly = sub.price * crate::date::period_factor(&sub.cycle, "monthly");
        let mut monthly_converted_raw: Option<f64> = None;
        if let (Some(tc), Some(r)) = (&target, &rates)
            && let Some(c) = crate::fx::try_convert(monthly, &sub.currency, tc, &r.rates)
        {
            monthly_converted_raw = Some(c);
        }
        raw_total += monthly_converted_raw.unwrap_or(monthly);
        let mut fields = vec![
            format!("\"name\":{}", js_str(&sub.name)),
            format!("\"price\":{}", number(sub.price)),
            format!("\"currency\":{}", js_str(&sub.currency)),
            format!("\"cycle\":{}", js_str(&sub.cycle)),
            format!(
                "\"monthly\":{}",
                number(crate::price::round_currency(monthly))
            ),
        ];
        if let Some(c) = monthly_converted_raw {
            fields.push(format!(
                "\"monthlyConverted\":{}",
                number(crate::price::round_currency(c))
            ));
        }
        entries.push(format!("{{{}}}", fields.join(",")));
    }

    let display = target.clone().unwrap_or_else(|| "mixed".to_string());
    let body = format!(
        "\"months\":{},\"currency\":{},\"monthlyTotal\":{},\"yearlyTotal\":{},\"totalSubscriptions\":{},\"entries\":[{}]",
        months,
        js_str(&display),
        number(crate::price::round_currency(raw_total)),
        number(crate::price::round_currency(raw_total * 12.0)),
        entries.len(),
        entries.join(","),
    );
    Resp::ok(format!("{{{}}}", body))
}

fn compare_totals(
    subs: &[Subscription],
    rates: &Option<crate::fx::FxRates>,
    target: Option<&str>,
    period: &str,
    use_previous: bool,
) -> Vec<(String, f64)> {
    let price_before: HashMap<i64, (f64, String)> = if use_previous {
        let mut m: HashMap<i64, (f64, String)> = HashMap::new();
        if let Ok(changes) = db::get_all_price_changes() {
            for c in &changes {
                if let Some(old) = c.old_price {
                    m.entry(c.subscription_id).or_insert_with(|| {
                        (
                            old,
                            c.old_currency
                                .clone()
                                .or_else(|| c.new_currency.clone())
                                .unwrap_or_default(),
                        )
                    });
                }
            }
        }
        m
    } else {
        HashMap::new()
    };

    let mut totals: Vec<(String, f64)> = Vec::new();
    let mut add = |ccy: String, amount: f64| match totals.iter_mut().find(|(c, _)| *c == ccy) {
        Some((_, v)) => *v += amount,
        None => totals.push((ccy, amount)),
    };

    for sub in subs {
        if sub.status == "cancelled" {
            continue;
        }
        let (price, currency) = if use_previous {
            match price_before.get(&sub.id) {
                Some((p, c)) => (*p, c.clone()),
                None => (sub.price, sub.currency.clone()),
            }
        } else {
            (sub.price, sub.currency.clone())
        };
        let monthly = price * crate::date::period_factor(&sub.cycle, period);
        match (target, rates) {
            (Some(tc), Some(r)) => match crate::fx::try_convert(monthly, &currency, tc, &r.rates) {
                Some(c) => add(tc.to_string(), c),
                None => add(currency, monthly),
            },
            _ => add(currency, monthly),
        }
    }
    totals
}

fn handle_compare(args: Option<&serde_json::Map<String, serde_json::Value>>) -> Resp {
    let period = arg_str(args, "period").unwrap_or("monthly").to_string();
    if crate::date::cycle_days(&period).is_some()
        || !crate::validation::CYCLE_VALUES.contains(&period.as_str())
    {
        return Resp::err(format!("Invalid period: {}", period));
    }
    let target = arg_str(args, "currency").map(|s| s.to_string());
    let rates = target.as_ref().map(|_| crate::fx::fetch_rates());

    let subs = match db::get_subscriptions(&db::QueryOptions::default()) {
        Ok(s) => s,
        Err(e) => return Resp::err(e.to_string()),
    };
    let current = compare_totals(&subs, &rates, target.as_deref(), &period, false);
    let previous = compare_totals(&subs, &rates, target.as_deref(), &period, true);

    let mut currencies: Vec<String> = Vec::new();
    for (c, _) in current.iter().chain(previous.iter()) {
        if !currencies.contains(c) {
            currencies.push(c.clone());
        }
    }
    currencies.sort();

    let rows: Vec<String> = currencies
        .iter()
        .map(|c| {
            let cur = current
                .iter()
                .find(|(k, _)| k == c)
                .map(|(_, v)| *v)
                .unwrap_or(0.0);
            let prev = previous
                .iter()
                .find(|(k, _)| k == c)
                .map(|(_, v)| *v)
                .unwrap_or(0.0);
            format!(
                "{{\"currency\":{},\"current\":{},\"previous\":{}}}",
                js_str(c),
                number(crate::price::round_currency(cur)),
                number(crate::price::round_currency(prev)),
            )
        })
        .collect();

    let grand_current: f64 = currencies
        .iter()
        .map(|c| {
            current
                .iter()
                .find(|(k, _)| k == c)
                .map(|(_, v)| *v)
                .unwrap_or(0.0)
        })
        .map(crate::price::round_currency)
        .sum();
    let grand_previous: f64 = currencies
        .iter()
        .map(|c| {
            previous
                .iter()
                .find(|(k, _)| k == c)
                .map(|(_, v)| *v)
                .unwrap_or(0.0)
        })
        .map(crate::price::round_currency)
        .sum();
    let change = grand_current - grand_previous;
    let change_percent = if grand_previous > 0.0 {
        ((change / grand_previous) * 10000.0).round() / 100.0
    } else {
        0.0
    };

    let body = format!(
        "\"period\":{},\"currency\":{},\"rows\":[{}],\"grandTotal\":{{\"current\":{},\"previous\":{},\"change\":{},\"changePercent\":{}}}",
        js_str(&period),
        target
            .as_deref()
            .map(js_str)
            .unwrap_or_else(|| "null".to_string()),
        rows.join(","),
        number(grand_current),
        number(grand_previous),
        number(change),
        number(change_percent),
    );
    Resp::ok(format!("{{{}}}", body))
}

fn handle_bulk_operations(args: Option<&serde_json::Map<String, serde_json::Value>>) -> Resp {
    let action = arg_str(args, "action").unwrap_or("").to_string();
    if action == "delete" && arg_bool(args, "confirm") != Some(true) {
        return Resp::err("Bulk delete requires confirm: true");
    }

    let mut filter_tag: Option<String> = None;
    let mut filter_status: Option<String> = None;
    let mut filter_name: Option<String> = None;
    if let Some(t) = arg_str(args, "filter_tag") {
        filter_tag = Some(t.to_string());
    }
    if let Some(s) = arg_str(args, "filter_status") {
        filter_status = Some(s.to_string());
    }
    if let Some(n) = arg_str(args, "filter_name") {
        filter_name = Some(n.to_string());
    }

    let subs = match db::get_subscriptions(&db::QueryOptions::default()) {
        Ok(s) => s,
        Err(e) => return Resp::err(e.to_string()),
    };
    let mut matched: Vec<Subscription> = subs;
    if let Some(tag) = &filter_tag {
        let set: Vec<String> = tag.split(',').map(|t| t.trim().to_string()).collect();
        matched.retain(|s| s.tags.iter().any(|t| set.contains(t)));
    }
    if let Some(status) = &filter_status {
        matched.retain(|s| &s.status == status);
    }
    if let Some(name) = &filter_name {
        let lower = name.to_lowercase();
        matched.retain(|s| s.name.to_lowercase().contains(&lower));
    }

    let affected: Vec<Subscription> = if action == "status" {
        matched
            .into_iter()
            .filter(|s| s.status != "cancelled")
            .collect()
    } else {
        matched
    };
    let affected_ids: Vec<i64> = affected.iter().map(|s| s.id).collect();
    if affected_ids.len() > MAX_BULK_AFFECTED {
        return Resp::err(format!(
            "Bulk operation would affect {} subscriptions (max {})",
            affected_ids.len(),
            MAX_BULK_AFFECTED
        ));
    }

    let conn = match db::open_db() {
        Ok(c) => c,
        Err(e) => return Resp::err(e.to_string()),
    };
    let mut result_count = 0usize;
    let mut errors: Vec<String> = Vec::new();
    let report = |id: i64, errors: &mut Vec<String>, msg: String| {
        errors.push(format!("id {}: {}", id, msg));
    };

    match action.as_str() {
        "status" => {
            let target_status = arg_str(args, "status").unwrap_or("active").to_string();
            if !crate::validation::is_valid_status(&target_status) {
                return Resp::err(format!(
                    "Invalid status \"{}\". Use: active, paused, cancelled, archived",
                    target_status
                ));
            }
            for id in &affected_ids {
                match db::update_subscription(
                    &conn,
                    *id,
                    &[("status", SqlValue::Text(target_status.clone()))],
                    false,
                ) {
                    Ok(true) => result_count += 1,
                    Ok(false) => {}
                    Err(e) => report(*id, &mut errors, e.to_string()),
                }
            }
        }
        "delete" => {
            for id in &affected_ids {
                match db::remove_subscription(&conn, *id, false) {
                    Ok(true) => result_count += 1,
                    Ok(false) => {}
                    Err(e) => report(*id, &mut errors, e.to_string()),
                }
            }
        }
        "tag_add" => {
            let tag_name = arg_str(args, "tag_name").unwrap_or("").trim().to_string();
            if tag_name.is_empty() {
                return Resp::err("tag_name is required for tag_add action");
            }
            if tag_name.contains(',') {
                return Resp::err("tag_name must be a single tag");
            }
            if let Err(e) = crate::validation::validate_tags(&tag_name) {
                return Resp::err(format!("Invalid tag name: {}", e));
            }
            for s in &affected {
                if !s.tags.contains(&tag_name) {
                    if s.tags.len() >= MAX_TAG_COUNT {
                        report(
                            s.id,
                            &mut errors,
                            format!("Maximum {} tags allowed", MAX_TAG_COUNT),
                        );
                        continue;
                    }
                    let mut tags = s.tags.clone();
                    tags.push(tag_name.clone());
                    match db::update_subscription(&conn, s.id, &[], false) {
                        Ok(_) => match db::set_subscription_tags(&conn, s.id, &tags, false) {
                            Ok(_) => result_count += 1,
                            Err(e) => report(s.id, &mut errors, e.to_string()),
                        },
                        Err(e) => report(s.id, &mut errors, e.to_string()),
                    }
                }
            }
        }
        "tag_remove" => {
            let tag_name = arg_str(args, "tag_name").unwrap_or("").trim().to_string();
            if tag_name.is_empty() {
                return Resp::err("tag_name is required for tag_remove action");
            }
            if tag_name.contains(',') {
                return Resp::err("tag_name must be a single tag");
            }
            if let Err(e) = crate::validation::validate_tags(&tag_name) {
                return Resp::err(format!("Invalid tag name: {}", e));
            }
            for s in &affected {
                if s.tags.contains(&tag_name) {
                    let tags: Vec<String> =
                        s.tags.iter().filter(|t| **t != tag_name).cloned().collect();
                    match db::set_subscription_tags(&conn, s.id, &tags, false) {
                        Ok(_) => result_count += 1,
                        Err(e) => report(s.id, &mut errors, e.to_string()),
                    }
                }
            }
        }
        _ => {
            return Resp::err(format!(
                "Unknown bulk action: {}. Use: status, delete, tag_add, tag_remove",
                action
            ));
        }
    }

    if result_count > 0 {
        let _ = db::save_db(&conn);
    }

    let filters = format!(
        "{{{} }}",
        [
            filter_tag
                .as_ref()
                .map(|t| format!("\"tag\":{}", js_str(t))),
            filter_status
                .as_ref()
                .map(|s| format!("\"status\":{}", js_str(s))),
            filter_name
                .as_ref()
                .map(|n| format!("\"name\":{}", js_str(n))),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(",")
    );
    let filters = filters.replace("{} ", "{}").replace("{ }", "{}");
    let ids: Vec<String> = affected_ids.iter().map(|i| i.to_string()).collect();
    let errs: Vec<String> = errors.iter().map(|e| js_str(e)).collect();
    let body = format!(
        "\"action\":{},\"filters\":{},\"matchedCount\":{},\"affectedCount\":{},\"affectedIds\":[{}],\"errors\":[{}]",
        js_str(&action),
        filters,
        affected.len(),
        result_count,
        ids.join(","),
        errs.join(","),
    );
    Resp::ok(format!("{{{}}}", body))
}

fn handle_get_trials(args: Option<&serde_json::Map<String, serde_json::Value>>) -> Resp {
    let expiring = arg_num(args, "expiring_soon").map(|n| n as i64);
    let entries = if let Some(days) = expiring {
        db::get_trials_expiring_soon(days)
    } else {
        db::get_trials()
    };
    match entries {
        Ok(list) => {
            let items: Vec<String> = list.iter().map(trial_compact).collect();
            Resp::ok(format!("[{}]", items.join(",")))
        }
        Err(e) => Resp::err(e.to_string()),
    }
}

fn handle_list_tags(_args: Option<&serde_json::Map<String, serde_json::Value>>) -> Resp {
    match db::get_tags_with_count() {
        Ok(tags) => {
            let items: Vec<String> = tags
                .iter()
                .map(|(name, count)| format!("{{\"name\":{},\"count\":{}}}", js_str(name), count))
                .collect();
            Resp::ok(format!("[{}]", items.join(",")))
        }
        Err(e) => Resp::err(e.to_string()),
    }
}

fn handle_get_tag_subscriptions(args: Option<&serde_json::Map<String, serde_json::Value>>) -> Resp {
    let tag = match arg_str(args, "tag") {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return Resp::err("tag is required"),
    };
    let names: Vec<String> = tag
        .split(',')
        .map(|t| t.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if names.is_empty() {
        return Resp::err("tag is required");
    }
    let opts = db::QueryOptions {
        tags: names,
        include_archived: true,
        ..Default::default()
    };
    match db::get_subscriptions(&opts) {
        Ok(subs) => Resp::ok(subs_compact(&subs)),
        Err(e) => Resp::err(e.to_string()),
    }
}

fn handle_get_usage_total(args: Option<&serde_json::Map<String, serde_json::Value>>) -> Resp {
    let (from, to) = if arg_present(args, "from") && arg_present(args, "to") {
        (
            arg_str(args, "from").unwrap_or("").to_string(),
            arg_str(args, "to").unwrap_or("").to_string(),
        )
    } else {
        crate::date::get_period_date_range("monthly")
    };
    let total = db::get_llm_usage_total(&from, &to).unwrap_or(0);
    let (input_tokens, output_tokens) = db::get_llm_usage_token_total(&from, &to).unwrap_or((0, 0));
    let by_provider = db::get_llm_usage_total_by_provider(&from, &to).unwrap_or_default();
    let by_model = db::get_llm_usage_total_by_model(&from, &to).unwrap_or_default();

    let providers: Vec<String> = by_provider
        .iter()
        .map(|(p, t)| format!("{{\"provider\":{},\"total\":{}}}", js_str(p), t))
        .collect();
    let models: Vec<String> = by_model
        .iter()
        .map(|(m, p, t, i, o)| {
            format!(
                "{{\"model\":{},\"provider\":{},\"total\":{},\"inputTokens\":{},\"outputTokens\":{}}}",
                js_str(m),
                js_str(p),
                t,
                i,
                o
            )
        })
        .collect();
    let body = format!(
        "\"from\":{},\"to\":{},\"total\":{},\"tokens\":{{\"inputTokens\":{},\"outputTokens\":{}}},\"byProvider\":[{}],\"byModel\":[{}]",
        js_str(&from),
        js_str(&to),
        total,
        input_tokens,
        output_tokens,
        providers.join(","),
        models.join(","),
    );
    Resp::ok(format!("{{{}}}", body))
}

fn handle_list_usage(args: Option<&serde_json::Map<String, serde_json::Value>>) -> Resp {
    let limit = arg_num(args, "limit").map(|n| n as i64).unwrap_or(100);
    match db::get_llm_usage(
        arg_str(args, "provider"),
        arg_str(args, "from"),
        arg_str(args, "to"),
        Some(0.0),
        Some(limit),
        None,
    ) {
        Ok(entries) => {
            let items: Vec<String> = entries.iter().map(llm_usage_compact).collect();
            Resp::ok(format!("[{}]", items.join(",")))
        }
        Err(e) => Resp::err(e.to_string()),
    }
}

fn call_tool(name: &str, args: Option<&serde_json::Map<String, serde_json::Value>>) -> Resp {
    if let Some(err) = validate_tool_call(name, args) {
        return Resp::err(err);
    }
    match name {
        "list_subscriptions" => handle_list_subscriptions(args),
        "get_subscription" => handle_get_subscription(args),
        "search_subscriptions" => handle_search_subscriptions(args),
        "add_subscription" => handle_add_subscription(args),
        "delete_subscription" => handle_delete_subscription(args),
        "get_summary" => handle_get_summary(args),
        "get_upcoming" => handle_get_upcoming(args),
        "get_calendar" => handle_get_calendar(args),
        "export_data" => handle_export_data(args),
        "edit_subscription" => handle_edit_subscription(args),
        "get_history" => handle_get_history(args),
        "get_analytics" => handle_get_analytics(args),
        "get_forecast" => handle_get_forecast(args),
        "compare" => handle_compare(args),
        "bulk_operations" => handle_bulk_operations(args),
        "get_trials" => handle_get_trials(args),
        "list_tags" => handle_list_tags(args),
        "get_tag_subscriptions" => handle_get_tag_subscriptions(args),
        "get_usage_total" => handle_get_usage_total(args),
        "list_usage" => handle_list_usage(args),
        _ => Resp::err(format!("Unknown tool: {}", name)),
    }
}

// ── JSON-RPC envelope ────────────────────────────────────

fn send_result(id: &str, result: &str) -> String {
    format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{},\"result\":{}}}",
        id, result
    )
}

fn send_error(id: &str, code: i64, message: &str) -> String {
    format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{},\"error\":{{\"code\":{},\"message\":{}}}}}",
        id,
        code,
        js_str(message)
    )
}

fn id_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => js_str(s),
        _ => "null".to_string(),
    }
}

fn build_initialize_result(params: Option<&serde_json::Value>) -> String {
    let protocol = params
        .and_then(|p| p.get("protocolVersion"))
        .and_then(|v| v.as_str())
        .unwrap_or(DEFAULT_PROTOCOL_VERSION);
    format!(
        "{{\"protocolVersion\":{},\"capabilities\":{{\"tools\":{{}}}},\"serverInfo\":{{\"name\":{},\"version\":{}}}}}",
        js_str(protocol),
        js_str(SERVER_NAME),
        js_str(SERVER_VERSION)
    )
}

struct Server {
    initialized: bool,
    out: Vec<String>,
    deferred: Vec<String>,
}

impl Server {
    fn new() -> Self {
        Server {
            initialized: false,
            out: Vec::new(),
            deferred: Vec::new(),
        }
    }

    fn handle_message(&mut self, msg: &serde_json::Value) {
        let m = match msg.as_object() {
            Some(o) => o,
            None => {
                self.out
                    .push(send_error("null", ERR_INVALID_REQUEST, "Invalid Request"));
                return;
            }
        };

        let id = m.get("id");
        let is_notification = matches!(id, None | Some(serde_json::Value::Null));
        if is_notification {
            return;
        }

        let method = match m.get("method").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                self.out
                    .push(send_error("null", ERR_INVALID_REQUEST, "Invalid Request"));
                return;
            }
        };
        let id_v = id.unwrap();
        if !id_v.is_number() && !id_v.is_string() {
            self.out
                .push(send_error("null", ERR_INVALID_REQUEST, "Invalid Request"));
            return;
        }
        let id_str = id_to_string(id_v);

        if !self.initialized && method != "initialize" {
            self.out.push(send_error(
                &id_str,
                ERR_INVALID_REQUEST,
                "Invalid Request: initialize required",
            ));
            return;
        }

        match method {
            "initialize" => {
                self.initialized = true;
                let result = build_initialize_result(m.get("params"));
                self.out.push(send_result(&id_str, &result));
            }
            "ping" => {
                self.out.push(send_result(&id_str, "{}"));
            }
            "tools/list" => {
                let result = format!("{{\"tools\":{}}}", tools_json());
                self.out.push(send_result(&id_str, &result));
            }
            "tools/call" => {
                let params = m.get("params").and_then(|v| v.as_object());
                let name = params.and_then(|p| p.get("name")).and_then(|v| v.as_str());
                let name = match name {
                    Some(n) => n.to_string(),
                    None => {
                        self.out.push(send_error(
                            &id_str,
                            ERR_INVALID_PARAMS,
                            "tools/call requires a string params.name",
                        ));
                        return;
                    }
                };
                let args = params
                    .and_then(|p| p.get("arguments"))
                    .and_then(|v| v.as_object());
                let resp = call_tool(&name, args);
                self.deferred.push(send_result(&id_str, &resp.to_json()));
            }
            other => {
                self.out.push(send_error(
                    &id_str,
                    ERR_METHOD_NOT_FOUND,
                    &format!("Method not found: {}", other),
                ));
            }
        }
    }

    fn handle_raw(&mut self, text: &str) {
        if text.len() > MAX_REQUEST_SIZE {
            self.out.push(send_error(
                "null",
                ERR_PARSE,
                &format!("Request too large (max {} bytes)", MAX_REQUEST_SIZE),
            ));
            return;
        }
        match serde_json::from_str::<serde_json::Value>(text) {
            Ok(v) => self.handle_message(&v),
            Err(_) => self.out.push(send_error("null", ERR_PARSE, "Parse error")),
        }
    }

    /// Consume a byte buffer using the same framing state machine as
    /// transport.ts (line mode auto-switches to Content-Length headers).
    fn consume(&mut self, input: &[u8]) {
        let buffer: Vec<u8> = input.to_vec();
        let mut pos = 0usize;
        let mut mode = 0u8; // 0 = line, 1 = headers, 2 = body
        let mut header_text = String::new();
        let mut body_remaining = 0usize;
        let mut skip_remaining = 0usize;

        loop {
            if skip_remaining > 0 {
                if pos >= buffer.len() {
                    break;
                }
                let drop = std::cmp::min(buffer.len() - pos, skip_remaining);
                pos += drop;
                skip_remaining -= drop;
                if skip_remaining == 0 {
                    mode = 0;
                }
                continue;
            }

            if mode == 0 {
                let nl = buffer[pos..].iter().position(|&b| b == 0x0a);
                let nl = match nl {
                    Some(n) => pos + n,
                    None => {
                        if buffer.len() - pos >= MAX_REQUEST_SIZE {
                            self.out.push(send_error(
                                "null",
                                ERR_PARSE,
                                &format!("Request too large (max {} bytes)", MAX_REQUEST_SIZE),
                            ));
                            pos = buffer.len();
                        }
                        break;
                    }
                };
                let mut line = buffer[pos..nl].to_vec();
                pos = nl + 1;
                if line.last() == Some(&0x0d) {
                    line.pop();
                }
                if line.is_empty() {
                    continue;
                }
                if line.len() > MAX_REQUEST_SIZE {
                    self.out.push(send_error(
                        "null",
                        ERR_PARSE,
                        &format!("Request too large (max {} bytes)", MAX_REQUEST_SIZE),
                    ));
                    continue;
                }
                let text = String::from_utf8_lossy(&line).to_string();
                if is_content_length_line(text.trim()) {
                    mode = 1;
                    header_text = text;
                    continue;
                }
                self.handle_raw(&text);
                continue;
            }

            if mode == 1 {
                let nl = buffer[pos..].iter().position(|&b| b == 0x0a);
                let nl = match nl {
                    Some(n) => pos + n,
                    None => {
                        if header_text.len() + (buffer.len() - pos) > MAX_REQUEST_SIZE {
                            self.out.push(send_error(
                                "null",
                                ERR_PARSE,
                                &format!("Request too large (max {} bytes)", MAX_REQUEST_SIZE),
                            ));
                            header_text.clear();
                            pos = buffer.len();
                        }
                        break;
                    }
                };
                let mut line = String::from_utf8_lossy(&buffer[pos..nl]).to_string();
                pos = nl + 1;
                if line.ends_with('\r') {
                    line.pop();
                }
                if line.is_empty() {
                    let length = parse_content_length(&header_text);
                    header_text.clear();
                    let length = match length {
                        Some(l) => l,
                        None => {
                            self.out.push(send_error(
                                "null",
                                ERR_INVALID_REQUEST,
                                "Invalid Request: missing Content-Length header",
                            ));
                            mode = 0;
                            continue;
                        }
                    };
                    if length > MAX_REQUEST_SIZE {
                        self.out.push(send_error(
                            "null",
                            ERR_PARSE,
                            &format!("Request too large (max {} bytes)", MAX_REQUEST_SIZE),
                        ));
                        skip_remaining = length;
                        mode = 0;
                        continue;
                    }
                    mode = 2;
                    body_remaining = length;
                    continue;
                }
                if header_text.is_empty() {
                    header_text = line;
                } else {
                    header_text.push('\n');
                    header_text.push_str(&line);
                }
                continue;
            }

            // mode == 2 (body)
            if buffer.len() - pos < body_remaining {
                break;
            }
            let body = buffer[pos..pos + body_remaining].to_vec();
            pos += body_remaining;
            mode = 0;
            body_remaining = 0;
            self.handle_raw(&String::from_utf8_lossy(&body));
        }

        // Leftover bytes in line mode with no trailing newline (EOF).
        if mode == 0 && skip_remaining == 0 && pos < buffer.len() {
            let mut text = String::from_utf8_lossy(&buffer[pos..]).to_string();
            if text.ends_with('\r') {
                text.pop();
            }
            if text.len() > MAX_REQUEST_SIZE {
                self.out.push(send_error(
                    "null",
                    ERR_PARSE,
                    &format!("Request too large (max {} bytes)", MAX_REQUEST_SIZE),
                ));
            } else if !text.is_empty() {
                self.handle_raw(&text);
            }
        }
    }
}

fn is_content_length_line(s: &str) -> bool {
    let lower = s.to_lowercase();
    let rest = match lower.strip_prefix("content-length:") {
        Some(r) => r,
        None => return false,
    };
    let trimmed = rest.trim();
    !trimmed.is_empty() && trimmed.chars().all(|c| c.is_ascii_digit())
}

fn parse_content_length(headers: &str) -> Option<usize> {
    for line in headers.lines() {
        let lower = line.to_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:")
            && let Ok(n) = rest.trim().parse::<usize>()
        {
            return Some(n);
        }
    }
    None
}

// ── entry point ──────────────────────────────────────────

pub fn run_mcp(_parsed: &ParsedArgs) -> i32 {
    use std::io::Read;
    let mut input = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut input);

    let mut server = Server::new();
    server.consume(&input);
    // Async tool responses resolve after the current synchronous stack —
    // flush them last (mirrors the promise microtask ordering in transport.ts).
    let mut lines = server.out;
    lines.append(&mut server.deferred);
    for line in lines {
        logger::outln(&line);
    }
    0
}
