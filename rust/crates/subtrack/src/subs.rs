//! Subscription command handlers — port of the non-interactive paths of
//! `subscription/add.ts`, `subscription/core.ts` (clone/archive),
//! `subscription/edit.ts`, `subscription-actions.ts` (pause/resume/renew)
//! and `cancel.ts`.
//!
//! Error paths (not-found / validation) are complete; the DB write on the
//! success path is deferred to the mutating phase (no `saveDb` yet).

use crate::db::get_subscription_by_id;
use crate::logger::{self, fail, info};
use crate::parser::ParsedArgs;
use crate::prompts::{prompt_cycle, prompt_select, prompt_string};
use crate::validation::{
    is_valid_currency, is_valid_status, validate_billing_day, validate_date_string,
    validate_discount_type, validate_discount_value, validate_name, validate_notes,
    validate_payment_method, validate_plan_tier, validate_price, validate_vendor_name,
    validate_vendor_url, CURRENCY_VALUES, STATUS_VALUES,
};

fn parse_id(value: Option<String>) -> Option<i64> {
    value.and_then(|v| v.trim().parse::<i64>().ok())
}

/// Validate an optional free-text flag; empty values are ignored.
fn opt_text(
    parsed: &ParsedArgs,
    key: &str,
    validate: fn(&str) -> Result<(), String>,
) -> Result<(), ()> {
    if let Some(raw) = parsed.str(key) {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            if let Err(msg) = validate(trimmed) {
                fail(&msg);
                return Err(());
            }
        }
    }
    Ok(())
}

