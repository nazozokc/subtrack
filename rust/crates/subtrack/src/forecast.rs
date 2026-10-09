//! `forecast` command — port of `commands/report.ts` (forecastCommand)
//! + `forecast.ts` (handleForecast).

use crate::ansi;
use crate::config::display_theme;
use crate::date::period_factor;
use crate::db::{get_subscriptions, QueryOptions, Subscription};
use crate::fx::{try_convert, FxRates};
use crate::json::{number, quote_string};
use crate::logger::{self, info, log, out_str};
use crate::parser::ParsedArgs;
use crate::price::{format_price, round_currency};
use crate::table::{calc_column_widths, section_title, table_style, zebra_row, ColumnConfig};

struct ForecastEntry {
    name: String,
    monthly: f64,
    currency: String,
}

struct Group {
    entries: Vec<ForecastEntry>,
    total: f64,
}

pub fn run_forecast(parsed: &ParsedArgs) -> i32 {
    // months validation (command layer)
    let months = match parsed.str("months") {
        Some(s) => match s.trim().parse::<i64>() {
            Ok(n) if n >= 1 => n,
            _ => {
                logger::fail("months must be a positive integer");
                return 1;
            }
        },
        None => 12,
    };

    let cancel: Vec<String> = parsed
        .str("cancel")
        .map(|s| {
            s.split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect()
        })
        .unwrap_or_default();

    // add-initiated hypothetical subscription
    let mut add_entry: Option<(String, f64, String, String)> = None;
    if let Some(name) = parsed.str("addName") {
        let price_str = parsed.str("addPrice").unwrap_or_else(|| "0".to_string());
        let price = match price_str.trim().parse::<f64>() {
            Ok(p) if p >= 0.0 => p,
            _ => {
                logger::fail(&format!("Invalid addPrice: {}", price_str));
                return 1;
            }
        };
        let add_currency = parsed.str("addCurrency").unwrap_or_else(|| "USD".to_string());
        if !is_valid_currency(&add_currency) {
            logger::fail(&format!("Invalid addCurrency: \"{}\"", add_currency));
            return 1;
        }
        let add_cycle = parsed.str("addCycle").unwrap_or_else(|| "monthly".to_string());
        if !is_valid_cycle(&add_cycle) {
            logger::fail(&format!("Invalid addCycle: \"{}\"", add_cycle));
            return 1;
        }
        add_entry = Some((name, price, add_currency, add_cycle));
    }

    let subs = match get_subscriptions(&QueryOptions::default()) {
        Ok(v) => v,
        Err(e) => {
            logger::fail(&format!("Failed to load subscriptions: {}", e));
            return 1;
        }
    };
    let list: Vec<Subscription> = subs.into_iter().filter(|s| s.status != "cancelled").collect();

    let mut entries: Vec<ForecastEntry> = list
        .iter()
        .filter(|s| !cancel.contains(&s.name))
        .map(|s| ForecastEntry {
            name: s.name.clone(),
            monthly: s.price * period_factor(&s.cycle, "monthly"),
            currency: s.currency.clone(),
        })
        .collect();

    if let Some((name, price, currency, cycle)) = add_entry {
        entries.push(ForecastEntry {
            name,
            monthly: price * period_factor(&cycle, "monthly"),
            currency,
        });
    }

    if entries.is_empty() {
        info("No active subscriptions to forecast");
        return 0;
    }

    // Currency conversion
    let target_currency = parsed.str("currency");
    let rates: Option<FxRates> = if target_currency.is_some() {
        Some(crate::fx::fetch_rates())
    } else {
        None
    };

    // Calculate totals
    let mut groups: Vec<(String, Group)> = Vec::new();
    for entry in entries {
        let ccy = target_currency.clone().unwrap_or_else(|| entry.currency.clone());
        let slot = groups.iter_mut().find(|(c, _)| *c == ccy);
        let monthly = if let (Some(tc), Some(rates)) = (target_currency.as_deref(), &rates) {
            if tc != entry.currency {
                try_convert(entry.monthly, &entry.currency, tc, &rates.rates).unwrap_or(entry.monthly)
            } else {
                entry.monthly
            }
        } else {
            entry.monthly
        };
        match slot {
            Some((_, g)) => {
                g.entries.push(ForecastEntry {
                    name: entry.name,
                    monthly,
                    currency: ccy,
                });
                g.total += monthly;
            }
            None => {
                let g = Group {
                    entries: vec![ForecastEntry {
                        name: entry.name,
                        monthly,
                        currency: ccy.clone(),
                    }],
                    total: monthly,
                };
                groups.push((ccy, g));
            }
        }
    }
    groups.sort_by(|a, b| a.0.cmp(&b.0));

    // JSON
    if parsed.bool("json") == Some(true) {
        let json = forecast_json(months, target_currency.as_deref(), &cancel, &groups);
        out_str(&format!("{}\n", json));
        return 0;
    }

    let period_label = if months == 12 {
        "Year".to_string()
    } else {
        format!("{} Months", months)
    };
    let is_multi = groups.len() > 1;

    for (ccy, group) in &groups {
        if is_multi {
            log("");
            log(&section_title(ccy, &display_theme()));
        }

        let max_rows = 8;
        let display: Vec<&ForecastEntry> = group.entries.iter().take(max_rows).collect();
        let overflow = group.entries.len() - display.len();

        let headers: Vec<String> = ["Subscription", "Monthly", &period_label]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let min_widths: Vec<i64> = vec![16, 10, 10];
        let max_widths: Vec<i64> = vec![60, 20, 20];

        let rows: Vec<Vec<String>> = display
            .iter()
            .map(|e| {
                vec![
                    e.name.clone(),
                    format_price(e.monthly, &e.currency),
                    format_price(e.monthly * months as f64, &e.currency),
                ]
            })
            .collect();

        let config = ColumnConfig {
            headers: headers.clone(),
            min_widths,
            max_widths,
            min_avail: None,
        };
        let widths = calc_column_widths(&rows, &config, 80, 40);

        let theme = display_theme();
        let col_aligns: Vec<String> = ["left", "right", "right"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        let mut table = crate::table::TableBuilder::new(table_style(&theme));
        table.col_widths = widths.clone();
        table.head = headers;
        table.col_aligns = col_aligns;
        for (i, e) in display.iter().enumerate() {
            let truncated = if e.name.chars().count() > 30 {
                let name: String = e.name.chars().take(27).collect();
                format!("{}...", name)
            } else {
                e.name.clone()
            };
            let row = vec![
                truncated,
                format_price(e.monthly, &e.currency),
                format_price(e.monthly * months as f64, &e.currency),
            ];
            if i % 2 == 0 {
                table.push(zebra_row(&row, theme.zebra))
            } else {
                table.push(row)
            }
        }

        if overflow > 0 {
            table.push(vec![
                ansi::dim(&format!("... and {} more", overflow)),
                String::new(),
                String::new(),
            ]);
        }

        table.push(vec![
            ansi::dim(&"─".repeat(widths[0] as usize - 2)),
            ansi::dim("─"),
            ansi::dim("─"),
        ]);

        let grand_monthly = group.total;
        let grand_total = group.total * months as f64;
        table.push(vec![
            ansi::bold(&ansi::yellow("Total")),
            ansi::bold(&ansi::yellow(&format_price(grand_monthly, ccy))),
            ansi::bold(&ansi::yellow(&format_price(grand_total, ccy))),
        ]);

        if !is_multi {
            log("");
        }
        log(&table.to_string());

        // What-if savings when cancelling (single-currency groups only)
        if !cancel.is_empty() && !is_multi {
            let removed: Vec<ForecastEntry> = list
                .iter()
                .filter(|s| cancel.contains(&s.name))
                .map(|s| ForecastEntry {
                    name: s.name.clone(),
                    monthly: s.price * period_factor(&s.cycle, "monthly"),
                    currency: s.currency.clone(),
                })
                .collect();
            let mut removed_monthly = 0.0_f64;
            for re in &removed {
                removed_monthly += if let Some(rates) = &rates {
                    try_convert(re.monthly, &re.currency, ccy, &rates.rates).unwrap_or(re.monthly)
                } else {
                    re.monthly
                };
            }
            let saved_monthly = removed_monthly;
            let saved_total = removed_monthly * months as f64;
            let new_monthly = grand_monthly - saved_monthly;
            let new_total = grand_total - saved_total;
            let plural = if months == 1 { "month" } else { "month" };
            log("");
            log(&ansi::dim(&format!(
                "Without {}: {} /{} ({} /{}) {}",
                cancel.join(", "),
                ansi::bold(&format_price(new_monthly, ccy)),
                plural,
                format_price(new_total, ccy),
                period_label.to_lowercase(),
                ansi::green(&format!(
                    "— save {}/month ({}/{})",
                    format_price(saved_monthly, ccy),
                    format_price(saved_total, ccy),
                    period_label.to_lowercase()
                ))
            )));
        }
    }
    0
}

fn forecast_json(
    months: i64,
    target_currency: Option<&str>,
    cancel: &[String],
    groups: &[(String, Group)],
) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"months\": {},\n", months));
    out.push_str(&format!(
        "  \"currency\": {},\n",
        target_currency
            .map(quote_string)
            .unwrap_or_else(|| "null".to_string())
    ));
    if !cancel.is_empty() {
        let items: Vec<String> = cancel.iter().map(|c| quote_string(c)).collect();
        out.push_str(&format!(
            "  \"excluded\": [\n    {}\n  ],\n",
            items.join(",\n    ")
        ));
    }
    out.push_str("  \"groups\": {\n");
    let group_items: Vec<String> = groups
        .iter()
        .map(|(ccy, g)| {
            let total = round_currency(g.total);
            let period_total = round_currency(g.total * months as f64);
            let entry_items: Vec<String> = g
                .entries
                .iter()
                .map(|e| {
                    format!(
                        "      {{\n        \"name\": {},\n        \"monthly\": {},\n        \"currency\": {},\n        \"periodTotal\": {}\n      }}",
                        quote_string(&e.name),
                        number(round_currency(e.monthly)),
                        quote_string(&e.currency),
                        number(round_currency(e.monthly * months as f64))
                    )
                })
                .collect();
            format!(
                "    {}: {{\n      \"total\": {},\n      \"monthlyTotal\": {},\n      \"periodTotal\": {},\n      \"entries\": [\n{}\n      ]\n    }}",
                quote_string(ccy),
                number(total),
                number(total),
                number(period_total),
                entry_items.join(",\n")
            )
        })
        .collect();
    out.push_str(&group_items.join(",\n"));
    out.push_str("\n  }\n}");
    out
}

fn is_valid_currency(s: &str) -> bool {
    matches!(s, "USD" | "JPY" | "EUR" | "GBP" | "CAD" | "AUD" | "NZD" | "CHF" | "CNY" | "KRW" | "INR" | "SEK" | "NOK" | "DKK" | "SGD" | "HKD")
}

fn is_valid_cycle(s: &str) -> bool {
    matches!(
        s,
        "monthly" | "yearly" | "weekly" | "bi-weekly" | "quarterly" | "semi-annual"
    )
}