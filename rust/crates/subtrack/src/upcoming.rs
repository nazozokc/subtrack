//! `upcoming` command — port of `upcoming.ts` + `domain/billing.ts`
//! (upcomingWithinDays).

use crate::db::{get_subscriptions, QueryOptions, Subscription};
use crate::date::{
    calculate_next_billing_inclusive, days_until, format_date, format_short_date, today,
};
use crate::json::{number, quote_string};
use crate::logger::{self, log, out_str};
use crate::parser::ParsedArgs;
use crate::price::{format_price, round_currency};
use chrono::NaiveDate;

struct Entry {
    idx: usize,
    next_date: NaiveDate,
    amount: f64,
}

fn active_subs() -> std::io::Result<Vec<Subscription>> {
    Ok(get_subscriptions(&QueryOptions::default())?
        .into_iter()
        .filter(|s| s.status != "cancelled")
        .collect())
}

/// Port of `upcomingWithinDays(listActive(), days)`.
fn upcoming_within_days(subs: &[Subscription], days: i64) -> Vec<Entry> {
    if subs.is_empty() {
        return Vec::new();
    }
    let now = today();
    let end = now + chrono::Duration::days(days);
    let mut entries: Vec<Entry> = Vec::new();
    for (idx, sub) in subs.iter().enumerate() {
        let next =
            calculate_next_billing_inclusive(sub.billing_day, &sub.created_at, &sub.cycle, now);
        if next >= now && next <= end {
            entries.push(Entry {
                idx,
                next_date: next,
                amount: sub.price,
            });
        }
    }
    entries.sort_by_key(|e| e.next_date);
    entries
}

/// Port of `calcUpcomingWithCurrency` (embedded rates never fail).
fn upcoming_with_currency(
    subs: &[Subscription],
    days: i64,
    target: Option<&str>,
) -> (Vec<Entry>, Vec<Subscription>) {
    let mut entries = upcoming_within_days(subs, days);
    let mut display: Vec<Subscription> = subs.to_vec();
    let Some(target) = target else {
        return (entries, display);
    };
    if entries.is_empty() {
        return (entries, display);
    }
    let rates = crate::fx::fetch_rates();
    for entry in entries.iter_mut() {
        let sub = &subs[entry.idx];
        if let Some(converted) =
            crate::fx::try_convert(entry.amount, &sub.currency, target, &rates.rates)
        {
            entry.amount = round_currency(converted);
            let mut s = sub.clone();
            s.price = round_currency(converted);
            s.currency = target.to_string();
            display[entry.idx] = s;
        }
    }
    (entries, display)
}

pub fn run_upcoming(parsed: &ParsedArgs) -> i32 {
    let days = match parsed.str("days") {
        Some(s) => match s.parse::<f64>() {
            Ok(v) if v >= 0.0 && v.fract() == 0.0 => v as i64,
            _ => {
                logger::fail("days must be a non-negative integer");
                return 1;
            }
        },
        None => 7,
    };
    let json = parsed.bool("json") == Some(true);
    let currency = parsed.str("currency");

    let subs = match active_subs() {
        Ok(s) => s,
        Err(e) => {
            logger::fail(&format!("Failed to read subscriptions: {}", e));
            return 1;
        }
    };

    if json {
        let (entries, display) = if currency.is_some() {
            upcoming_with_currency(&subs, days, currency.as_deref())
        } else {
            let e = upcoming_within_days(&subs, days);
            let d = subs.clone();
            (e, d)
        };
        out_str(&entries_json(&entries, &display, days, &subs));
        out_str("\n");
        return 0;
    }

    show_upcoming(&subs, days, currency.as_deref());
    0
}

fn show_upcoming(subs: &[Subscription], days: i64, currency: Option<&str>) {
    let (entries, display) = upcoming_with_currency(subs, days, currency);

    if entries.is_empty() {
        let plural = if days > 1 { "s" } else { "" };
        logger::info(&format!(
            "No upcoming bills in the next {} day{}",
            days, plural
        ));
        return;
    }

    let plural = if days > 1 { "s" } else { "" };
    log(&crate::ansi::bold(&format!(
        "Upcoming bills (next {} day{}):",
        days, plural
    )));
    log("");

    let mut currency_totals: Vec<(String, f64)> = Vec::new();
    for entry in &entries {
        let sub = &display[entry.idx];
        let date_str = format_short_date(entry.next_date);
        let du = days_until(entry.next_date);
        let day_label = if du == 0 {
            " (today)"
        } else if du == 1 {
            " (tomorrow)"
        } else {
            ""
        };
        let tag_str = if sub.tags.is_empty() {
            String::new()
        } else {
            format!("[{}]", sub.tags.join(", "))
        };
        log(&format!(
            "  {}{}  {}  {}/{}  {}",
            crate::ansi::cyan(&date_str),
            crate::ansi::dim(day_label),
            crate::ansi::bold(&sub.name),
            format_price(sub.price, &sub.currency),
            sub.cycle,
            crate::ansi::dim(&tag_str),
        ));
        match currency_totals.iter_mut().find(|(c, _)| *c == sub.currency) {
            Some((_, t)) => *t += sub.price,
            None => currency_totals.push((sub.currency.clone(), sub.price)),
        }
    }

    if entries.len() > 1 {
        log("");
        currency_totals.sort_by(|a, b| a.0.cmp(&b.0));
        let parts: Vec<String> = currency_totals
            .iter()
            .map(|(ccy, total)| format_price(round_currency(*total), ccy))
            .collect();
        log(&format!(
            "  {} {} (across {} subscriptions)",
            crate::ansi::bold("Total:"),
            parts.join(" + "),
            entries.len()
        ));
    }
}

fn entries_json(
    entries: &[Entry],
    display: &[Subscription],
    days: i64,
    _subs: &[Subscription],
) -> String {
    let _ = days;
    if entries.is_empty() {
        return "[]".to_string();
    }
    let items: Vec<String> = entries
        .iter()
        .map(|e| {
            let sub = &display[e.idx];
            let mut f: Vec<String> = Vec::new();
            let kv = |k: &str, v: String| format!("{}: {}", quote_string(k), v);
            f.push(kv("id", sub.id.to_string()));
            f.push(kv("name", quote_string(&sub.name)));
            f.push(kv("price", number(sub.price)));
            f.push(kv("currency", quote_string(&sub.currency)));
            f.push(kv("cycle", quote_string(&sub.cycle)));
            f.push(kv("nextDate", quote_string(&format_date(e.next_date))));
            f.push(kv("amount", number(round_currency(e.amount))));
            f.push(kv("tags", tags_json(&sub.tags)));
            format!("  {{\n    {}\n  }}", f.join(",\n    "))
        })
        .collect();
    format!("[\n{}\n]", items.join(",\n"))
}

/// JS `JSON.stringify(tags, null, 2)` at the entry-object indent level.
fn tags_json(tags: &[String]) -> String {
    if tags.is_empty() {
        return "[]".to_string();
    }
    let items: Vec<String> = tags.iter().map(|t| quote_string(t)).collect();
    format!("[\n      {}\n    ]", items.join(",\n      "))
}
