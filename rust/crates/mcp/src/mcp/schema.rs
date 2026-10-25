use subtrack_core::json::number;

// ── security limits ──────────────────────────────────────

pub(super) const MAX_REQUEST_SIZE: usize = 1024 * 100;
/// Default max string length; mirrors `MAX_STRING_LENGTH` in security.ts
/// (every per-tool schema passes an explicit `maxLength`, so it is unused).
#[allow(dead_code)]
pub(super) const MAX_STRING_LENGTH: usize = 500;
pub(super) const MAX_TAG_COUNT: usize = 20;
pub(super) const MAX_NEW_TAGS: usize = 10;
pub(super) const MAX_BULK_AFFECTED: usize = 1_000;
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

pub(super) fn validate_tool_call(
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
