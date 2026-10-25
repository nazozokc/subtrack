use subtrack_core::json::quote_string as js_str;

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

pub(super) fn tools_json() -> String {
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
