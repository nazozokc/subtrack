use crate::logger::{self, fail, info};
use crate::parser::ParsedArgs;
use crate::prompts::{
    prompt_currency, prompt_cycle, prompt_optional, prompt_select, prompt_string,
};
use crate::validation::{
    STATUS_VALUES, is_valid_status, validate_billing_day, validate_date_string,
    validate_discount_type, validate_discount_value, validate_name, validate_notes,
    validate_payment_method, validate_plan_tier, validate_price, validate_tags,
    validate_vendor_name, validate_vendor_url,
};

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
