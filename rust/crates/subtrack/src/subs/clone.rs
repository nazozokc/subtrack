use super::{not_archived, resolve_id};
use crate::db::get_subscription_by_id;
use crate::logger::{self, fail};
use crate::parser::ParsedArgs;
use crate::validation::{is_valid_currency, validate_price};

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
