//! JS-compatible JSON emitter — mirrors `JSON.stringify(value, null, 2)`
//! for the database rows subtrack serializes (list --json).
//!
//! serde_json is avoided because its float formatting prints "5999.0"
//! for integral f64 values, whereas JS prints "5999".

/// Quote + escape a JSON string exactly like JS JSON.stringify.
pub fn quote_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// JS-style number: integral values print without a decimal point.
pub fn number(v: f64) -> String {
    if v == 0.0 {
        return "0".to_string()
    }
    if v.is_finite() && v.fract() == 0.0 && v.abs() < 1e21 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}

/// A `serde_json` number value using JS number formatting (980, not 980.0).
pub fn num_value(v: f64) -> serde_json::Value {
    serde_json::from_str(&number(v)).unwrap_or(serde_json::Value::Null)
}

/// Emit a string value.
fn string(v: &str) -> String {
    quote_string(v)
}

/// Emit `JSON.stringify(list, null, 2)` for subscription rows.
/// Key order mirrors the object key order of the TS rows (SUB_COLUMNS order,
/// then `tags` appended by mapTags).
pub fn list_json(list: &[crate::db::Subscription]) -> String {
    let items: Vec<String> = list.iter().map(sub_json).collect();
    format!("[\n{}\n]", items.join(",\n"))
}

pub fn sub_json(sub: &crate::db::Subscription) -> String {
    let mut fields: Vec<String> = Vec::new();
    let kv = |k: &str, v: String| format!("{}: {}", quote_string(k), v);

    fields.push(kv("id", sub.id.to_string()));
    fields.push(kv("name", string(&sub.name)));
    fields.push(kv("price", number(sub.price)));
    fields.push(kv("currency", string(&sub.currency)));
    fields.push(kv("cycle", string(&sub.cycle)));
    fields.push(kv("status", string(&sub.status)));
    fields.push(kv(
        "billingDay",
        sub.billing_day.map(|v| v.to_string()).unwrap_or_else(|| "null".to_string()),
    ));
    fields.push(kv("createdAt", string(&sub.created_at)));
    fields.push(kv(
        "notes",
        sub.notes.as_deref().map(string).unwrap_or_else(|| "null".to_string()),
    ));
    fields.push(kv(
        "paymentMethod",
        sub.payment_method.as_deref().map(string).unwrap_or_else(|| "null".to_string()),
    ));
    fields.push(kv(
        "contractStart",
        sub.contract_start.as_deref().map(string).unwrap_or_else(|| "null".to_string()),
    ));
    fields.push(kv(
        "contractEnd",
        sub.contract_end.as_deref().map(string).unwrap_or_else(|| "null".to_string()),
    ));
    fields.push(kv("autoRenewal", if sub.auto_renewal { "true" } else { "false" }.to_string()));
    fields.push(kv(
        "vendorName",
        sub.vendor_name.as_deref().map(string).unwrap_or_else(|| "null".to_string()),
    ));
    fields.push(kv(
        "vendorUrl",
        sub.vendor_url.as_deref().map(string).unwrap_or_else(|| "null".to_string()),
    ));
    fields.push(kv(
        "planTier",
        sub.plan_tier.as_deref().map(string).unwrap_or_else(|| "null".to_string()),
    ));
    fields.push(kv(
        "discountAmount",
        sub.discount_amount.map(number).unwrap_or_else(|| "null".to_string()),
    ));
    fields.push(kv(
        "discountType",
        sub.discount_type.as_deref().map(string).unwrap_or_else(|| "null".to_string()),
    ));
    fields.push(kv("tags", tags_json(&sub.tags)));

    format!("  {{\n    {}\n  }}", fields.join(",\n    "))
}

fn tags_json(tags: &[String]) -> String {
    if tags.is_empty() {
        return "[]".to_string()
    }
    let items: Vec<String> = tags.iter().map(|t| string(t)).collect();
    format!("[\n      {}\n    ]", items.join(",\n      "))
}