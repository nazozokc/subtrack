use crate::mcp::compact::{sub_compact, subs_compact};
use crate::mcp::response::{Resp, arg_bool, arg_num, arg_present, arg_str};
use crate::mcp::schema::MAX_NEW_TAGS;
use subtrack_core::json::number;
use subtrack_db::db::{self, SqlValue};

pub(super) fn handle_list_subscriptions(
    args: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Resp {
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

pub(super) fn handle_get_subscription(
    args: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Resp {
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

pub(super) fn handle_search_subscriptions(
    args: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Resp {
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

pub(super) fn handle_add_subscription(
    args: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Resp {
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

    if let Err(e) = subtrack_core::validation::validate_name(&name) {
        return Resp::err(format!("Invalid name: {}", e));
    }
    let price_str = number(price);
    if let Err(e) = subtrack_core::validation::validate_price(&price_str) {
        return Resp::err(format!("Invalid price: {}", e));
    }
    if !subtrack_core::validation::is_valid_currency(&currency) {
        return Resp::err(format!(
            "Invalid currency \"{}\". Use a supported 3-letter ISO code (e.g. USD, JPY)",
            currency
        ));
    }
    if !subtrack_core::validation::is_valid_cycle(&cycle) {
        return Resp::err(format!(
            "Invalid cycle \"{}\". Use: weekly, bi-weekly, monthly, quarterly, semi-annual, yearly",
            cycle
        ));
    }
    let status = arg_str(args, "status").unwrap_or("active").to_string();
    if !subtrack_core::validation::is_valid_status(&status) {
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
    if let Err(e) = subtrack_core::validation::validate_tags(&tags.join(",")) {
        return Resp::err(format!("Invalid tags: {}", e));
    }

    if arg_present(args, "billingDay") {
        let bd = arg_num(args, "billingDay").unwrap_or(f64::NAN);
        if let Err(e) = subtrack_core::validation::validate_billing_day(&number(bd)) {
            return Resp::err(format!("Invalid billing day: {}", e));
        }
    }
    if arg_present(args, "paymentMethod") {
        let pm = arg_str(args, "paymentMethod").unwrap_or("");
        if let Err(e) = subtrack_core::validation::validate_payment_method(pm) {
            return Resp::err(format!("Invalid payment method: {}", e));
        }
    }
    if arg_present(args, "notes") {
        let notes = arg_str(args, "notes").unwrap_or("");
        if let Err(e) = subtrack_core::validation::validate_notes(notes) {
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

pub(super) fn handle_delete_subscription(
    args: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Resp {
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
pub(super) type McpMonthlyByTag = Vec<(String, i64, Vec<(String, f64)>)>;

pub(super) fn handle_edit_subscription(
    args: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Resp {
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
        if let Err(e) = subtrack_core::validation::validate_name(name) {
            return Resp::err(format!("Invalid name: {}", e));
        }
    }
    if arg_present(args, "price") {
        let price = arg_num(args, "price").unwrap_or(f64::NAN);
        if let Err(e) = subtrack_core::validation::validate_price(&number(price)) {
            return Resp::err(format!("Invalid price: {}", e));
        }
    }
    if arg_present(args, "currency") {
        let c = arg_str(args, "currency").unwrap_or("");
        if !subtrack_core::validation::is_valid_currency(c) {
            return Resp::err(format!(
                "Invalid currency \"{}\". Use a supported 3-letter ISO code (e.g. USD, JPY)",
                c
            ));
        }
    }
    if arg_present(args, "cycle") {
        let c = arg_str(args, "cycle").unwrap_or("");
        if !subtrack_core::validation::is_valid_cycle(c) {
            return Resp::err(format!(
                "Invalid cycle \"{}\". Use: weekly, bi-weekly, monthly, quarterly, semi-annual, yearly",
                c
            ));
        }
    }
    if arg_present(args, "status") {
        let s = arg_str(args, "status").unwrap_or("");
        if !subtrack_core::validation::is_valid_status(s) {
            return Resp::err(format!(
                "Invalid status \"{}\". Use: active, paused, cancelled, archived",
                s
            ));
        }
    }
    if arg_present(args, "billingDay") {
        let bd = arg_num(args, "billingDay").unwrap_or(f64::NAN);
        if let Err(e) = subtrack_core::validation::validate_billing_day(&number(bd)) {
            return Resp::err(format!("Invalid billing day: {}", e));
        }
    }
    if arg_present(args, "paymentMethod") {
        let pm = arg_str(args, "paymentMethod").unwrap_or("");
        if let Err(e) = subtrack_core::validation::validate_payment_method(pm) {
            return Resp::err(format!("Invalid payment method: {}", e));
        }
    }
    if arg_present(args, "notes") {
        let notes = arg_str(args, "notes").unwrap_or("");
        if let Err(e) = subtrack_core::validation::validate_notes(notes) {
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
        if let Err(e) = subtrack_core::validation::validate_tags(&tags.join(",")) {
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
