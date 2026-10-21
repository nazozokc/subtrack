use super::parse_id;
use crate::db::get_subscription_by_id;
use crate::logger::{self, fail, info};
use crate::parser::ParsedArgs;
use crate::prompts::prompt_cycle;
use crate::validation::{
    STATUS_VALUES, is_valid_currency, is_valid_status, validate_billing_day, validate_date_string,
    validate_discount_type, validate_discount_value, validate_name, validate_notes,
    validate_payment_method, validate_plan_tier, validate_price, validate_vendor_name,
    validate_vendor_url,
};

pub fn run_edit(parsed: &ParsedArgs) -> i32 {
    let all = match crate::db::get_subscriptions(&crate::db::QueryOptions {
        include_archived: true,
        ..Default::default()
    }) {
        Ok(v) => v,
        Err(e) => {
            logger::fail(&format!("Failed to read subscriptions: {}", e));
            return 1;
        }
    };
    if all.is_empty() {
        info("No subscriptions found — try `subtrack add`");
        return 0;
    }
    let id = match parse_id(parsed.str("id")) {
        Some(id) => id,
        None => match crate::prompt::pick_subscription("select subscription to edit", &all, true) {
            Some(id) => id,
            None => return 0,
        },
    };
    let sub = match get_subscription_by_id(id) {
        Ok(Some(s)) => s,
        Ok(None) => {
            fail(&format!("Subscription with id {} not found", id));
            return 1;
        }
        Err(e) => {
            logger::fail(&format!("Failed to read subscription: {}", e));
            return 1;
        }
    };

    // hasFlags: any of the editable flags present.
    let flag_keys = [
        "name",
        "price",
        "currency",
        "cycle",
        "status",
        "billingDay",
        "tags",
        "notes",
        "paymentMethod",
        "vendorName",
        "vendorUrl",
        "planTier",
        "discountAmount",
        "discountType",
        "contractStart",
        "contractEnd",
        "autoRenewal",
    ];
    let has_flags = flag_keys.iter().any(|k| parsed.str(k).is_some());
    if !has_flags {
        if crate::prompt::interactive() {
            return interactive_edit(id);
        }
        info(&format!(
            "Editing: {} — {}/{} [{}]",
            sub.name,
            crate::price::format_price(sub.price, &sub.currency),
            sub.cycle,
            if sub.tags.is_empty() {
                "no tags".to_string()
            } else {
                sub.tags.join(", ")
            }
        ));
        return 0;
    }

    use crate::db::SqlValue;
    let mut sets: Vec<(&str, SqlValue)> = Vec::new();
    let mut changed_keys: Vec<&str> = Vec::new();
    let mut new_tags: Option<Vec<String>> = None;

    if let Some(v) = parsed.str("name") {
        match validate_name(&v) {
            Ok(()) => {
                sets.push(("name", SqlValue::Text(v)));
                changed_keys.push("name");
            }
            Err(e) => {
                fail(&format!("Invalid name: {}", e));
                return 1;
            }
        }
    }
    if let Some(v) = parsed.str("price") {
        match validate_price(&v) {
            Ok(()) => {
                sets.push((
                    "price",
                    SqlValue::Real(v.trim().parse::<f64>().unwrap_or(0.0)),
                ));
                changed_keys.push("price");
            }
            Err(e) => {
                fail(&format!("Invalid price: {}", e));
                return 1;
            }
        }
    }
    if let Some(v) = parsed.str("currency") {
        if !is_valid_currency(&v) {
            fail(&format!("Invalid currency: \"{}\"", v));
            return 1;
        }
        sets.push(("currency", SqlValue::Text(v)));
        changed_keys.push("currency");
    }
    if let Some(v) = parsed.str("cycle") {
        if !crate::validation::is_valid_cycle(&v) {
            fail(&format!("Invalid cycle: \"{}\"", v));
            return 1;
        }
        sets.push(("cycle", SqlValue::Text(v)));
        changed_keys.push("cycle");
    }
    if let Some(v) = parsed.str("status") {
        if !is_valid_status(&v) {
            fail(&format!("Invalid status: \"{}\"", v));
            return 1;
        }
        sets.push(("status", SqlValue::Text(v)));
        changed_keys.push("status");
    }
    if let Some(raw) = parsed.str("billingDay") {
        let trimmed = raw.trim();
        if let Err(e) = validate_billing_day(trimmed) {
            fail(&format!("Invalid billing day: {}", e));
            return 1;
        }
        let val = if trimmed.is_empty() {
            SqlValue::Null
        } else {
            SqlValue::Int(trimmed.parse::<i64>().unwrap_or(0))
        };
        sets.push(("billing_day", val));
        changed_keys.push("billingDay");
    }
    if let Some(v) = parsed.str("tags") {
        if let Err(e) = crate::validation::validate_tags(&v) {
            fail(&format!("Invalid tags: {}", e));
            return 1;
        }
        new_tags = Some(
            v.split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect(),
        );
        changed_keys.push("tags");
    }
    if let Some(v) = parsed.str("notes") {
        let trimmed = v.trim();
        if let Err(e) = validate_notes(trimmed) {
            fail(&format!("Invalid notes: {}", e));
            return 1;
        }
        sets.push(("notes", opt_text_val(trimmed)));
        changed_keys.push("notes");
    }
    if let Some(v) = parsed.str("paymentMethod") {
        let trimmed = v.trim();
        if let Err(e) = validate_payment_method(trimmed) {
            fail(&format!("Invalid payment method: {}", e));
            return 1;
        }
        sets.push(("payment_method", opt_text_val(trimmed)));
        changed_keys.push("paymentMethod");
    }
    if let Some(v) = parsed.str("vendorName") {
        let trimmed = v.trim();
        if let Err(e) = validate_vendor_name(trimmed) {
            fail(&format!("Invalid vendor name: {}", e));
            return 1;
        }
        sets.push(("vendor_name", opt_text_val(trimmed)));
        changed_keys.push("vendorName");
    }
    if let Some(v) = parsed.str("vendorUrl") {
        let trimmed = v.trim();
        if let Err(e) = validate_vendor_url(trimmed) {
            fail(&format!("Invalid vendor URL: {}", e));
            return 1;
        }
        sets.push(("vendor_url", opt_text_val(trimmed)));
        changed_keys.push("vendorUrl");
    }
    if let Some(v) = parsed.str("planTier") {
        let trimmed = v.trim();
        if let Err(e) = validate_plan_tier(trimmed) {
            fail(&format!("Invalid plan tier: {}", e));
            return 1;
        }
        sets.push(("plan_tier", opt_text_val(trimmed)));
        changed_keys.push("planTier");
    }
    if let Some(v) = parsed.str("contractStart") {
        let trimmed = v.trim();
        if let Err(e) = validate_date_string(trimmed) {
            fail(&format!("Invalid contract start: {}", e));
            return 1;
        }
        sets.push(("contract_start", opt_text_val(trimmed)));
        changed_keys.push("contractStart");
    }
    if let Some(v) = parsed.str("contractEnd") {
        let trimmed = v.trim();
        if let Err(e) = validate_date_string(trimmed) {
            fail(&format!("Invalid contract end: {}", e));
            return 1;
        }
        sets.push(("contract_end", opt_text_val(trimmed)));
        changed_keys.push("contractEnd");
    }
    if let Some(v) = parsed.str("discountAmount") {
        let trimmed = v.trim();
        if let Err(e) = validate_discount_value(trimmed) {
            fail(&format!("Invalid discount amount: {}", e));
            return 1;
        }
        let val = if trimmed.is_empty() {
            SqlValue::Null
        } else {
            SqlValue::Real(trimmed.parse::<f64>().unwrap_or(0.0))
        };
        sets.push(("discount_amount", val));
        changed_keys.push("discountAmount");
    }
    if let Some(v) = parsed.str("discountType") {
        let trimmed = v.trim();
        if let Err(e) = validate_discount_type(trimmed) {
            fail(&format!("Invalid discount type: {}", e));
            return 1;
        }
        sets.push(("discount_type", opt_text_val(trimmed)));
        changed_keys.push("discountType");
    }
    if let Some(v) = parsed.str("autoRenewal") {
        if let Err(e) = crate::validation::validate_auto_renewal(&v) {
            fail(&format!("Invalid autoRenewal: {}", e));
            return 1;
        }
        sets.push((
            "auto_renewal",
            SqlValue::Int(if v == "true" { 1 } else { 0 }),
        ));
        changed_keys.push("autoRenewal");
    }

    let conn = match crate::db::open_db() {
        Ok(c) => c,
        Err(e) => {
            logger::fail(&format!("Failed to edit subscription: {}", e));
            return 1;
        }
    };
    let new_price = parsed
        .str("price")
        .and_then(|v| v.trim().parse::<f64>().ok())
        .unwrap_or(sub.price);
    let new_currency = parsed
        .str("currency")
        .unwrap_or_else(|| sub.currency.clone());

    if let Err(e) = crate::db::update_subscription(&conn, sub.id, &sets, false) {
        fail(&format!("Failed to edit subscription: {}", e));
        return 1;
    }
    if let Some(tags) = &new_tags
        && let Err(e) = crate::db::replace_tags(&conn, sub.id, tags)
    {
        fail(&format!("Failed to edit subscription: {}", e));
        return 1;
    }
    // Record price history when price or currency changed.
    if sub.price != new_price || sub.currency != new_currency {
        let _ = crate::db::write_price_history(
            &conn,
            sub.id,
            sub.price,
            new_price,
            &sub.currency,
            &new_currency,
        );
    }
    let _ = crate::db::save_db(&conn);
    let updated = crate::db::get_subscription_by_id(sub.id).ok().flatten();
    let details = format!("{}: {} changed", sub.name, changed_keys.join(", "));
    let _ = crate::db::add_audit_log(
        &conn,
        "subscription.edit",
        Some("subscription"),
        Some(sub.id),
        Some(&details),
        false,
    );
    let _ = crate::db::save_db(&conn);
    if let Some(u) = updated {
        logger::success(&format!(
            "Updated: {} — {}/{}",
            u.name,
            crate::price::format_price(u.price, &u.currency),
            u.cycle
        ));
    }
    0
}

