//! `suggest` command (reads) — port of `suggest/suggest.ts`
//! (handleSuggestList / handleSuggestView).

use crate::ansi;
use crate::db::{get_suggestion, get_suggestions};
use crate::json::quote_string;
use crate::logger::{self, fail, info, log};
use crate::parser::ParsedArgs;
use crate::price::format_price;

pub fn run_suggest_list(parsed: &ParsedArgs) -> i32 {
    let all = parsed.bool("all") == Some(true);
    let json = parsed.bool("json") == Some(true);
    let status = if all { None } else { Some("pending") };
    let suggestions = match get_suggestions(status) {
        Ok(s) => s,
        Err(e) => {
            logger::fail(&format!("Failed to read suggestions: {}", e));
            return 1;
        }
    };

    if json {
        let mut out = String::from("[\n");
        for (i, s) in suggestions.iter().enumerate() {
            out.push_str("  {\n");
            out.push_str(&format!("    \"id\": {},\n", s.id));
            out.push_str(&format!("    \"name\": {},\n", quote_string(&s.name)));
            out.push_str(&format!(
                "    \"price\": {},\n",
                match s.price {
                    Some(p) => crate::json::number(p),
                    None => "null".to_string(),
                }
            ));
            out.push_str(&format!(
                "    \"currency\": {},\n",
                match &s.currency {
                    Some(c) => quote_string(c),
                    None => "null".to_string(),
                }
            ));
            out.push_str(&format!(
                "    \"cycle\": {},\n",
                match &s.cycle {
                    Some(c) => quote_string(c),
                    None => "null".to_string(),
                }
            ));
            out.push_str(&format!(
                "    \"vendorName\": {},\n",
                match &s.vendor_name {
                    Some(v) => quote_string(v),
                    None => "null".to_string(),
                }
            ));
            out.push_str(&format!(
                "    \"vendorUrl\": {},\n",
                match &s.vendor_url {
                    Some(v) => quote_string(v),
                    None => "null".to_string(),
                }
            ));
            out.push_str(&format!(
                "    \"planTier\": {},\n",
                match &s.plan_tier {
                    Some(v) => quote_string(v),
                    None => "null".to_string(),
                }
            ));
            out.push_str(&format!(
                "    \"paymentMethod\": {},\n",
                match &s.payment_method {
                    Some(v) => quote_string(v),
                    None => "null".to_string(),
                }
            ));
            out.push_str(&format!("    \"source\": {},\n", quote_string(&s.source)));
            out.push_str(&format!(
                "    \"sourceDetail\": {},\n",
                match &s.source_detail {
                    Some(v) => quote_string(v),
                    None => "null".to_string(),
                }
            ));
            out.push_str(&format!(
                "    \"emailSubject\": {},\n",
                match &s.email_subject {
                    Some(v) => quote_string(v),
                    None => "null".to_string(),
                }
            ));
            out.push_str(&format!(
                "    \"emailFrom\": {},\n",
                match &s.email_from {
                    Some(v) => quote_string(v),
                    None => "null".to_string(),
                }
            ));
            out.push_str(&format!(
                "    \"emailDate\": {},\n",
                match &s.email_date {
                    Some(v) => quote_string(v),
                    None => "null".to_string(),
                }
            ));
            out.push_str(&format!(
                "    \"confidence\": {},\n",
                crate::json::number(s.confidence)
            ));
            out.push_str(&format!("    \"status\": {},\n", quote_string(&s.status)));
            out.push_str(&format!(
                "    \"matchedSubId\": {},\n",
                match s.matched_sub_id {
                    Some(v) => v.to_string(),
                    None => "null".to_string(),
                }
            ));
            out.push_str(&format!(
                "    \"createdAt\": {}\n",
                match &s.created_at {
                    Some(v) => quote_string(v),
                    None => "null".to_string(),
                }
            ));
            out.push_str(if i + 1 == suggestions.len() {
                "  }\n"
            } else {
                "  },\n"
            });
        }
        out.push_str("]\n");
        logger::out_str(&out);
        return 0;
    }

    if suggestions.is_empty() {
        if all {
            info("No suggestions found.");
        } else {
            info("No pending suggestions.");
        }
        return 0;
    }

    log(&ansi::bold(&ansi::cyan(&format!(
        "── Suggestions ({}) ──",
        suggestions.len()
    ))));
    log("");
    for s in &suggestions {
        let price_str = match (s.price, &s.currency) {
            (Some(p), Some(c)) => format_price(p, c),
            _ => "N/A".to_string(),
        };
        let cycle_str = s.cycle.clone().unwrap_or_default();
        let status_tag = if s.status == "pending" {
            ansi::green("pending")
        } else if s.status == "dismissed" {
            ansi::dim("dismissed")
        } else {
            ansi::colored("blue", "added")
        };
        let cycle_part = if cycle_str.is_empty() {
            String::new()
        } else {
            ansi::dim(&format!("/{}", cycle_str))
        };
        log(&format!(
            "  {} {}  {}{}  {}  {}",
            ansi::cyan(&format!("#{}", s.id)),
            ansi::bold(&s.name),
            ansi::yellow(&price_str),
            cycle_part,
            status_tag,
            ansi::dim(s.source_detail.as_deref().unwrap_or("")),
        ));
    }
    0
}

