use super::{not_cancelled, resolve_id};
use crate::db::get_subscription_by_id;
use crate::logger::{self, fail, info};
use crate::parser::ParsedArgs;

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