/// Guided edit: pick fields, answer one prompt per field, then run the normal
/// flag-based update path with the collected values.
fn interactive_edit(id: i64) -> i32 {
    use crate::parser::ArgValue;
    use crate::prompt::{self, Choice};

    let sub = match get_subscription_by_id(id) {
        Ok(Some(s)) => s,
        Ok(None) => {
            fail(&format!("Subscription with id {} not found", id));
            return 1;
        }
        Err(e) => {
            logger::fail(&format!("Failed to read subscription: {}", e));
            return 1;
        }
    };

    let choices = vec![
        Choice::new(&format!("name ({})", sub.name), "name"),
        Choice::new(
            &format!(
                "price ({})",
                crate::price::format_price(sub.price, &sub.currency)
            ),
            "price",
        ),
        Choice::new(&format!("currency ({})", sub.currency), "currency"),
        Choice::new(&format!("cycle ({})", sub.cycle), "cycle"),
        Choice::new(&format!("status ({})", sub.status), "status"),
        Choice::new(
            &format!(
                "billing day ({})",
                sub.billing_day
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| "not set".to_string())
            ),
            "billingDay",
        ),
        Choice::new(
            &format!(
                "tags ({})",
                if sub.tags.is_empty() {
                    "none".to_string()
                } else {
                    sub.tags.join(", ")
                }
            ),
            "tags",
        ),
        Choice::new(
            &format!("notes ({})", sub.notes.as_deref().unwrap_or("not set")),
            "notes",
        ),
        Choice::new(
            &format!(
                "payment method ({})",
                sub.payment_method.as_deref().unwrap_or("not set")
            ),
            "paymentMethod",
        ),
        Choice::new(
            &format!(
                "vendor name ({})",
                sub.vendor_name.as_deref().unwrap_or("not set")
            ),
            "vendorName",
        ),
        Choice::new(
            &format!(
                "vendor URL ({})",
                sub.vendor_url.as_deref().unwrap_or("not set")
            ),
            "vendorUrl",
        ),
        Choice::new(
            &format!(
                "plan tier ({})",
                sub.plan_tier.as_deref().unwrap_or("not set")
            ),
            "planTier",
        ),
        Choice::new(
            &format!(
                "contract start ({})",
                sub.contract_start.as_deref().unwrap_or("not set")
            ),
            "contractStart",
        ),
        Choice::new(
            &format!(
                "contract end ({})",
                sub.contract_end.as_deref().unwrap_or("not set")
            ),
            "contractEnd",
        ),
        Choice::new(
            &format!(
                "auto renew ({})",
                if sub.auto_renewal { "yes" } else { "no" }
            ),
            "autoRenewal",
        ),
        Choice::new(
            &format!(
                "discount ({})",
                match sub.discount_amount {
                    Some(amount) => format!(
                        "{}{}",
                        amount,
                        if sub.discount_type.as_deref() == Some("percentage") {
                            "%"
                        } else {
                            sub.currency.as_str()
                        }
                    ),
                    None => "none".to_string(),
                }
            ),
            "discount",
        ),
    ];

    info(&format!(
        "Editing: {} — {}/{} [{}]",
        sub.name,
        crate::price::format_price(sub.price, &sub.currency),
        sub.cycle,
        if sub.tags.is_empty() {
            "no tags".to_string()
        } else {
            sub.tags.join(", ")
        }
    ));

    let Some(fields) = prompt::checkbox("Select fields to edit", &choices) else {
        return 0;
    };
    if fields.is_empty() {
        info("Cancelled");
        return 0;
    }

    let mut values: std::collections::HashMap<String, ArgValue> = std::collections::HashMap::new();
    values.insert("id".to_string(), ArgValue::Str(id.to_string()));

    for field in fields {
        match field.as_str() {
            "name" => {
                let Some(v) =
                    prompt::input("New name", Some(sub.name.as_str()), Some(validate_name))
                else {
                    return 0;
                };
                values.insert("name".to_string(), ArgValue::Str(v));
            }
            "price" => {
                let default = format!("{}", sub.price);
                let Some(v) =
                    prompt::input("New price", Some(default.as_str()), Some(validate_price))
                else {
                    return 0;
                };
                values.insert("price".to_string(), ArgValue::Str(v));
            }
            "currency" => {
                let entries: Vec<Choice> = crate::prompts::CURRENCY_CHOICES
                    .iter()
                    .map(|(label, value)| Choice::new(label, value))
                    .collect();
                let default = entries.iter().position(|c| c.value == sub.currency);
                let Some(v) = prompt::select("New currency", &entries, default) else {
                    return 0;
                };
                values.insert("currency".to_string(), ArgValue::Str(v));
            }
            "cycle" => {
                let Some(v) = prompt_cycle(None, "New cycle") else {
                    return 0;
                };
                values.insert("cycle".to_string(), ArgValue::Str(v));
            }
            "status" => {
                let entries: Vec<Choice> = STATUS_VALUES.iter().map(|v| Choice::plain(v)).collect();
                let default = entries.iter().position(|c| c.value == sub.status);
                let Some(v) = prompt::select("New status", &entries, default) else {
                    return 0;
                };
                values.insert("status".to_string(), ArgValue::Str(v));
            }
            "billingDay" => {
                let default = sub.billing_day.map(|d| d.to_string()).unwrap_or_default();
                let Some(v) = prompt::input(
                    "New billing day (1-31, empty to clear)",
                    Some(default.as_str()),
                    Some(validate_billing_day),
                ) else {
                    return 0;
                };
                values.insert("billingDay".to_string(), ArgValue::Str(v));
            }
            "tags" => {
                let default = sub.tags.join(", ");
                let existing: Vec<String> = crate::db::get_tags_with_count()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(name, _)| name)
                    .collect();
                let message = if existing.is_empty() {
                    "New tags (comma-separated)".to_string()
                } else {
                    format!(
                        "New tags (comma-separated; existing: {})",
                        existing.join(", ")
                    )
                };
                let Some(v) = prompt::input(
                    &message,
                    Some(default.as_str()),
                    Some(crate::validation::validate_tags),
                ) else {
                    return 0;
                };
                values.insert("tags".to_string(), ArgValue::Str(v));
            }
            "notes" => {
                let default = sub.notes.clone().unwrap_or_default();
                let Some(v) = prompt::input(
                    "New notes (empty to clear)",
                    Some(default.as_str()),
                    Some(validate_notes),
                ) else {
                    return 0;
                };
                values.insert("notes".to_string(), ArgValue::Str(v));
            }
            "paymentMethod" => {
                let default = sub.payment_method.clone().unwrap_or_default();
                let Some(v) = prompt::input(
                    "New payment method (empty to clear)",
                    Some(default.as_str()),
                    Some(validate_payment_method),
                ) else {
                    return 0;
                };
                values.insert("paymentMethod".to_string(), ArgValue::Str(v));
            }
            "vendorName" => {
                let default = sub.vendor_name.clone().unwrap_or_default();
                let Some(v) = prompt::input(
                    "New vendor name (empty to clear)",
                    Some(default.as_str()),
                    Some(validate_vendor_name),
                ) else {
                    return 0;
                };
                values.insert("vendorName".to_string(), ArgValue::Str(v));
            }
            "vendorUrl" => {
                let default = sub.vendor_url.clone().unwrap_or_default();
                let Some(v) = prompt::input(
                    "New vendor URL (empty to clear)",
                    Some(default.as_str()),
                    Some(validate_vendor_url),
                ) else {
                    return 0;
                };
                values.insert("vendorUrl".to_string(), ArgValue::Str(v));
            }
            "planTier" => {
                let default = sub.plan_tier.clone().unwrap_or_default();
                let Some(v) = prompt::input(
                    "New plan tier (empty to clear)",
                    Some(default.as_str()),
                    Some(validate_plan_tier),
                ) else {
                    return 0;
                };
                values.insert("planTier".to_string(), ArgValue::Str(v));
            }
            "contractStart" => {
                let default = sub.contract_start.clone().unwrap_or_default();
                let Some(v) = prompt::input(
                    "New contract start (YYYY-MM-DD, empty to clear)",
                    Some(default.as_str()),
                    Some(validate_date_string),
                ) else {
                    return 0;
                };
                values.insert("contractStart".to_string(), ArgValue::Str(v));
            }
            "contractEnd" => {
                let default = sub.contract_end.clone().unwrap_or_default();
                let Some(v) = prompt::input(
                    "New contract end (YYYY-MM-DD, empty to clear)",
                    Some(default.as_str()),
                    Some(validate_date_string),
                ) else {
                    return 0;
                };
                values.insert("contractEnd".to_string(), ArgValue::Str(v));
            }
            "autoRenewal" => {
                let Some(v) = prompt::confirm("Auto renew?", sub.auto_renewal) else {
                    return 0;
                };
                values.insert(
                    "autoRenewal".to_string(),
                    ArgValue::Str(if v { "true" } else { "false" }.to_string()),
                );
            }
            "discount" => {
                let default = sub
                    .discount_amount
                    .map(|d| d.to_string())
                    .unwrap_or_default();
                let Some(amount) = prompt::input(
                    "New discount amount (empty to clear)",
                    Some(default.as_str()),
                    Some(validate_discount_value),
                ) else {
                    return 0;
                };
                values.insert("discountAmount".to_string(), ArgValue::Str(amount.clone()));
                if !amount.trim().is_empty() {
                    let type_default = sub
                        .discount_type
                        .clone()
                        .unwrap_or_else(|| "percentage".to_string());
                    let Some(kind) = prompt::input(
                        "Discount type (percentage or fixed)",
                        Some(type_default.as_str()),
                        Some(validate_discount_type),
                    ) else {
                        return 0;
                    };
                    values.insert("discountType".to_string(), ArgValue::Str(kind));
                } else {
                    values.insert("discountType".to_string(), ArgValue::Str(String::new()));
                }
            }
            _ => {}
        }
    }

    let synthetic = ParsedArgs {
        values,
        ..Default::default()
    };
    run_edit(&synthetic)
}

/// `trimmed || null` — empty string becomes SQL NULL.
fn opt_text_val(trimmed: &str) -> crate::db::SqlValue {
    if trimmed.is_empty() {
        crate::db::SqlValue::Null
    } else {
        crate::db::SqlValue::Text(trimmed.to_string())
    }
}
