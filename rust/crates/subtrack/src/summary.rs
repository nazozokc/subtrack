//! `summary` command — port of `payment.ts` (calcSummary + showSummary).

use crate::db::{get_subscriptions, QueryOptions, Subscription};
use crate::json::{number, quote_string, sub_json};
use crate::logger::{log, out_str};
use crate::parser::ParsedArgs;
use crate::price::{format_price, round_currency};

#[derive(Clone)]
struct TagInfo {
    count: i64,
    /// (currency, monthly total) in first-appearance order (JS object order).
    monthly: Vec<(String, f64)>,
}

pub struct Summary {
    total_count: usize,
    /// (currency, total) in first-appearance order.
    pub monthly_by_currency: Vec<(String, f64)>,
    monthly_by_tag: Vec<(String, TagInfo)>,
    most_expensive: Option<usize>,
}

pub fn calc_summary_internal(subs: &[Subscription]) -> Summary {
    calc_summary(subs)
}

fn calc_summary(subs: &[Subscription]) -> Summary {
    let mut monthly_by_currency: Vec<(String, f64)> = Vec::new();
    let mut monthly_by_tag: Vec<(String, TagInfo)> = Vec::new();

    for sub in subs {
        let monthly = sub.price * crate::date::period_factor(&sub.cycle, "monthly");

        match monthly_by_currency.iter_mut().find(|(c, _)| *c == sub.currency) {
            Some((_, v)) => *v += monthly,
            None => monthly_by_currency.push((sub.currency.clone(), monthly)),
        }

        for tag in &sub.tags {
            let entry = match monthly_by_tag.iter_mut().find(|(t, _)| t == tag) {
                Some((_, info)) => info,
                None => {
                    monthly_by_tag.push((
                        tag.clone(),
                        TagInfo {
                            count: 0,
                            monthly: Vec::new(),
                        },
                    ));
                    let last = monthly_by_tag.len() - 1;
                    &mut monthly_by_tag[last].1
                }
            };
            entry.count += 1;
            match entry.monthly.iter_mut().find(|(c, _)| *c == sub.currency) {
                Some((_, v)) => *v += monthly,
                None => entry.monthly.push((sub.currency.clone(), monthly)),
            }
        }
    }

    let most_expensive = if subs.is_empty() {
        None
    } else {
        let mut idx = 0usize;
        for (i, sub) in subs.iter().enumerate() {
            if sub.price > subs[idx].price {
                idx = i;
            }
        }
        Some(idx)
    };

    Summary {
        total_count: subs.len(),
        monthly_by_currency,
        monthly_by_tag,
        most_expensive,
    }
}

pub fn run_summary(parsed: &ParsedArgs) -> i32 {
    let json = parsed.bool("json") == Some(true);

    let subs: Vec<Subscription> = match get_subscriptions(&QueryOptions::default()) {
        Ok(s) => s.into_iter().filter(|s| s.status != "cancelled").collect(),
        Err(e) => {
            crate::logger::fail(&format!("Failed to read subscriptions: {}", e));
            return 1;
        }
    };

    if json {
        let data = calc_summary(&subs);
        out_str(&summary_json(&subs, &data));
        out_str("\n");
        return 0;
    }

    show_summary(&subs);
    0
}

fn show_summary(subs: &[Subscription]) {
    if subs.is_empty() {
        crate::logger::info("No subscriptions found — try `subtrack add`");
        return;
    }
    let data = calc_summary(subs);

    log(&format!(
        "Total subscriptions:  {}",
        crate::ansi::bold(&data.total_count.to_string())
    ));

    if let Some(idx) = data.most_expensive {
        let me = &subs[idx];
        log(&format!(
            "Most expensive:       {} ({}/{})",
            crate::ansi::bold(&me.name),
            format_price(me.price, &me.currency),
            crate::date::format_cycle(&me.cycle)
        ));
    }

    log("");
    log(&crate::ansi::bold("Monthly by currency:"));
    let mut currencies = data.monthly_by_currency.clone();
    currencies.sort_by(|a, b| a.0.cmp(&b.0));
    for (ccy, total) in currencies {
        log(&format!(
            "  {}    {}",
            ccy,
            format_price(round_currency(total), &ccy)
        ));
    }

    if !data.monthly_by_tag.is_empty() {
        log("");
        log(&crate::ansi::bold("Monthly by tag:"));
        let mut tags = data.monthly_by_tag.clone();
        tags.sort_by(|a, b| {
            let sa: f64 = a.1.monthly.iter().map(|(_, v)| *v).sum();
            let sb: f64 = b.1.monthly.iter().map(|(_, v)| *v).sum();
            sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
        });
        for (tag, info) in tags {
            let price_str = if info.monthly.len() == 1 {
                let (ccy, total) = &info.monthly[0];
                format_price(round_currency(*total), ccy)
            } else {
                info.monthly
                    .iter()
                    .map(|(ccy, total)| format_price(round_currency(*total), ccy))
                    .collect::<Vec<_>>()
                    .join(" + ")
            };
            let plural = if info.count > 1 { "s" } else { "" };
            log(&format!(
                "  {} {}/month ({} sub{})",
                pad_end(&tag, 16),
                price_str,
                info.count,
                plural
            ));
        }
    }
}

/// JS `String.prototype.padEnd` (no truncation).
fn pad_end(s: &str, len: usize) -> String {
    let n = s.chars().count();
    if n >= len {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(len - n))
    }
}

fn summary_json(subs: &[Subscription], data: &Summary) -> String {
    let mut f: Vec<String> = Vec::new();
    let kv = |k: &str, v: String| format!("{}: {}", quote_string(k), v);

    f.push(kv("totalCount", data.total_count.to_string()));

    let by_ccy: Vec<String> = data
        .monthly_by_currency
        .iter()
        .map(|(c, v)| format!("    {}: {}", quote_string(c), number(*v)))
        .collect();
    f.push(kv(
        "monthlyByCurrency",
        format!("{{\n{}\n  }}", by_ccy.join(",\n")),
    ));

    let tags: Vec<String> = data
        .monthly_by_tag
        .iter()
        .map(|(tag, info)| {
            let monthly: Vec<String> = info
                .monthly
                .iter()
                .map(|(c, v)| format!("        {}: {}", quote_string(c), number(*v)))
                .collect();
            format!(
                "    {}: {{\n      \"count\": {},\n      \"monthly\": {{\n{}\n      }}\n    }}",
                quote_string(tag),
                info.count,
                monthly.join(",\n")
            )
        })
        .collect();
    f.push(kv("monthlyByTag", format!("{{\n{}\n  }}", tags.join(",\n"))));

    if let Some(idx) = data.most_expensive {
        // `sub_json` already uses the 2-space object body indentation this
        // nesting level needs; only drop its leading first-line indent.
        f.push(kv("mostExpensive", sub_json(&subs[idx]).trim_start().to_string()));
    }

    format!("{{\n  {}\n}}", f.join(",\n  "))
}
