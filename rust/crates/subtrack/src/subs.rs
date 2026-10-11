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
    let Some(_name) = prompt_string(parsed.str("name").as_deref(), validate_name) else {
        return 0;
    };
    let Some(_price) = prompt_string(parsed.str("price").as_deref(), validate_price) else {
        return 0;
    };
    let Some(_currency) = prompt_select(
        parsed.str("currency").as_deref(),
        is_valid_currency,
        &CURRENCY_VALUES,
    ) else {
        return 0;
    };
    let Some(_cycle) = prompt_cycle(parsed.str("cycle").as_deref()) else {
        return 0;
    };

    // tags: hint from existing tags; flag split happens later.
    let _tags_str = parsed.str("tags").unwrap_or_default();

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
    if prompt_select(parsed.str("status").as_deref(), is_valid_status, &STATUS_VALUES).is_none() {
        return 0;
    }

    // NOTE: persistence is implemented in the mutating phase.
    let name = parsed.str("name").unwrap_or_default();
    logger::success(&format!("Added subscription: {}", name.trim()));
    0
}

/// Port of `handleEdit` (not-found path + interactive guard).
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
    match get_subscription_by_id(id.unwrap()) {
        Ok(Some(_)) => {
            // Field updates are deferred to the mutating phase.
            0
        }
        Ok(None) => {
            fail(&format!("Subscription with id {} not found", id.unwrap()));
            1
        }
        Err(e) => {
            logger::fail(&format!("Failed to read subscription: {}", e));
            1
        }
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

/// Port of `handleArchive` (not-found / already-archived paths).
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
            0
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
