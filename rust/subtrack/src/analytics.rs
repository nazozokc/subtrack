//! `analytics` command — port of `commands/report.ts` (analyticsCommand)
//! + `analytics.ts` (handleAnalytics / showAnalytics).

use crate::ansi;
use crate::config::load_config;
use crate::date::period_factor;
use crate::db::{QueryOptions, Subscription, get_subscriptions};
use crate::fx::{try_convert, try_convert_subs};
use crate::json::{number, quote_string, sub_json};
use crate::logger::{self, log, out_str, warn};
use crate::parser::ParsedArgs;
use crate::price::{format_price, round_currency};
use std::collections::BTreeMap;

/// Per-tag aggregate: `name → (count, [(currency, monthly), ...])` in
/// first-seen insertion order (JS object semantics).
type ByTag = Vec<(String, (i64, Vec<(String, f64)>))>;
/// (currency, subscription) pairs; comparing raw prices across currencies is
/// meaningless, so each currency gets its own "most expensive" entry.
type MostExpensive<'a> = Vec<(String, &'a Subscription)>;
type SummaryResult<'a> = (i64, BTreeMap<String, f64>, ByTag, MostExpensive<'a>);

/// Port of `calcSummary`. Tag/currency maps keep first-seen insertion order
/// (JS object semantics), so multi-currency joins print in sub order.
fn calc_summary(subs: &[Subscription]) -> SummaryResult<'_> {
    let mut by_currency: BTreeMap<String, f64> = BTreeMap::new();
    let mut by_tag: ByTag = Vec::new();
    let mut most_expensive: MostExpensive<'_> = Vec::new();

    for sub in subs {
        let monthly = sub.price * period_factor(&sub.cycle, "monthly");
        *by_currency.entry(sub.currency.clone()).or_insert(0.0) += monthly;
        for tag in &sub.tags {
            match by_tag.iter_mut().find(|(t, _)| t == tag) {
                Some((_, (count, m))) => {
                    *count += 1;
                    match m.iter_mut().find(|(c, _)| *c == sub.currency) {
                        Some((_, v)) => *v += monthly,
                        None => m.push((sub.currency.clone(), monthly)),
                    }
                }
                None => by_tag.push((tag.clone(), (1, vec![(sub.currency.clone(), monthly)]))),
            }
        }
        match most_expensive
            .iter_mut()
            .find(|(currency, _)| *currency == sub.currency)
        {
            Some((_, current)) => {
                if sub.price > current.price {
                    *current = sub;
                }
            }
            None => most_expensive.push((sub.currency.clone(), sub)),
        }
    }
    most_expensive.sort_by(|a, b| a.0.cmp(&b.0));
    (subs.len() as i64, by_currency, by_tag, most_expensive)
}