pub fn run_suggest_view(parsed: &ParsedArgs) -> i32 {
    let id = parsed
        .str("id")
        .and_then(|i| i.parse::<i64>().ok())
        .unwrap_or(-1);
    let suggestion = match get_suggestion(id) {
        Ok(s) => s,
        Err(e) => {
            logger::fail(&format!("Failed to read suggestion: {}", e));
            return 1;
        }
    };
    let Some(suggestion) = suggestion else {
        fail(&format!("Suggestion #{} not found.", id));
        return 1;
    };

    let price_str = match (suggestion.price, &suggestion.currency) {
        (Some(p), Some(c)) => format_price(p, c),
        _ => "N/A".to_string(),
    };
    log(&ansi::bold(&ansi::cyan(&format!(
        "── Suggestion #{} ──",
        suggestion.id
    ))));
    log(&format!(
        "  {}       {}",
        ansi::bold("Name:"),
        suggestion.name
    ));
    log(&format!(
        "  {}      {}{}",
        ansi::bold("Price:"),
        price_str,
        suggestion
            .cycle
            .as_ref()
            .map(|c| ansi::dim(&format!("/{}", c)))
            .unwrap_or_default(),
    ));
    if let Some(v) = &suggestion.vendor_name {
        log(&format!("  {}     {}", ansi::bold("Vendor:"), v));
    }
    if let Some(v) = &suggestion.plan_tier {
        log(&format!("  {}       {}", ansi::bold("Plan:"), v));
    }
    if let Some(v) = &suggestion.payment_method {
        log(&format!("  {}    {}", ansi::bold("Payment:"), v));
    }
    log(&format!(
        "  {}     {}",
        ansi::bold("Source:"),
        suggestion
            .source_detail
            .as_deref()
            .unwrap_or(&suggestion.source)
    ));
    log(&format!(
        "  {} {}%",
        ansi::bold("Confidence:"),
        (suggestion.confidence * 100.0).round() as i64
    ));
    log(&format!(
        "  {}     {}",
        ansi::bold("Status:"),
        suggestion.status
    ));
    if let Some(m) = suggestion.matched_sub_id {
        log(&format!(
            "  {}   subscription #{}",
            ansi::bold("Added as:"),
            m
        ));
    }
    if let Some(c) = &suggestion.created_at {
        log(&format!("  {}   {}", ansi::bold("Created:"), c));
    }
    0
}

