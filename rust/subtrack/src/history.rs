//! `history` command — port of `history.ts` (handleHistory).

use crate::ansi;
use crate::db::{
    PriceChange, get_all_price_changes, get_price_changes_recent, get_price_history,
    get_subscription_by_id,
};
use crate::json::quote_string;
use crate::logger::{self, fail, info, log};
use crate::parser::ParsedArgs;
use crate::price::format_price;

/// formatDiff — port of `history.ts`.
pub fn format_diff(
    old_price: Option<f64>,
    new_price: f64,
    old_currency: Option<&str>,
    new_currency: &str,
) -> String {
    let old_str = match old_price {
        Some(p) => format_price(p, old_currency.unwrap_or(new_currency)),
        None => "—".to_string(),
    };
    let new_str = format_price(new_price, new_currency);
    if let Some(oc) = old_currency
        && oc != new_currency
    {
        return format!("{} ({}) → {} ({})", old_str, oc, new_str, new_currency);
    }
    if let Some(old) = old_price
        && old != new_price
    {
        let diff = new_price - old;
        let sign = if diff > 0.0 { "+" } else { "-" };
        return format!(
            "{} → {}  ({}{})",
            old_str,
            new_str,
            sign,
            format_price(diff.abs(), new_currency)
        );
    }
    format!("→ {}", new_str)
}

fn display_history(entries: &[PriceChange]) {
    if entries.is_empty() {
        info("No price history found");
        return;
    }
    // Group by subscription (preserving order).
    let mut groups: Vec<(i64, String, Vec<PriceChange>)> = Vec::new();
    for e in entries {
        if let Some(g) = groups
            .iter_mut()
            .find(|(id, _, _)| *id == e.subscription_id)
        {
            g.2.push(e.clone());
        } else {
            groups.push((e.subscription_id, e.name.clone(), vec![e.clone()]));
        }
    }
    for (_, name, group_entries) in &groups {
        log(&ansi::bold(&format!("{}:", name)));
        for e in group_entries {
            let date = e.changed_at.chars().take(10).collect::<String>();
            log(&format!(
                "  {}  {}",
                ansi::dim(&date),
                format_diff(
                    e.old_price,
                    e.new_price,
                    e.old_currency.as_deref(),
                    e.new_currency.as_deref().unwrap_or("USD"),
                )
            ));
        }
        log("");
    }
}

pub fn run_history(parsed: &ParsedArgs) -> i32 {
    let id = parsed.str("id").and_then(|i| i.parse::<i64>().ok());
    let json = parsed.bool("json") == Some(true);
    let all = parsed.bool("all") == Some(true);
    let days = parsed
        .str("days")
        .and_then(|d| d.parse::<i64>().ok())
        .filter(|d| *d > 0);

    if json {
        let entries: Vec<PriceChange> = match id {
            Some(i) => get_price_history(i).unwrap_or_default(),
            None => match days {
                Some(d) => get_price_changes_recent(d).unwrap_or_default(),
                None => get_all_price_changes().unwrap_or_default(),
            },
        };
        let mut out = String::from("[\n");
        for (i, e) in entries.iter().enumerate() {
            out.push_str("  {\n");
            out.push_str(&format!("    \"id\": {},\n", e.id));
            out.push_str(&format!("    \"subscriptionId\": {},\n", e.subscription_id));
            out.push_str(&format!(
                "    \"subscriptionName\": {},\n",
                quote_string(&e.name)
            ));
            out.push_str(&format!(
                "    \"oldPrice\": {},\n",
                match e.old_price {
                    Some(p) => crate::json::number(p),
                    None => "null".to_string(),
                }
            ));
            out.push_str(&format!(
                "    \"newPrice\": {},\n",
                crate::json::number(e.new_price)
            ));
            out.push_str(&format!(
                "    \"oldCurrency\": {},\n",
                match &e.old_currency {
                    Some(cc) => quote_string(cc),
                    None => "null".to_string(),
                }
            ));
            out.push_str(&format!(
                "    \"newCurrency\": {},\n",
                match &e.new_currency {
                    Some(cc) => quote_string(cc),
                    None => "null".to_string(),
                }
            ));
            out.push_str(&format!(
                "    \"changedAt\": {}\n",
                quote_string(&e.changed_at)
            ));
            out.push_str(if i + 1 == entries.len() {
                "  }\n"
            } else {
                "  },\n"
            });
        }
        out.push_str("]\n");
        logger::out_str(&out);
        return 0;
    }

    if let Some(i) = id {
        let sub = match get_subscription_by_id(i) {
            Ok(Some(s)) => s,
            Ok(None) => {
                fail(&format!("Subscription with id {} not found", i));
                return 1;
            }
            Err(e) => {
                logger::fail(&format!("Failed to read subscription: {}", e));
                return 1;
            }
        };
        log(&ansi::bold(&format!("Price history for: {}", sub.name)));
        log("");
        let entries = get_price_history(i).unwrap_or_default();
        display_history(&entries);
        return 0;
    }

    if all {
        let entries: Vec<PriceChange> = match days {
            Some(d) => get_price_changes_recent(d).unwrap_or_default(),
            None => get_all_price_changes().unwrap_or_default(),
        };
        if entries.is_empty() {
            info("No price changes recorded");
            return 0;
        }
        log(&ansi::bold("All price changes:"));
        log("");
        display_history(&entries);
        return 0;
    }

    info("Usage: subtrack history <id> | subtrack history --all");
    0
}