/// Non-JSON analytics (port of `showAnalytics`).
fn show_analytics(parsed: &ParsedArgs) -> i32 {
    let all = match get_subscriptions(&QueryOptions::default()) {
        Ok(v) => v,
        Err(e) => {
            logger::fail(&format!("Failed to load subscriptions: {}", e));
            return 1;
        }
    };
    let list: Vec<Subscription> = all
        .iter()
        .filter(|s| s.status != "cancelled")
        .cloned()
        .collect();
    if list.is_empty() {
        logger::info("No active subscriptions found — try `subtrack add` or change status");
        return 0;
    }

    let config = load_config();
    let target_currency = parsed.str("currency");
    let rates = target_currency.as_ref().map(|_| crate::fx::fetch_rates());
    let mut display_list = list.clone();
    if let Some(target) = target_currency.as_deref() {
        let rates = rates.as_ref().expect("rates fetched for target currency");
        let converted = try_convert_subs(&list, target, rates);
        if converted.has_missing {
            warn(&format!(
                "Some prices could not be converted to {}; they are excluded from analytics",
                target
            ));
        }
        display_list = converted
            .list
            .into_iter()
            .filter(|s| s.currency == target)
            .collect();
    }
    let total_count = list.len();
    let (_, _, by_tag, most_expensive) = calc_summary(&display_list);

    let is_yearly = parsed.str("period").as_deref() == Some("yearly");
    let mult: f64 = if is_yearly { 12.0 } else { 1.0 };
    let period_label = if is_yearly { "Yearly" } else { "Monthly" };
    let period_unit = if is_yearly { "year" } else { "month" };

    // Header
    log(&ansi::bold("Subscription Analytics"));
    log("");

    // Overview
    log(&ansi::bold("Overview:"));
    log(&format!(
        "  Total subscriptions:  {}",
        ansi::bold(&total_count.to_string())
    ));
    log("  Status breakdown:");
    let active_count = list.iter().filter(|s| s.status == "active").count();
    let paused_count = list.iter().filter(|s| s.status == "paused").count();
    let cancelled_count = all.iter().filter(|s| s.status == "cancelled").count();
    log(&format!(
        "    {}",
        ansi::green(&format!("active: {}", active_count))
    ));
    if paused_count > 0 {
        log(&format!(
            "    {}",
            ansi::yellow(&format!("paused: {}", paused_count))
        ));
    }
    if cancelled_count > 0 {
        log(&format!(
            "    {}",
            ansi::red(&format!("cancelled: {}", cancelled_count))
        ));
    }

    match most_expensive.as_slice() {
        [] => {}
        [(_, me)] => {
            log(&format!(
                "  Most expensive:       {} ({}/{})",
                ansi::bold(&me.name),
                format_price(me.price, &me.currency),
                crate::date::format_cycle(&me.cycle)
            ));
        }
        most => {
            log(&ansi::bold("  Most expensive by currency:"));
            for (currency, me) in most {
                log(&format!(
                    "    {}  {} ({}/{})",
                    currency,
                    ansi::bold(&me.name),
                    format_price(me.price, &me.currency),
                    crate::date::format_cycle(&me.cycle)
                ));
            }
        }
    }

    // Spending (per currency, optionally converted to target)
    log("");
    log(&ansi::bold(&format!("{} spending:", period_label)));
    let mut spending: BTreeMap<String, f64> = BTreeMap::new();
    for sub in &display_list {
        let monthly = sub.price * period_factor(&sub.cycle, "monthly");
        *spending.entry(sub.currency.clone()).or_insert(0.0) += monthly * mult;
    }
    for (ccy, total) in &spending {
        log(&format!(
            "  {}    {}",
            ccy,
            format_price(round_currency(*total), ccy)
        ));
    }

    // Budget comparison (converted to the display currency when possible)
    let budget = if is_yearly {
        config.yearly_budget.unwrap_or(config.monthly_budget * 12.0)
    } else {
        config.monthly_budget
    };
    if budget > 0.0 {
        let budget_currency = if config.default_currency.is_empty() {
            "USD".to_string()
        } else {
            config.default_currency.clone()
        };
        let keys: Vec<String> = spending.keys().cloned().collect();
        let display_ccy: Option<String> = match &target_currency {
            Some(tc) => Some(tc.clone()),
            None if keys.len() == 1 => Some(keys[0].clone()),
            None => None,
        };

        log(&format!("  {}", ansi::dim(&"─".repeat(30))));
        log(&format!(
            "  Budget:     {}{}",
            ansi::bold(&format_price(budget, &budget_currency)),
            if is_yearly { "/year" } else { "" }
        ));

        let spending_total: f64 = spending.values().sum();

        if let Some(dccy) = &display_ccy {
            if dccy == &budget_currency {
                let remaining = budget - spending_total;
                let display = format_price(remaining, &budget_currency);
                if remaining >= 0.0 {
                    log(&format!("  Remaining:  {}", ansi::green(&display)));
                } else {
                    log(&format!(
                        "  Over budget: {}",
                        ansi::red(&display.replace('-', ""))
                    ));
                }
            } else if let Some(rates) = &rates {
                match try_convert(spending_total, dccy, &budget_currency, &rates.rates) {
                    Some(spending_in_budget) => {
                        let remaining = budget - spending_in_budget;
                        let display = format_price(remaining, &budget_currency);
                        let spent_display = format_price(spending_total, dccy);
                        if remaining >= 0.0 {
                            log(&format!(
                                "  Remaining:  {} ({} in {})",
                                ansi::green(&display),
                                ansi::dim(&format!("{} spent", spent_display)),
                                dccy
                            ));
                        } else {
                            log(&format!(
                                "  Over budget: {} ({} in {})",
                                ansi::red(&display.replace('-', "")),
                                ansi::dim(&format!("{} spent", spent_display)),
                                dccy
                            ));
                        }
                    }
                    None => log(&ansi::dim(&format!(
                        "  (Cannot convert {} budget to {} — missing rate)",
                        budget_currency, dccy
                    ))),
                }
            } else {
                log(&ansi::dim(
                    "  (Cannot compare — no exchange rates available)",
                ));
            }
        } else {
            log(&ansi::dim(
                "  (Multiple currencies — use --currency to compare against budget)",
            ));
        }
    }

    // Tags breakdown
    if !by_tag.is_empty() {
        log("");
        log(&ansi::bold(&format!("{} by tag:", period_label)));
        let mut sorted: Vec<_> = by_tag.iter().collect();
        sorted.sort_by(|a, b| {
            let sum_b: f64 = b.1.1.iter().map(|(_, v)| v).sum();
            let sum_a: f64 = a.1.1.iter().map(|(_, v)| v).sum();
            sum_b
                .partial_cmp(&sum_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for (tag, (count, monthly)) in sorted {
            let price_str = if monthly.len() == 1 {
                format_price(round_currency(monthly[0].1 * mult), &monthly[0].0)
            } else {
                monthly
                    .iter()
                    .map(|(ccy, total)| format_price(round_currency(*total * mult), ccy))
                    .collect::<Vec<_>>()
                    .join(" + ")
            };
            log(&format!(
                "  {} {}/{} ({} sub{})",
                pad_end(tag, 16),
                price_str,
                period_unit,
                count,
                if *count > 1 { "s" } else { "" }
            ));
        }
    }
    0
}

fn pad_end(s: &str, len: usize) -> String {
    let mut out = s.to_string();
    while out.chars().count() < len {
        out.push(' ')
    }
    out
}

/// JSON analytics (port of handleAnalytics json path).
fn analytics_json(parsed: &ParsedArgs) -> i32 {
    let subs = match get_subscriptions(&QueryOptions::default()) {
        Ok(v) => v,
        Err(e) => {
            logger::fail(&format!("Failed to load subscriptions: {}", e));
            return 1;
        }
    };
    let list: Vec<Subscription> = subs
        .iter()
        .filter(|s| s.status != "cancelled")
        .cloned()
        .collect();
    let target_currency = parsed.str("currency");
    let mut display_list = list.clone();
    if let Some(target) = target_currency.as_deref() {
        let rates = crate::fx::fetch_rates();
        let converted = try_convert_subs(&list, target, &rates);
        if converted.has_missing {
            warn(&format!(
                "Some prices could not be converted to {}; they are excluded from analytics",
                target
            ));
        }
        display_list = converted
            .list
            .into_iter()
            .filter(|s| s.currency == target)
            .collect();
    }
    let total_count = list.len();
    let (_, by_currency, by_tag, most_expensive) = calc_summary(&display_list);

    // monthlyByCurrency (insertion order preserved by BTreeMap? — no;
    // calcSummary keeps insertion order; re-derive in display order).
    let mut ccy_order: Vec<String> = Vec::new();
    for sub in &display_list {
        if !ccy_order.contains(&sub.currency) {
            ccy_order.push(sub.currency.clone())
        }
    }
    let mut ccy_json: Vec<String> = Vec::new();
    // JSON mirrors insertion order even though text sorts.
    let mut insertion: Vec<(String, f64)> = Vec::new();
    for ccy in &ccy_order {
        if let Some(v) = by_currency.get(ccy) {
            insertion.push((ccy.clone(), round_currency(*v)))
        }
    }
    for (ccy, total) in insertion {
        ccy_json.push(format!("    {}: {}", quote_string(&ccy), number(total)))
    }

    // monthlyByTag with sub-insertion order
    let mut tag_json: Vec<String> = Vec::new();
    for (tag, (count, monthly)) in &by_tag {
        let mut ccy_entries: Vec<String> = Vec::new();
        for (ccy, total) in monthly {
            ccy_entries.push(format!(
                "        {}: {}",
                quote_string(ccy),
                number(round_currency(*total))
            ))
        }
        let inner = format!(
            "      \"count\": {},\n      \"monthly\": {{\n{}\n      }}",
            count,
            ccy_entries.join(",\n")
        );
        tag_json.push(format!("    {}: {{\n{}\n    }}", quote_string(tag), inner))
    }

    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"totalCount\": {},\n", total_count));
    out.push_str(&format!(
        "  \"monthlyByCurrency\": {{\n{}\n  }},\n",
        ccy_json.join(",\n")
    ));
    out.push_str(&format!(
        "  \"monthlyByTag\": {{\n{}\n  }},\n",
        tag_json.join(",\n")
    ));
    match most_expensive.as_slice() {
        [] => {}
        [(_, me)] => {
            out.push_str(&format!(
                "  \"mostExpensive\": {},\n",
                sub_json(me).replacen("  {", "{", 1)
            ));
        }
        most => {
            let entries: Vec<String> = most
                .iter()
                .map(|(currency, me)| nested_sub_entry(currency, me))
                .collect();
            out.push_str(&format!(
                "  \"mostExpensiveByCurrency\": {{\n{}\n  }},\n",
                entries.join(",\n")
            ));
        }
    }
    out.push_str(&format!(
        "  \"period\": {}\n",
        quote_string(parsed.str("period").as_deref().unwrap_or("monthly"))
    ));
    out.push_str("}\n");
    out_str(&out);
    0
}

/// Render `"CCY": { ... }` for `mostExpensiveByCurrency`, adding two spaces
/// to `sub_json`'s body so the nested object stays readable.
fn nested_sub_entry(currency: &str, sub: &Subscription) -> String {
    let rendered = sub_json(sub).replacen("  {", "{", 1);
    let mut lines = rendered.lines();
    let first = lines.next().unwrap_or("{").trim_start();
    let rest: Vec<String> = lines.map(|line| format!("  {line}")).collect();
    format!(
        "    {}: {}\n{}",
        quote_string(currency),
        first,
        rest.join("\n")
    )
}

pub fn run_analytics(parsed: &ParsedArgs) -> i32 {
    // period validation happens in the command wrapper (parsed errors)
    if parsed.bool("json") == Some(true) {
        analytics_json(parsed)
    } else {
        show_analytics(parsed)
    }
}
