//! Subscription command handlers — port of the non-interactive paths of
//! `subscription/add.ts`, `subscription/core.ts` (clone/archive),
//! `subscription/edit.ts`, `subscription-actions.ts` (pause/resume/renew)
//! and `cancel.ts`.
//!
//! Error paths (not-found / validation) and the DB write on the success path
//! are both implemented (`write_subscription` / `update_subscription` +
//! `save_db`).

use crate::db::get_subscription_by_id;
use crate::logger::{self, fail, info};
use crate::parser::ParsedArgs;
use crate::prompts::{
    prompt_currency, prompt_cycle, prompt_optional, prompt_select, prompt_string,
};
use crate::validation::{
    STATUS_VALUES, is_valid_currency, is_valid_status, validate_billing_day, validate_date_string,
    validate_discount_type, validate_discount_value, validate_name, validate_notes,
    validate_payment_method, validate_plan_tier, validate_price, validate_tags, validate_vendor_name,
    validate_vendor_url,
};

fn parse_id(value: Option<String>) -> Option<i64> {
    value.and_then(|v| v.trim().parse::<i64>().ok())
}

/// Resolve an id from the positional/flag, or prompt for a subscription.
fn resolve_id(
    parsed: &ParsedArgs,
    message: &str,
    filter: fn(&crate::db::Subscription) -> bool,
) -> Option<i64> {
    if let Some(id) = parse_id(parsed.str("id")) {
        return Some(id);
    }
    let all = crate::db::get_subscriptions(&crate::db::QueryOptions {
        include_archived: true,
        ..Default::default()
    })
    .ok()?;
    let filtered: Vec<_> = all.into_iter().filter(|s| filter(s)).collect();
    if filtered.is_empty() {
        info("No matching subscriptions found");
        return None;
    }
    let choices: Vec<crate::prompt::Choice> = filtered
        .iter()
        .map(|s| {
            crate::prompt::Choice::new(
                &format!(
                    "#{} {} — {}/{}{}",
                    s.id,
                    s.name,
                    crate::price::format_price(s.price, &s.currency),
                    s.cycle,
                    if s.status == "active" {
                        String::new()
                    } else {
                        format!(" ({})", s.status)
                    }
                ),
                &s.id.to_string(),
            )
        })
        .collect();
    crate::prompt::select(message, &choices, None)?.parse::<i64>().ok()
}

fn any_sub(_: &crate::db::Subscription) -> bool {
    true
}

fn not_cancelled(sub: &crate::db::Subscription) -> bool {
    sub.status != "cancelled"
}

fn archived(sub: &crate::db::Subscription) -> bool {
    sub.status == "archived"
}

fn not_archived(sub: &crate::db::Subscription) -> bool {
    sub.status != "archived"
}

/// Validate an optional free-text flag; empty values are ignored.
fn opt_text(
    parsed: &ParsedArgs,
    key: &str,
    validate: fn(&str) -> Result<(), String>,
) -> Result<(), ()> {
    if let Some(raw) = parsed.str(key) {
        let trimmed = raw.trim();
        if !trimmed.is_empty()
            && let Err(msg) = validate(trimmed)
        {
            fail(&msg);
            return Err(());
        }
    }
    Ok(())
}