/// Port of `handleSuggestAdd` (non-interactive).
pub fn run_suggest_add(parsed: &ParsedArgs) -> i32 {
    let id = match parsed.str("id").and_then(|s| s.trim().parse::<i64>().ok()) {
        Some(id) => id,
        None => {
            logger::fail("Invalid suggestion ID");
            return 1;
        }
    };
    match crate::db::get_suggestion(id) {
        Ok(Some(s)) => {
            if s.status != "pending" {
                logger::fail(&format!("Suggestion #{} is already {}.", id, s.status));
                return 1;
            }
            // Port of the success path of `handleSuggestAdd`: create a
            // subscription from the suggestion, then link it back.
            let new = crate::db::NewSubscription {
                name: s.name.clone(),
                price: s.price.unwrap_or(0.0),
                currency: s.currency.clone().unwrap_or_else(|| "USD".to_string()),
                cycle: s.cycle.clone().unwrap_or_else(|| "monthly".to_string()),
                status: "active".to_string(),
                billing_day: None,
                notes: None,
                payment_method: s.payment_method.clone(),
                contract_start: None,
                contract_end: None,
                auto_renewal: true,
                vendor_name: s.vendor_name.clone(),
                vendor_url: None,
                plan_tier: s.plan_tier.clone(),
                discount_amount: None,
                discount_type: None,
                tags: Vec::new(),
            };
            let conn = match crate::db::open_db() {
                Ok(c) => c,
                Err(e) => {
                    logger::fail(&format!("Failed to open database: {}", e));
                    return 1;
                }
            };
            let sub_id = match crate::db::write_subscription(&conn, &new, false) {
                Ok(id) => id,
                Err(e) => {
                    logger::fail(&format!("Failed to add subscription: {}", e));
                    return 1;
                }
            };
            if let Err(e) = crate::db::mark_suggestion_added(&conn, id, sub_id) {
                logger::fail(&format!("Failed to update suggestion: {}", e));
                return 1;
            }
            if let Err(e) = crate::db::save_db(&conn) {
                logger::fail(&format!("Failed to save subscription: {}", e));
                return 1;
            }
            logger::success(&format!(
                "Added \"{}\" as subscription #{}.",
                s.name, sub_id
            ));
            0
        }
        Ok(None) => {
            logger::fail(&format!("Suggestion #{} not found.", id));
            1
        }
        Err(e) => {
            logger::fail(&format!("Failed to read suggestion: {}", e));
            1
        }
    }
}

/// Port of `handleSuggestDismiss`.
pub fn run_suggest_dismiss(parsed: &ParsedArgs) -> i32 {
    if parsed.bool("all") == Some(true) {
        let count = match crate::db::get_suggestions(Some("pending")) {
            Ok(v) => v.len(),
            Err(e) => {
                logger::fail(&format!("Failed to dismiss suggestions: {}", e));
                return 1;
            }
        };
        if count == 0 {
            logger::info("No pending suggestions to dismiss.");
            return 0;
        }
        if let Err(e) = crate::db::dismiss_all_suggestions() {
            logger::fail(&format!("Failed to dismiss suggestions: {}", e));
            return 1;
        }
        logger::success(&format!(
            "{} suggestion{} dismissed.",
            count,
            if count > 1 { "s" } else { "" }
        ));
        return 0;
    }
    let Some(id) = parsed.str("id").and_then(|s| s.trim().parse::<i64>().ok()) else {
        logger::fail("Specify a suggestion ID or use --all");
        return 1;
    };
    match crate::db::dismiss_suggestion(id) {
        Ok(true) => {
            logger::success(&format!("Suggestion #{} dismissed.", id));
            0
        }
        Ok(false) => {
            logger::fail(&format!(
                "Suggestion #{} not found or already processed.",
                id
            ));
            1
        }
        Err(e) => {
            logger::fail(&format!("Failed to dismiss suggestion: {}", e));
            1
        }
    }
}
