//! `subtrack calendar` — monthly calendar with billing days highlighted.
//! Port of `commands/calendar.ts`. The billing day math lives in
//! `subtrack_core::calendar`; this module owns the DB fetch and rendering.

use chrono::Datelike;

use crate::date::days_in_month;
use crate::db::{QueryOptions, get_subscriptions};
use crate::fx::{fetch_rates, try_convert};
use crate::logger;
use crate::parser::ParsedArgs;
use crate::price::{format_price, round_currency};
use crate::{ansi, json};
use subtrack_core::calendar::{CalendarEntry, CalendarSub, entries_from_subs};

const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

pub(crate) fn calc_calendar_entries(month: u32, year: i32) -> Vec<CalendarEntry> {
    let subs = get_subscriptions(&QueryOptions::default()).unwrap_or_default();
    entries_from_subs(&subs, month, year)
}

fn superscript(n: u32) -> String {
    let sup_map: [&str; 10] = [
        "\u{2070}", "\u{00B9}", "\u{00B2}", "\u{00B3}", "\u{2074}", "\u{2075}", "\u{2076}",
        "\u{2077}", "\u{2078}", "\u{2079}",
    ];
    n.to_string()
        .chars()
        .map(|c| sup_map[(c.to_digit(10).unwrap_or(0)) as usize])
        .collect()
}

pub fn run_calendar(parsed: &ParsedArgs) -> i32 {
    let now = crate::date::today();
    let raw_month = parsed.str("month").and_then(|m| m.parse::<u32>().ok());
    let raw_year = parsed.str("year").and_then(|y| y.parse::<i32>().ok());
    let month = match raw_month {
        Some(m) if (1..=12).contains(&m) => m,
        _ => now.month(),
    };
    let year = match raw_year {
        Some(y) if y >= 1 => y,
        _ => now.year(),
    };

    let mut entries = calc_calendar_entries(month, year);
    let target_ccy = parsed.str("currency");
    if let Some(tc) = &target_ccy {
        let rates = fetch_rates();
        for entry in &mut entries {
            for sub in &mut entry.subs {
                if let Some(converted) = try_convert(sub.price, &sub.currency, tc, &rates.rates) {
                    sub.price = round_currency(converted);
                    sub.currency = tc.clone();
                }
            }
        }
    }

    if parsed.flag("json") {
        let mut out = String::new();
        if entries.is_empty() {
            out.push_str("[]\n");
        } else {
            out.push_str("[\n");
            for (i, entry) in entries.iter().enumerate() {
                out.push_str("  {\n");
                out.push_str(&format!(
                    "    \"day\": {},\n",
                    json::number(entry.day as f64)
                ));
                out.push_str("    \"subs\": [\n");
                for (j, sub) in entry.subs.iter().enumerate() {
                    out.push_str("      {\n");
                    out.push_str(&format!(
                        "        \"name\": {},\n",
                        json::quote_string(&sub.name)
                    ));
                    out.push_str(&format!(
                        "        \"price\": {},\n",
                        json::number(sub.price)
                    ));
                    out.push_str(&format!(
                        "        \"currency\": {},\n",
                        json::quote_string(&sub.currency)
                    ));
                    out.push_str(&format!(
                        "        \"status\": {},\n",
                        json::quote_string(&sub.status)
                    ));
                    out.push_str(&format!(
                        "        \"id\": {}\n",
                        json::number(sub.id as f64)
                    ));
                    out.push_str(if j + 1 == entry.subs.len() {
                        "      }\n"
                    } else {
                        "      },\n"
                    });
                }
                out.push_str(if i + 1 == entries.len() {
                    "    ]\n  }\n"
                } else {
                    "    ]\n  },\n"
                });
            }
            out.push_str("]\n");
        }
        logger::out_str(&out);
        return 0;
    }

    let entry_map: std::collections::HashMap<u32, &Vec<CalendarSub>> =
        entries.iter().map(|e| (e.day, &e.subs)).collect();

    logger::log(&ansi::bold(&format!(
        "      {} {}",
        MONTH_NAMES[(month - 1) as usize],
        year
    )));
    logger::log(&ansi::dim(" Su Mo Tu We Th Fr Sa"));

    let first_day = chrono::NaiveDate::from_ymd_opt(year, month, 1)
        .unwrap()
        .weekday()
        .num_days_from_sunday() as i32;
    let total_days = days_in_month(year, month);

    let mut line = String::new();
    for _ in 0..first_day {
        line.push_str("   ")
    }

    for day in 1..=total_days {
        let day_entries = entry_map.get(&day);
        if let Some(subs) = day_entries {
            let n = subs.len();
            let label = if n > 1 {
                format!("{}{}", day, superscript(n as u32))
            } else {
                format!("{:>2}", day)
            };
            line.push_str(&format!(" {}", ansi::green(&ansi::bold(&label))));
        } else {
            line.push_str(&format!(" {:>2}", day));
        }

        if (first_day + day as i32) % 7 == 0 {
            logger::log(&line);
            line = String::new();
        }
    }
    if !line.is_empty() {
        logger::log(&line);
    }

    logger::log("");

    if entries.is_empty() {
        logger::info("No billing events this month");
        return 0;
    }

    logger::log(&ansi::bold("Billing events:"));
    logger::log("");

    let mut currency_totals: std::collections::BTreeMap<String, f64> =
        std::collections::BTreeMap::new();
    let mut total_subs = 0usize;

    for entry in &entries {
        for sub in &entry.subs {
            let day_label = format!("Day {:>2}", entry.day);
            logger::log(&format!(
                "  {}  {}  {}  {}",
                ansi::cyan(&day_label),
                ansi::bold(&sub.name),
                format_price(sub.price, &sub.currency),
                crate::table::status_color(&sub.status, &crate::table::default_theme())
            ));
            *currency_totals.entry(sub.currency.clone()).or_insert(0.0) += sub.price;
            total_subs += 1;
        }
    }

    if total_subs > 0 {
        logger::log("");
        let parts: Vec<String> = currency_totals
            .iter()
            .map(|(ccy, total)| format_price(round_currency(*total), ccy))
            .collect();
        let plural = if total_subs > 1 { "s" } else { "" };
        logger::log(&format!(
            "  {} {} ({} event{})",
            ansi::bold("Total:"),
            parts.join(" + "),
            total_subs,
            plural
        ));
    }
    0
}