/// `subtrack add` — port of `resolveAddOptions` (flags + interactive fallback).
pub fn run_add(parsed: &ParsedArgs) -> i32 {
    let prompted = ["name", "price", "currency", "cycle"]
        .iter()
        .any(|key| parsed.str(key).is_none());

    let Some(name) = prompt_string(
        parsed.str("name").as_deref(),
        "subscription name",
        validate_name,
    ) else {
        return 0;
    };
    let Some(price) = prompt_string(
        parsed.str("price").as_deref(),
        "monthly payment amount",
        validate_price,
    ) else {
        return 0;
    };
    let Some(currency) = prompt_currency(parsed.str("currency").as_deref(), "currency") else {
        return 0;
    };
    let Some(cycle) = prompt_cycle(parsed.str("cycle").as_deref(), "cycle") else {
        return 0;
    };

    // tags: hint from existing tags during the interactive flow.
    let tags: Vec<String> = match parsed.str("tags") {
        Some(raw) => raw
            .split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect(),
        None if prompted => {
            let existing: Vec<String> = crate::db::get_tags_with_count()
                .unwrap_or_default()
                .into_iter()
                .map(|(name, _)| name)
                .collect();
            let message = if existing.is_empty() {
                "tags (comma-separated, optional)".to_string()
            } else {
                format!(
                    "tags (comma-separated, optional; existing: {})",
                    existing.join(", ")
                )
            };
            match crate::prompt::input(&message, Some(""), Some(validate_tags)) {
                Some(raw) => raw
                    .split(',')
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect(),
                None => return 0,
            }
        }
        None => Vec::new(),
    };

    let Some(notes) = prompt_optional(
        parsed.str("notes").as_deref(),
        "notes (optional, max 500 chars)",
        validate_notes,
    ) else {
        return 0;
    };
    let Some(payment_method) = prompt_optional(
        parsed.str("paymentMethod").as_deref(),
        "payment method (optional, e.g. credit_card, paypal)",
        validate_payment_method,
    ) else {
        return 0;
    };
    let Some(billing_day) = prompt_optional(
        parsed.str("billingDay").as_deref(),
        "billing day (1-31, optional)",
        validate_billing_day,
    ) else {
        return 0;
    };
    let Some(vendor_name) = prompt_optional(
        parsed.str("vendorName").as_deref(),
        "vendor name (optional, max 100 chars)",
        validate_vendor_name,
    ) else {
        return 0;
    };
    let Some(vendor_url) = prompt_optional(
        parsed.str("vendorUrl").as_deref(),
        "vendor URL (optional)",
        validate_vendor_url,
    ) else {
        return 0;
    };
    let Some(plan_tier) = prompt_optional(
        parsed.str("planTier").as_deref(),
        "plan tier (optional, max 100 chars)",
        validate_plan_tier,
    ) else {
        return 0;
    };
    let Some(contract_start) = prompt_optional(
        parsed.str("contractStart").as_deref(),
        "contract start (YYYY-MM-DD, optional)",
        validate_date_string,
    ) else {
        return 0;
    };
    let Some(contract_end) = prompt_optional(
        parsed.str("contractEnd").as_deref(),
        "contract end (YYYY-MM-DD, optional)",
        validate_date_string,
    ) else {
        return 0;
    };
    let Some(discount_amount) = prompt_optional(
        parsed.str("discountAmount").as_deref(),
        "discount amount (optional, non-negative integer)",
        validate_discount_value,
    ) else {
        return 0;
    };
    let discount_type = if discount_amount.is_some() {
        match prompt_optional(
            parsed.str("discountType").as_deref(),
            "discount type (percentage or fixed, optional)",
            validate_discount_type,
        ) {
            Some(v) => v,
            None => return 0,
        }
    } else {
        match parsed.str("discountType") {
            Some(raw) => {
                let trimmed = raw.trim();
                if !trimmed.is_empty()
                    && let Err(e) = validate_discount_type(trimmed)
                {
                    fail(&e);
                    return 1;
                }
                Some(trimmed.to_string())
            }
            None => None,
        }
    };

    let auto_renewal = match parsed.str("autoRenewal") {
        // validateAutoRenewal runs on the raw flag value (no trimming).
        Some(v) => {
            if let Err(e) = crate::validation::validate_auto_renewal(&v) {
                fail(&e);
                return 1;
            }
            v == "true"
        }
        None if prompted => match crate::prompt::confirm("auto renew?", true) {
            Some(v) => v,
            None => return 0,
        },
        None => true,
    };

    // status (last)
    let status_prompted = parsed.str("status").is_none();
    let Some(status) = prompt_select(
        parsed.str("status").as_deref(),
        "status",
        &STATUS_VALUES,
        is_valid_status,
    ) else {
        return 0;
    };

    if (prompted || status_prompted)
        && crate::prompt::confirm(
            &format!(
                "Save \"{}\" ({} {}, {}{})?",
                name.trim(),
                price.trim(),
                currency,
                cycle,
                if status != "active" {
                    format!(", status: {}", status)
                } else {
                    String::new()
                }
            ),
            true,
        ) != Some(true)
    {
        info("Cancelled");
        return 0;
    }

    let new_sub = crate::db::NewSubscription {
        name: name.trim().to_string(),
        price: price.trim().parse::<f64>().unwrap_or(0.0),
        currency,
        cycle,
        status,
        billing_day: billing_day.and_then(|v| v.trim().parse::<i64>().ok()),
        notes,
        payment_method,
        contract_start,
        contract_end,
        auto_renewal,
        vendor_name,
        vendor_url,
        plan_tier,
        discount_amount: discount_amount.and_then(|v| v.trim().parse::<f64>().ok()),
        discount_type,
        tags,
    };

    let conn = match crate::db::open_db() {
        Ok(c) => c,
        Err(e) => {
            fail(&format!("Failed to add subscription: {}", e));
            return 1;
        }
    };
    match crate::db::write_subscription(&conn, &new_sub, true) {
        Ok(id) => {
            let details = format!(
                "{} — {}/{}",
                new_sub.name,
                crate::price::format_price(new_sub.price, &new_sub.currency),
                new_sub.cycle
            );
            let _ = crate::db::add_audit_log(
                &conn,
                "subscription.add",
                Some("subscription"),
                Some(id),
                Some(&details),
                true,
            );
            logger::success(&format!("Added subscription: {}", new_sub.name));
            0
        }
        Err(e) => {
            fail(&format!("Failed to add subscription: {}", e));
            1
        }
    }
}

