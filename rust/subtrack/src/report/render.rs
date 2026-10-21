use crate::ansi;
use crate::config::load_config;
use crate::db::{QueryOptions, Subscription, get_all_price_changes, get_subscriptions};
use crate::fx::{FxRates, fetch_rates, try_convert};
use crate::json::quote_string;
use crate::logger::{self, fail, log, warn};
use crate::parser::ParsedArgs;
use crate::price::{format_price, round_currency};
use crate::timeline::{MonthTotal, convert_subs, render_bar_chart};
use chrono::Datelike;

use super::calc::{calc_added, calc_cancelled, calc_top, calc_yearly_totals, yearly_cost};
pub fn run_yearly(parsed: &ParsedArgs) -> i32 {
    if let Some(raw) = parsed.str("year") {
        let valid = raw
            .trim()
            .parse::<i64>()
            .map(|n| (1970..=9999).contains(&n))
            .unwrap_or(false);
        if !valid {
            fail("year must be a valid year");
            return 1;
        }
    }
    run_report(parsed)
}

pub fn run_report(parsed: &ParsedArgs) -> i32 {
    let today = crate::date::today();
    let year = parsed
        .str("year")
        .and_then(|y| y.parse::<i64>().ok())
        .unwrap_or(today.year() as i64);
    if !(1970..=9999).contains(&year) {
        fail("year must be a valid year (e.g. 2025)");
        return 1;
    }

    let subs = match get_subscriptions(&QueryOptions {
        include_archived: true,
        ..Default::default()
    }) {
        Ok(s) => s,
        Err(e) => {
            logger::fail(&format!("Failed to read subscriptions: {}", e));
            return 1;
        }
    };

    let mut display_subs = subs.clone();
    let mut display_currency: Option<String> = None;
    let mut rates: Option<FxRates> = None;
    if let Some(target) = parsed.str("currency") {
        let r = fetch_rates();
        // All-or-nothing conversion: try converting every subscription.
        let all_ok = subs.iter().all(|s| {
            s.currency == target || try_convert(s.price, &s.currency, &target, &r.rates).is_some()
        });
        if all_ok {
            display_subs = convert_subs(&subs, &target, &r);
            display_currency = Some(target.clone());
            rates = Some(r);
        } else {
            warn("Failed to fetch exchange rates; showing in original currencies");
        }
    }

    let totals = calc_yearly_totals(&display_subs, year);
    let total: f64 = totals.iter().map(|t| t.total).sum();

    // Per-currency totals for the selected year (only when no conversion).
    // Use the same month-by-month calculation as `total` so the breakdown and
    // the headline can never disagree.
    let mut by_currency: Vec<(String, f64)> = Vec::new();
    if display_currency.is_none() {
        for (currency, group) in crate::fx::group_by_currency(&subs) {
            let amount: f64 = calc_yearly_totals(&group, year)
                .iter()
                .map(|t| t.total)
                .sum();
            if amount > 0.0 {
                by_currency.push((currency, round_currency(amount)));
            }
        }
        by_currency.sort_by(|a, b| a.0.cmp(&b.0));
    }

    let top = calc_top(&display_subs, 5);
    let price_changes: Vec<crate::db::PriceChange> = get_all_price_changes()
        .unwrap_or_default()
        .into_iter()
        .filter(|c| c.changed_at.starts_with(&format!("{}-", year)))
        .collect();
    let added = calc_added(&subs, year);
    let cancelled = calc_cancelled(&subs, year);

    // ── budget ──
    let config = load_config();
    let yearly_budget = config
        .yearly_budget
        .or(if config.monthly_budget > 0.0 {
            Some(config.monthly_budget * 12.0)
        } else {
            None
        })
        .unwrap_or(0.0);
    let budget_currency = if config.default_currency.is_empty() {
        "USD".to_string()
    } else {
        config.default_currency.clone()
    };

    let chart_ccy = match &display_currency {
        Some(c) => Some(c.clone()),
        None => {
            if by_currency.len() == 1 {
                Some(by_currency[0].0.clone())
            } else {
                None
            }
        }
    };

    // spending used for the budget block
    let mut budget_info: Option<(f64, String, f64, bool)> = None; // (amount, currency, remaining, over)
    if yearly_budget > 0.0 {
        let mut spending = total;
        let mut ccy = chart_ccy.clone();
        if let (Some(c), Some(r)) = (&chart_ccy, &rates)
            && c != &budget_currency
            && let Some(converted) = crate::fx::try_convert(total, c, &budget_currency, &r.rates)
        {
            spending = converted;
            ccy = Some(budget_currency.clone());
        }
        if ccy.as_deref() == Some(budget_currency.as_str()) {
            let remaining = yearly_budget - spending;
            budget_info = Some((
                round_currency(yearly_budget),
                budget_currency.clone(),
                round_currency(remaining),
                remaining < 0.0,
            ));
        }
    }

    let json = parsed.bool("json") == Some(true);

    if json {
        let out = report_json(
            year,
            (display_currency.is_some() || by_currency.len() <= 1).then_some(total),
            &display_currency,
            &by_currency,
            &totals,
            &top,
            &price_changes,
            &added,
            &cancelled,
            &budget_info,
        );
        logger::out_str(&out);
        return 0;
    }

    // ── text output ──
    log(&ansi::bold(&ansi::cyan(&format!(
        "📊 Subscription Report — {}",
        year
    ))));
    log("");
    log(&ansi::bold("Total spending:"));
    if let Some(c) = &display_currency {
        log(&format!(
            "  {}",
            ansi::bold(&ansi::yellow(&format_price(round_currency(total), c)))
        ));
    } else if by_currency.len() == 1 {
        let (ccy, amount) = &by_currency[0];
        log(&format!(
            "  {}",
            ansi::bold(&ansi::yellow(&format_price(*amount, ccy)))
        ));
    } else {
        let mut parts: Vec<(String, f64)> = by_currency.clone();
        parts.sort_by(|a, b| a.0.cmp(&b.0));
        let joined: Vec<String> = parts
            .iter()
            .map(|(ccy, amount)| ansi::bold(&ansi::yellow(&format_price(*amount, ccy))))
            .collect();
        log(&format!("  {}", joined.join(" + ")));
    }

    log("");
    match &chart_ccy {
        Some(c) => {
            log(&render_bar_chart(&totals, c));
        }
        None => {
            log(&ansi::dim(
                "(Monthly chart requires a single currency — use --currency to convert)",
            ));
        }
    }

    log("");
    log(&ansi::bold("Top subscriptions by yearly cost:"));
    if top.is_empty() {
        log("  (none)");
    }
    for s in &top {
        log(&format!(
            "  {:<24} {}/year",
            s.name,
            format_price(round_currency(yearly_cost(s)), &s.currency)
        ));
    }

    if !price_changes.is_empty() {
        log("");
        log(&ansi::bold("Price changes:"));
        for c in &price_changes {
            let date = c.changed_at.chars().take(10).collect::<String>();
            if let (Some(old), true) = (
                &c.old_price,
                c.old_currency.as_deref() == c.new_currency.as_deref(),
            ) {
                let old = *old;
                if old != c.new_price {
                    let diff = c.new_price - old;
                    let sign = if diff > 0.0 { "+" } else { "" };
                    log(&format!(
                        "  {}  {}: {} → {}  ({}{})",
                        ansi::dim(&date),
                        c.name,
                        format_price(old, c.new_currency.as_deref().unwrap_or("USD")),
                        format_price(c.new_price, c.new_currency.as_deref().unwrap_or("USD")),
                        sign,
                        format_price(diff, c.new_currency.as_deref().unwrap_or("USD")),
                    ));
                    continue;
                }
            }
            if let Some(old_ccy) = &c.old_currency
                && c.new_currency.as_deref() != Some(old_ccy.as_str())
            {
                let old_price = c.old_price.unwrap_or(c.new_price);
                log(&format!(
                    "  {}  {}: {} ({}) → {} ({})",
                    ansi::dim(&date),
                    c.name,
                    format_price(old_price, old_ccy),
                    old_ccy,
                    format_price(c.new_price, c.new_currency.as_deref().unwrap_or("USD")),
                    c.new_currency.as_deref().unwrap_or("USD"),
                ));
                continue;
            }
            log(&format!(
                "  {}  {}: → {}",
                ansi::dim(&date),
                c.name,
                format_price(c.new_price, c.new_currency.as_deref().unwrap_or("USD")),
            ));
        }
    }

    log("");
    log(&ansi::bold(&format!("Added this year ({}):", added.len())).to_string());
    for s in &added {
        log(&format!(
            "  {} {}  {}/{}  ({})",
            ansi::green("+"),
            s.name,
            format_price(s.price, &s.currency),
            s.cycle,
            s.created_at,
        ));
    }
    if added.is_empty() {
        log("  (none)");
    }

    log("");
    log(&ansi::bold(&format!("Cancelled this year ({}):", cancelled.len())).to_string());
    for (name, date) in &cancelled {
        log(&format!("  {} {}  ({})", ansi::red("-"), name, date));
    }
    if cancelled.is_empty() {
        log("  (none)");
    }

    if let Some((amount, currency, remaining, over)) = &budget_info {
        log("");
        log(&ansi::bold("Budget:"));
        log(&format!(
            "  Budget: {}/year",
            format_price(*amount, currency)
        ));
        if *over {
            log(&format!(
                "  Over budget: {}",
                ansi::red(&format_price(-remaining, currency))
            ));
        } else {
            log(&format!(
                "  Remaining: {}",
                ansi::green(&format_price(*remaining, currency))
            ));
        }
    } else if yearly_budget > 0.0 {
        log("");
        log(&ansi::dim(
            "(Cannot compare budget — multiple currencies. Use --currency.)",
        ));
    } else {
        log("");
        log(&ansi::dim(
            "(No budget set — use: subtrack config set yearlyBudget <amount>)",
        ));
    }

    0
}

