use super::{archived, not_archived, not_cancelled, resolve_id};
use crate::db::get_subscription_by_id;
use crate::logger::{self, fail, info};
use crate::parser::ParsedArgs;
use crate::validation::{
    is_valid_currency, validate_date_string, validate_plan_tier, validate_price,
};

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
    if !force
        && crate::prompt::confirm(
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