/// Port of `handleEdit`.
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
            &format!(
                "notes ({})",
                sub.notes.as_deref().unwrap_or("not set")
            ),
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
            &format!("auto renew ({})", if sub.auto_renewal { "yes" } else { "no" }),
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
                let Some(v) = prompt::input(
                    "New name",
                    Some(sub.name.as_str()),
                    Some(validate_name),
                ) else {
                    return 0;
                };
                values.insert("name".to_string(), ArgValue::Str(v));
            }
            "price" => {
                let default = format!("{}", sub.price);
                let Some(v) = prompt::input(
                    "New price",
                    Some(default.as_str()),
                    Some(validate_price),
                ) else {
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
                let entries: Vec<Choice> =
                    STATUS_VALUES.iter().map(|v| Choice::plain(v)).collect();
                let default = entries.iter().position(|c| c.value == sub.status);
                let Some(v) = prompt::select("New status", &entries, default) else {
                    return 0;
                };
                values.insert("status".to_string(), ArgValue::Str(v));
            }
            "billingDay" => {
                let default = sub
                    .billing_day
                    .map(|d| d.to_string())
                    .unwrap_or_default();
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

/// Port of `handleClone` (not-found path).
/// Port of `handleDelete` — flag/id path (no interactive selection).
pub fn run_delete(parsed: &ParsedArgs) -> i32 {
    let mut ids: Vec<i64> = parsed
        .str_array("id")
        .unwrap_or_default()
        .iter()
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .collect();
    if ids.is_empty() {
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
        let choices: Vec<crate::prompt::Choice> = all
            .iter()
            .map(|s| {
                crate::prompt::Choice::new(
                    &format!(
                        "{} — {}/{}{}",
                        s.name,
                        crate::price::format_price(s.price, &s.currency),
                        s.cycle,
                        if s.tags.is_empty() {
                            String::new()
                        } else {
                            format!(" [{}]", s.tags.join(", "))
                        }
                    ),
                    &s.id.to_string(),
                )
            })
            .collect();
        let Some(selected) = crate::prompt::checkbox("select subscriptions to delete", &choices)
        else {
            return 0;
        };
        if selected.is_empty() {
            info("Cancelled");
            return 0;
        }
        ids = selected
            .iter()
            .filter_map(|v| v.trim().parse::<i64>().ok())
            .collect();
        let names: Vec<String> = all
            .iter()
            .filter(|s| ids.contains(&s.id))
            .map(|s| s.name.clone())
            .collect();
        if crate::prompt::confirm(
            &format!(
                "Delete {} subscription{}? ({})",
                names.len(),
                if names.len() > 1 { "s" } else { "" },
                names.join(", ")
            ),
            false,
        ) != Some(true)
        {
            info("Cancelled");
            return 0;
        }
    }
    let conn = match crate::db::open_db() {
        Ok(c) => c,
        Err(e) => {
            logger::fail(&format!("Failed to delete: {}", e));
            return 1;
        }
    };
    for id in ids {
        match get_subscription_by_id(id) {
            Ok(Some(sub)) => {
                if let Err(e) = crate::db::remove_subscription(&conn, id, true) {
                    logger::fail(&format!("Failed to delete: {}", e));
                    continue;
                }
                let _ = crate::db::add_audit_log(
                    &conn,
                    "subscription.delete",
                    Some("subscription"),
                    Some(id),
                    Some(&sub.name),
                    true,
                );
                logger::success(&format!("Deleted: {}", sub.name));
            }
            Ok(None) => {
                fail(&format!("Subscription with id {} not found", id));
            }
            Err(e) => {
                logger::fail(&format!("Failed to read subscription: {}", e));
            }
        }
    }
    0
}

/// Port of `handleClone`.
pub fn run_clone(parsed: &ParsedArgs) -> i32 {
    let Some(id) = resolve_id(parsed, "select subscription to clone", not_archived) else {
        return 0;
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

    let new_name = parsed
        .str("name")
        .unwrap_or_else(|| format!("{} (copy)", sub.name));
    let mut price = sub.price;
    if let Some(v) = parsed.str("price") {
        if let Err(e) = validate_price(&v) {
            fail(&format!("Invalid price: {}", e));
            return 1;
        }
        price = v.trim().parse::<f64>().unwrap_or(sub.price);
    }
    let mut currency = sub.currency.clone();
    if let Some(v) = parsed.str("currency") {
        if !is_valid_currency(&v) {
            fail(&format!("Invalid currency: \"{}\"", v));
            return 1;
        }
        currency = v;
    }
    let mut cycle = sub.cycle.clone();
    if let Some(v) = parsed.str("cycle") {
        if !crate::validation::is_valid_cycle(&v) {
            fail(&format!("Invalid cycle: \"{}\"", v));
            return 1;
        }
        cycle = v;
    }
    let tags = match parsed.str("tags") {
        Some(v) => v
            .split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>(),
        None => sub.tags.clone(),
    };

    let conn = match crate::db::open_db() {
        Ok(c) => c,
        Err(e) => {
            fail(&format!("Failed to clone subscription: {}", e));
            return 1;
        }
    };
    let new = crate::db::NewSubscription {
        name: new_name.clone(),
        price,
        currency,
        cycle,
        status: sub.status.clone(),
        billing_day: sub.billing_day,
        notes: sub.notes.clone(),
        payment_method: sub.payment_method.clone(),
        contract_start: sub.contract_start.clone(),
        contract_end: sub.contract_end.clone(),
        auto_renewal: sub.auto_renewal,
        vendor_name: sub.vendor_name.clone(),
        vendor_url: sub.vendor_url.clone(),
        plan_tier: sub.plan_tier.clone(),
        discount_amount: sub.discount_amount,
        discount_type: sub.discount_type.clone(),
        tags,
    };
    match crate::db::write_subscription(&conn, &new, true) {
        Ok(new_id) => {
            let _ = crate::db::add_audit_log(
                &conn,
                "subscription.clone",
                Some("subscription"),
                Some(new_id),
                Some(&format!(
                    "Cloned from #{} \"{}\" → \"{}\"",
                    id, sub.name, new_name
                )),
                true,
            );
            logger::success(&format!(
                "Cloned: \"{}\" → \"{}\" (id={})",
                sub.name, new_name, new_id
            ));
            0
        }
        Err(e) => {
            fail(&format!("Failed to clone subscription: {}", e));
            1
        }
    }
}

/// Port of `handleUnarchive`.
pub fn run_unarchive(parsed: &ParsedArgs) -> i32 {
    let Some(id) = resolve_id(parsed, "select subscription to unarchive", archived) else {
        return 0;
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
    if sub.status != "archived" {
        info(&format!(
            "\"{}\" is not archived (status: {})",
            sub.name, sub.status
        ));
        return 0;
    }
    let conn = match crate::db::open_db() {
        Ok(c) => c,
        Err(e) => {
            logger::fail(&format!("Failed to unarchive: {}", e));
            return 1;
        }
    };
    match crate::db::unarchive_subscription(&conn, id, true) {
        Ok(true) => {
            let _ = crate::db::add_audit_log(
                &conn,
                "subscription.unarchive",
                Some("subscription"),
                Some(id),
                Some(&sub.name),
                true,
            );
            logger::success(&format!("Unarchived: \"{}\"", sub.name));
            0
        }
        Ok(false) => 0,
        Err(e) => {
            logger::fail(&format!("Failed to unarchive: {}", e));
            1
        }
    }
}

/// Port of `handleArchive` (not-found / already-archived / success).
pub fn run_archive(parsed: &ParsedArgs) -> i32 {
    let Some(id) = resolve_id(parsed, "select subscription to archive", not_archived) else {
        return 0;
    };
    match get_subscription_by_id(id) {
        Ok(Some(sub)) => {
            if sub.status == "archived" {
                info(&format!("\"{}\" is already archived", sub.name));
                return 0;
            }
            let conn = match crate::db::open_db() {
                Ok(c) => c,
                Err(e) => {
                    fail(&format!("Failed to archive subscription: {}", e));
                    return 1;
                }
            };
            match crate::db::archive_subscription(&conn, id, true) {
                Ok(true) => {
                    let _ = crate::db::add_audit_log(
                        &conn,
                        "subscription.archive",
                        Some("subscription"),
                        Some(id),
                        Some(&sub.name),
                        true,
                    );
                    logger::success(&format!("Archived: \"{}\"", sub.name));
                    0
                }
                Ok(false) => 0,
                Err(e) => {
                    fail(&format!("Failed to archive subscription: {}", e));
                    1
                }
            }
        }
        Ok(None) => {
            fail(&format!("Subscription with id {} not found", id));
            1
        }
        Err(e) => {
            logger::fail(&format!("Failed to read subscription: {}", e));
            1
        }
    }
}

/// `pause` / `resume` — port of `changeStatus`.
fn change_status(parsed: &ParsedArgs, target: &str) -> i32 {
    let mut ids: Vec<i64> = parsed
        .str_array("id")
        .unwrap_or_default()
        .iter()
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .collect();
    if ids.is_empty() {
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
        let choices: Vec<crate::prompt::Choice> = all
            .iter()
            .filter(|s| s.status != target && s.status != "archived")
            .map(|s| {
                crate::prompt::Choice::new(
                    &format!(
                        "#{} {} — {}/{}",
                        s.id,
                        s.name,
                        crate::price::format_price(s.price, &s.currency),
                        s.cycle
                    ),
                    &s.id.to_string(),
                )
            })
            .collect();
        if choices.is_empty() {
            info("No subscriptions need changing");
            return 0;
        }
        let Some(selected) =
            crate::prompt::checkbox(&format!("select subscriptions to {}", target), &choices)
        else {
            return 0;
        };
        if selected.is_empty() {
            info("Cancelled");
            return 0;
        }
        ids = selected
            .iter()
            .filter_map(|v| v.trim().parse::<i64>().ok())
            .collect();
    }
    let force = parsed.bool("force").unwrap_or(false);
    let mut pending: Vec<i64> = Vec::new();
    for id in &ids {
        if let Ok(Some(sub)) = get_subscription_by_id(*id)
            && sub.status != target
            && sub.status != "archived"
        {
            pending.push(*id);
        }
    }
    if pending.is_empty() {
        info("No subscriptions need changing");
        return 0;
    }
    if !force {
        if crate::prompt::confirm(
            &format!(
                "Update {} subscription{} to \"{}\"?",
                pending.len(),
                if pending.len() > 1 { "s" } else { "" },
                target
            ),
            false,
        ) != Some(true)
        {
            info("Cancelled");
            return 0;
        }
    }
    let conn = match crate::db::open_db() {
        Ok(c) => c,
        Err(e) => {
            logger::fail(&format!("Failed to update: {}", e));
            return 1;
        }
    };
    for id in &pending {
        let name = get_subscription_by_id(*id)
            .ok()
            .flatten()
            .map(|s| s.name)
            .unwrap_or_default();
        if let Err(e) = crate::db::update_subscription(
            &conn,
            *id,
            &[("status", crate::db::SqlValue::Text(target.to_string()))],
            true,
        ) {
            fail(&format!("Failed to update: {}", e));
            return 1;
        }
        let _ = crate::db::add_audit_log(
            &conn,
            if target == "active" {
                "subscription.resume"
            } else {
                "subscription.pause"
            },
            Some("subscription"),
            Some(*id),
            Some(&name),
            true,
        );
    }
    logger::success(&format!("Updated {} subscription(s)", pending.len()));
    0
}

pub fn run_pause(parsed: &ParsedArgs) -> i32 {
    change_status(parsed, "paused")
}

pub fn run_resume(parsed: &ParsedArgs) -> i32 {
    change_status(parsed, "active")
}

/// Port of `handleRenew`.
pub fn run_renew(parsed: &ParsedArgs) -> i32 {
    let Some(id) = resolve_id(parsed, "select subscription to renew", not_cancelled) else {
        return 0;
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
    use crate::db::SqlValue;
    let mut sets: Vec<(&str, SqlValue)> = vec![("status", SqlValue::Text("active".to_string()))];
    if let Some(price) = parsed.str("price") {
        if let Err(e) = validate_price(&price) {
            fail(&format!("Invalid price: {}", e));
            return 1;
        }
        sets.push((
            "price",
            SqlValue::Real(price.trim().parse::<f64>().unwrap_or(sub.price)),
        ));
    }
    if let Some(currency) = parsed.str("currency") {
        if !is_valid_currency(&currency) {
            fail(&format!("Invalid currency: \"{}\"", currency));
            return 1;
        }
        sets.push(("currency", SqlValue::Text(currency)));
    }
    if let Some(cycle) = parsed.str("cycle") {
        if !crate::validation::is_valid_cycle(&cycle) {
            fail(&format!("Invalid cycle: \"{}\"", cycle));
            return 1;
        }
        sets.push(("cycle", SqlValue::Text(cycle)));
    }
    if let Some(contract_end) = parsed.str("contractEnd") {
        let trimmed = contract_end.trim();
        if trimmed.is_empty() {
            sets.push(("contract_end", SqlValue::Null));
        } else {
            if let Err(e) = validate_date_string(trimmed) {
                fail(&format!("Invalid contract end: {}", e));
                return 1;
            }
            sets.push(("contract_end", SqlValue::Text(trimmed.to_string())));
        }
    }
    if let Some(plan_tier) = parsed.str("planTier") {
        let trimmed = plan_tier.trim();
        if trimmed.is_empty() {
            sets.push(("plan_tier", SqlValue::Null));
        } else {
            if let Err(e) = validate_plan_tier(trimmed) {
                fail(&format!("Invalid plan tier: {}", e));
                return 1;
            }
            sets.push(("plan_tier", SqlValue::Text(trimmed.to_string())));
        }
    }
    if let Some(v) = parsed.str("autoRenewal") {
        sets.push((
            "auto_renewal",
            SqlValue::Int(if v == "true" { 1 } else { 0 }),
        ));
    }

    let conn = match crate::db::open_db() {
        Ok(c) => c,
        Err(e) => {
            fail(&format!("Failed to renew subscription: {}", e));
            return 1;
        }
    };
    match crate::db::update_subscription(&conn, id, &sets, false) {
        Ok(true) => {
            let new_price = parsed
                .str("price")
                .and_then(|v| v.trim().parse::<f64>().ok())
                .unwrap_or(sub.price);
            let new_currency = parsed
                .str("currency")
                .unwrap_or_else(|| sub.currency.clone());
            let _ = crate::db::write_price_history(
                &conn,
                id,
                sub.price,
                new_price,
                &sub.currency,
                &new_currency,
            );
            let _ = crate::db::add_audit_log(
                &conn,
                "subscription.renew",
                Some("subscription"),
                Some(id),
                Some(&format!("{} renewed", sub.name)),
                false,
            );
            let _ = crate::db::save_db(&conn);
            logger::success(&format!("Renewed: {}", sub.name));
            0
        }
        Ok(false) => {
            fail(&format!("Subscription with id {} not found", id));
            1
        }
        Err(e) => {
            fail(&format!("Failed to renew subscription: {}", e));
            1
        }
    }
}

/// Port of `handleCancel`.
pub fn run_cancel(parsed: &ParsedArgs) -> i32 {
    let Some(id) = resolve_id(parsed, "select subscription to cancel", not_cancelled) else {
        return 0;
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
    if sub.status == "cancelled" {
        info(&format!("\"{}\" is already cancelled", sub.name));
        return 0;
    }
    if sub.status == "archived" {
        info(&format!(
            "\"{}\" is archived — unarchive it first: subtrack unarchive {}",
            sub.name, id
        ));
        return 0;
    }

    let next_billing = crate::date::calculate_next_billing(
        sub.billing_day,
        &sub.created_at,
        &sub.cycle,
        crate::date::today(),
    );
    let cancellation_date = crate::date::today_string();
    let force = parsed.bool("force").unwrap_or(false);
    let as_json = parsed.bool("json").unwrap_or(false);

    if as_json {
        let out = format!(
            "{{\n  \"id\": {},\n  \"name\": {},\n  \"price\": {},\n  \"currency\": {},\n  \"cycle\": {},\n  \"status\": {},\n  \"nextBilling\": {},\n  \"cancellationDate\": {}\n}}\n",
            sub.id,
            crate::json::quote_string(&sub.name),
            crate::json::number(sub.price),
            crate::json::quote_string(&sub.currency),
            crate::json::quote_string(&sub.cycle),
            crate::json::quote_string(&sub.status),
            crate::json::quote_string(&crate::date::format_date(next_billing)),
            crate::json::quote_string(&cancellation_date),
        );
        crate::logger::out_str(&out);
        return 0;
    }

    crate::logger::log(&crate::ansi::bold(&format!("Cancelling: {}", sub.name)));
    crate::logger::log(&format!(
        "  Price:        {}/{}",
        crate::price::format_price(sub.price, &sub.currency),
        sub.cycle
    ));
    let cfg = crate::config::load_config();
    let fmt = if cfg.date_format == "short" {
        crate::date::format_short_date(next_billing)
    } else {
        crate::date::format_date(next_billing)
    };
    crate::logger::log(&format!("  Next billing: {}", fmt));
    crate::logger::log("");

    if !force {
        crate::logger::warn("You may want to review alternatives before cancelling");
        if crate::prompt::confirm(&format!("Cancel \"{}\"?", sub.name), false) != Some(true) {
            info("Aborted — no changes made");
            return 0;
        }
    }

    let conn = match crate::db::open_db() {
        Ok(c) => c,
        Err(e) => {
            fail(&format!("Failed to cancel subscription: {}", e));
            return 1;
        }
    };
    let mut sets: Vec<(&str, crate::db::SqlValue)> =
        vec![("status", crate::db::SqlValue::Text("cancelled".to_string()))];
    if sub.contract_end.is_none() {
        sets.push((
            "contract_end",
            crate::db::SqlValue::Text(cancellation_date.clone()),
        ));
    }
    if let Err(e) = crate::db::update_subscription(&conn, id, &sets, false) {
        fail(&format!("Failed to cancel subscription: {}", e));
        return 1;
    }
    let _ = crate::db::add_audit_log(
        &conn,
        "subscription.cancel",
        Some("subscription"),
        Some(id),
        Some(&sub.name),
        false,
    );
    let _ = crate::db::save_db(&conn);
    logger::success(&format!("Cancelled: \"{}\"", sub.name));
    info(&format!(
        "Remove it permanently with: subtrack delete {}",
        id
    ));
    0
}

/// Validate an optional notes/discount flag value (used by add side effects).
#[allow(dead_code)]
fn apply_opt(parsed: &ParsedArgs, key: &str, f: fn(&str) -> Result<(), String>) -> i32 {
    if let Some(raw) = parsed.str(key)
        && !raw.trim().is_empty()
        && let Err(e) = f(raw.trim())
    {
        fail(&e);
        return 1;
    }
    0
}