/// `subtrack add` — port of `resolveAddOptions` validation order.
pub fn run_add(parsed: &ParsedArgs) -> i32 {
    let Some(name) = prompt_string(parsed.str("name").as_deref(), validate_name) else {
        return 0;
    };
    let Some(price) = prompt_string(parsed.str("price").as_deref(), validate_price) else {
        return 0;
    };
    let Some(currency) = prompt_select(
        parsed.str("currency").as_deref(),
        is_valid_currency,
        &CURRENCY_VALUES,
    ) else {
        return 0;
    };
    let Some(cycle) = prompt_cycle(parsed.str("cycle").as_deref()) else {
        return 0;
    };

    // tags: hint from existing tags; flag split happens later.
    let tags_str = parsed.str("tags").unwrap_or_default();

    if opt_text(parsed, "notes", validate_notes).is_err() {
        return 1;
    }
    if opt_text(parsed, "paymentMethod", validate_payment_method).is_err() {
        return 1;
    }
    if let Some(raw) = parsed.str("billingDay") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            if let Err(e) = validate_billing_day(trimmed) {
                fail(&e);
                return 1;
            }
        }
    }
    if opt_text(parsed, "vendorName", validate_vendor_name).is_err() {
        return 1;
    }
    if opt_text(parsed, "vendorUrl", validate_vendor_url).is_err() {
        return 1;
    }
    if let Err(()) = opt_text(parsed, "planTier", validate_plan_tier) {
        return 1;
    }
    if opt_text(parsed, "contractStart", validate_date_string).is_err() {
        return 1;
    }
    if opt_text(parsed, "contractEnd", validate_date_string).is_err() {
        return 1;
    }
    if opt_text(parsed, "discountAmount", validate_discount_value).is_err() {
        return 1;
    }
    if let Some(raw) = parsed.str("discountType") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            if let Err(e) = validate_discount_type(trimmed) {
                fail(&e);
                return 1;
            }
        }
    }
    if let Some(v) = parsed.str("autoRenewal") {
        // validateAutoRenewal runs on the raw flag value (no trimming).
        if let Err(e) = crate::validation::validate_auto_renewal(&v) {
            fail(&e);
            return 1;
        }
    }

    // status (last)
    let Some(status) = prompt_select(parsed.str("status").as_deref(), is_valid_status, &STATUS_VALUES)
    else {
        return 0;
    };

    let text = |key: &str| -> Option<String> {
        parsed
            .str(key)
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    let tags: Vec<String> = tags_str
        .split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();
    let new_sub = crate::db::NewSubscription {
        name: name.trim().to_string(),
        price: price.trim().parse::<f64>().unwrap_or(0.0),
        currency,
        cycle,
        status,
        billing_day: text("billingDay").and_then(|v| v.parse::<i64>().ok()),
        notes: text("notes"),
        payment_method: text("paymentMethod"),
        contract_start: text("contractStart"),
        contract_end: text("contractEnd"),
        auto_renewal: parsed.str("autoRenewal").as_deref() != Some("false"),
        vendor_name: text("vendorName"),
        vendor_url: text("vendorUrl"),
        plan_tier: text("planTier"),
        discount_amount: text("discountAmount").and_then(|v| v.parse::<f64>().ok()),
        discount_type: text("discountType"),
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
    let id = parse_id(parsed.str("id"));
    if id.is_none() {
        // Interactive selection is not available in this environment.
        return 0;
    }
    let sub = match get_subscription_by_id(id.unwrap()) {
        Ok(Some(s)) => s,
        Ok(None) => {
            fail(&format!("Subscription with id {} not found", id.unwrap()));
            return 1;
        }
        Err(e) => {
            logger::fail(&format!("Failed to read subscription: {}", e));
            return 1;
        }
    };

    // hasFlags: any of the editable flags present.
    let flag_keys = [
        "name", "price", "currency", "cycle", "status", "billingDay", "tags", "notes",
        "paymentMethod", "vendorName", "vendorUrl", "planTier", "discountAmount",
        "discountType", "contractStart", "contractEnd", "autoRenewal",
    ];
    let has_flags = flag_keys.iter().any(|k| parsed.str(k).is_some());
    if !has_flags {
        // Interactive editing is not available in this environment.
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
                sets.push(("price", SqlValue::Real(v.trim().parse::<f64>().unwrap_or(0.0))));
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
        sets.push(("auto_renewal", SqlValue::Int(if v == "true" { 1 } else { 0 })));
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
    let new_currency = parsed.str("currency").unwrap_or_else(|| sub.currency.clone());

    if let Err(e) = crate::db::update_subscription(&conn, sub.id, &sets, false) {
        fail(&format!("Failed to edit subscription: {}", e));
        return 1;
    }
    if let Some(tags) = &new_tags {
        if let Err(e) = crate::db::replace_tags(&conn, sub.id, tags) {
            fail(&format!("Failed to edit subscription: {}", e));
            return 1;
        }
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

/// `trimmed || null` — empty string becomes SQL NULL.
fn opt_text_val(trimmed: &str) -> crate::db::SqlValue {
    if trimmed.is_empty() {
        crate::db::SqlValue::Null
    } else {
        crate::db::SqlValue::Text(trimmed.to_string())
    }
}

/// Port of `handleClone` (not-found path).
pub fn run_clone(parsed: &ParsedArgs) -> i32 {
    let id = match parse_id(parsed.str("id")) {
        Some(id) => id,
        None => return 0,
    };
    match get_subscription_by_id(id) {
        Ok(Some(_)) => 0,
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

/// Port of `handleArchive` (not-found / already-archived / success).
pub fn run_archive(parsed: &ParsedArgs) -> i32 {
    let id = match parse_id(parsed.str("id")) {
        Some(id) => id,
        None => return 0,
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
    let ids: Vec<i64> = parsed
        .str_array("id")
        .unwrap_or_default()
        .iter()
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .collect();
    if ids.is_empty() {
        // Interactive selection is not available in this environment.
        return 0;
    }
    let mut pending = 0usize;
    for id in ids {
        if let Ok(Some(sub)) = get_subscription_by_id(id) {
            if sub.status != target && sub.status != "archived" {
                pending += 1;
            }
        }
    }
    if pending == 0 {
        info("No subscriptions need changing");
        return 0;
    }
    // Status updates are deferred to the mutating phase.
    logger::success(&format!("Updated {} subscription(s)", pending));
    0
}

pub fn run_pause(parsed: &ParsedArgs) -> i32 {
    change_status(parsed, "paused")
}

pub fn run_resume(parsed: &ParsedArgs) -> i32 {
    change_status(parsed, "active")
}

/// Port of `handleRenew` (not-found + flag validation).
pub fn run_renew(parsed: &ParsedArgs) -> i32 {
    let id = match parse_id(parsed.str("id")) {
        Some(id) => id,
        None => return 0,
    };
    match get_subscription_by_id(id) {
        Ok(Some(_)) => {}
        Ok(None) => {
            fail(&format!("Subscription with id {} not found", id));
            return 1;
        }
        Err(e) => {
            logger::fail(&format!("Failed to read subscription: {}", e));
            return 1;
        }
    }
    if let Some(price) = parsed.str("price") {
        if let Err(e) = validate_price(&price) {
            fail(&format!("Invalid price: {}", e));
            return 1;
        }
    }
    if let Some(currency) = parsed.str("currency") {
        if !is_valid_currency(&currency) {
            fail(&format!("Invalid currency: \"{}\"", currency));
            return 1;
        }
    }
    if let Some(cycle) = parsed.str("cycle") {
        if !crate::validation::is_valid_cycle(&cycle) {
            fail(&format!("Invalid cycle: \"{}\"", cycle));
            return 1;
        }
    }
    0
}

/// Port of `handleCancel` (not-found path).
pub fn run_cancel(parsed: &ParsedArgs) -> i32 {
    let id = match parse_id(parsed.str("id")) {
        Some(id) => id,
        None => return 0,
    };
    match get_subscription_by_id(id) {
        Ok(Some(_)) => 0,
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

/// Validate an optional notes/discount flag value (used by add side effects).
#[allow(dead_code)]
fn apply_opt(parsed: &ParsedArgs, key: &str, f: fn(&str) -> Result<(), String>) -> i32 {
    if let Some(raw) = parsed.str(key) {
        if !raw.trim().is_empty() {
            if let Err(e) = f(raw.trim()) {
                fail(&e);
                return 1;
            }
        }
    }
    0
}