/// Serializes the yearly report command output. One argument per JSON
/// section, mirroring the TS report builder.
#[allow(clippy::too_many_arguments)]
fn report_json(
    year: i64,
    total: Option<f64>,
    display_currency: &Option<String>,
    by_currency: &[(String, f64)],
    totals: &[MonthTotal],
    top: &[Subscription],
    price_changes: &[crate::db::PriceChange],
    added: &[Subscription],
    cancelled: &[(String, String)],
    budget_info: &Option<(f64, String, f64, bool)>,
) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"year\": {},\n", year));
    out.push_str(&format!(
        "  \"total\": {},\n",
        match total {
            Some(total) => crate::json::number(round_currency(total)),
            None => "null".to_string(),
        }
    ));
    out.push_str(&format!(
        "  \"currency\": {},\n",
        match display_currency {
            Some(c) => quote_string(c),
            None => "null".to_string(),
        }
    ));
    out.push_str("  \"byCurrency\": ");
    if display_currency.is_some() || by_currency.is_empty() {
        out.push_str("{}\n");
    } else {
        out.push_str("{\n");
        for (i, (ccy, amount)) in by_currency.iter().enumerate() {
            out.push_str(&format!(
                "    {}: {}{}\n",
                quote_string(ccy),
                crate::json::number(round_currency(*amount)),
                if i + 1 == by_currency.len() { "" } else { "," }
            ));
        }
        out.push_str("  },\n");
    }
    if display_currency.is_some() || by_currency.is_empty() {
        // "byCurrency": {} then continue with comma handling below
        out.pop(); // remove trailing newline
        out.push_str(",\n");
    }
    out.push_str("  \"monthly\": [\n");
    for (i, t) in totals.iter().enumerate() {
        out.push_str("    {\n");
        out.push_str(&format!("      \"month\": {},\n", quote_string(&t.label)));
        out.push_str(&format!(
            "      \"total\": {}\n",
            crate::json::number(round_currency(t.total))
        ));
        out.push_str(if i + 1 == totals.len() {
            "    }\n"
        } else {
            "    },\n"
        });
    }
    out.push_str("  ],\n");
    out.push_str("  \"top\": [\n");
    for (i, s) in top.iter().enumerate() {
        out.push_str("    {\n");
        out.push_str(&format!("      \"id\": {},\n", s.id));
        out.push_str(&format!("      \"name\": {},\n", quote_string(&s.name)));
        out.push_str(&format!(
            "      \"yearlyCost\": {},\n",
            crate::json::number(round_currency(yearly_cost(s)))
        ));
        out.push_str(&format!(
            "      \"currency\": {}\n",
            quote_string(&s.currency)
        ));
        out.push_str(if i + 1 == top.len() {
            "    }\n"
        } else {
            "    },\n"
        });
    }
    out.push_str("  ],\n");
    out.push_str("  \"priceChanges\": [\n");
    for (i, c) in price_changes.iter().enumerate() {
        out.push_str("    {\n");
        out.push_str(&format!(
            "      \"subscriptionId\": {},\n",
            c.subscription_id
        ));
        out.push_str(&format!(
            "      \"subscriptionName\": {},\n",
            quote_string(&c.name)
        ));
        out.push_str(&format!(
            "      \"oldPrice\": {},\n",
            match c.old_price {
                Some(p) => crate::json::number(p),
                None => "null".to_string(),
            }
        ));
        out.push_str(&format!(
            "      \"newPrice\": {},\n",
            crate::json::number(c.new_price)
        ));
        out.push_str(&format!(
            "      \"oldCurrency\": {},\n",
            match &c.old_currency {
                Some(cc) => quote_string(cc),
                None => "null".to_string(),
            }
        ));
        out.push_str(&format!(
            "      \"newCurrency\": {},\n",
            match &c.new_currency {
                Some(cc) => quote_string(cc),
                None => "null".to_string(),
            }
        ));
        out.push_str(&format!(
            "      \"changedAt\": {},\n",
            quote_string(&c.changed_at)
        ));
        let diff = if let (Some(old), true) = (
            &c.old_price,
            c.old_currency.as_deref() == c.new_currency.as_deref(),
        ) {
            if *old != c.new_price {
                Some(round_currency(c.new_price - *old))
            } else {
                None
            }
        } else {
            None
        };
        out.push_str(&format!(
            "      \"diff\": {}\n",
            match diff {
                Some(d) => crate::json::number(d),
                None => "null".to_string(),
            }
        ));
        out.push_str(if i + 1 == price_changes.len() {
            "    }\n"
        } else {
            "    },\n"
        });
    }
    out.push_str("  ],\n");
    out.push_str("  \"added\": [\n");
    for (i, s) in added.iter().enumerate() {
        out.push_str("    {\n");
        out.push_str(&format!("      \"id\": {},\n", s.id));
        out.push_str(&format!("      \"name\": {},\n", quote_string(&s.name)));
        out.push_str(&format!(
            "      \"price\": {},\n",
            crate::json::number(s.price)
        ));
        out.push_str(&format!(
            "      \"currency\": {},\n",
            quote_string(&s.currency)
        ));
        out.push_str(&format!("      \"cycle\": {},\n", quote_string(&s.cycle)));
        out.push_str(&format!(
            "      \"createdAt\": {}\n",
            quote_string(&s.created_at)
        ));
        out.push_str(if i + 1 == added.len() {
            "    }\n"
        } else {
            "    },\n"
        });
    }
    out.push_str("  ],\n");
    // match golden format for empty arrays
    if cancelled.is_empty() {
        out.push_str("  \"cancelled\": [],\n");
    } else {
        out.push_str("  \"cancelled\": [\n");
        for (i, (name, date)) in cancelled.iter().enumerate() {
            out.push_str("    {\n");
            out.push_str(&format!("      \"name\": {},\n", quote_string(name)));
            out.push_str(&format!("      \"date\": {}\n", quote_string(date)));
            out.push_str(if i + 1 == cancelled.len() {
                "    }\n"
            } else {
                "    },\n"
            });
        }
        out.push_str("  ],\n");
    }
    out.push_str("  \"budget\": ");
    match budget_info {
        Some((amount, currency, remaining, over)) => {
            out.push_str("{\n");
            out.push_str(&format!(
                "    \"amount\": {},\n",
                crate::json::number(*amount)
            ));
            out.push_str(&format!("    \"currency\": {},\n", quote_string(currency)));
            out.push_str(&format!(
                "    \"remaining\": {},\n",
                crate::json::number(*remaining)
            ));
            out.push_str(&format!("    \"over\": {}\n", over));
            out.push_str("  }\n");
        }
        None => {
            out.push_str("null\n");
        }
    }
    out.push_str("}\n");
    out
}
