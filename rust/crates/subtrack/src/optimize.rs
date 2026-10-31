//! `optimize` command — port of `commands/report.ts` (optimizeCommand)
//! + `optimize.ts` (handleOptimize and analyzers).

mod analysis;
mod render;

use crate::date::today;
use crate::db::{QueryOptions, get_all_price_changes, get_subscriptions};
use crate::fx::{fetch_rates, try_convert_subs};
use crate::logger::{self, info, warn};
use crate::parser::ParsedArgs;
use crate::price::format_price;

/// Validate options, load/convert subscriptions, analyze, and render the report.
pub fn run_optimize(parsed: &ParsedArgs) -> i32 {
    let min_savings = match parsed.str("min-savings") {
        Some(v) => match v.trim().parse::<f64>() {
            Ok(n) if n >= 0.0 => n,
            _ => {
                logger::fail("min-savings must be a non-negative number");
                return 1;
            }
        },
        None => 0.0,
    };
    let discount_rate = match parsed.str("discount-rate") {
        Some(v) => match v.trim().parse::<f64>() {
            Ok(n) if (0.0..=100.0).contains(&n) => n,
            _ => {
                logger::fail("discount-rate must be a number between 0 and 100");
                return 1;
            }
        },
        None => 15.0,
    };
    let exclude: Vec<String> = parsed
        .str("exclude")
        .map(|s| {
            s.split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let mut subs = match get_subscriptions(&QueryOptions::default()) {
        Ok(v) => v,
        Err(e) => {
            logger::fail(&format!("Failed to load subscriptions: {}", e));
            return 1;
        }
    };
    if subs.is_empty() {
        info("No subscriptions found — try `subtrack add`");
        return 0;
    }
    if !exclude.is_empty() {
        subs.retain(|s| !exclude.contains(&s.name));
    }

    let mut display_currency: Option<String> = None;
    if let Some(target) = parsed
        .str("currency")
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
    {
        let rates = fetch_rates();
        let converted = try_convert_subs(&subs, &target, &rates);
        if converted.has_missing {
            warn(&format!(
                "Some prices could not be converted to {}; they are excluded from the report",
                target
            ));
        }
        subs = converted
            .list
            .into_iter()
            .filter(|s| s.currency == target)
            .collect();
        display_currency = Some(target);
    }

    let all_changes = match get_all_price_changes() {
        Ok(v) => v,
        Err(e) => {
            logger::fail(&format!("Failed to read price history: {}", e));
            return 1;
        }
    };
    let report = analysis::analyze(&subs, &all_changes, discount_rate, today());
    let total_yearly_savings = &report.total_yearly_savings;

    if min_savings > 0.0 {
        let (currency, total) = match (&display_currency, total_yearly_savings.as_slice()) {
            (Some(c), _) => (
                c.clone(),
                total_yearly_savings
                    .iter()
                    .find(|(ccy, _)| ccy == c)
                    .map(|(_, total)| *total)
                    .unwrap_or(0.0),
            ),
            (None, [(ccy, total)]) => (ccy.clone(), *total),
            (None, _) => {
                logger::fail(
                    "min-savings requires --currency when subscriptions use multiple currencies",
                );
                return 1;
            }
        };
        if total < min_savings {
            info(&format!(
                "Potential savings ({}/yr) below minimum threshold ({}/yr)",
                format_price(total, &currency),
                format_price(min_savings, &currency)
            ));
            return 0;
        }
    }

    if parsed.flag("json") {
        render::render_json(&report);
    } else {
        render::render_report(&report, display_currency.as_deref());
    }
    0
}
