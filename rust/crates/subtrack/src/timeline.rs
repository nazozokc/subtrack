//! `timeline` command — port of `timeline.ts` (handleTimeline + charts).
//! `render_bar_chart` is shared with report.ts.

use crate::ansi;
use crate::date::{period_factor, today};
use crate::db::{QueryOptions, Subscription, get_subscriptions};
use crate::fx::{FxRates, fetch_rates};
use crate::json::num_value;
use crate::logger::{self, fail, info, log, warn};
use crate::parser::ParsedArgs;
use crate::price::{format_price, round_currency};
use chrono::Datelike;

const SHORT_MONTH_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

pub struct MonthTotal {
    pub label: String,
    pub year: i64,
    pub month: usize,
    pub total: f64,
}

/// Convert each subscription's price to the target currency, rounding like
/// `roundCurrency(convertPrice(...))` (port of `convertSubsWithRates`).
pub fn convert_subs(subs: &[Subscription], target: &str, rates: &FxRates) -> Vec<Subscription> {
    subs.iter()
        .map(|s| {
            let mut c = s.clone();
            if let Some(converted) =
                crate::fx::try_convert(s.price, &s.currency, target, &rates.rates)
            {
                c.price = round_currency(converted);
                c.currency = target.to_string();
            }
            c
        })
        .collect()
}

/// `new Date(y, m, 0)` = last day of month `m` (0-based), as a
/// "month-end is before-or-equal" comparison surface.
fn month_end(year: i64, month0: i64) -> String {
    crate::date::last_day_of_month_string(year, month0)
}

/// Port of `calcMonthlyTotals` for the past N months ending now.
pub fn calc_monthly_totals(subs: &[Subscription], months: i64) -> Vec<MonthTotal> {
    let now = today();
    let mut results: Vec<MonthTotal> = Vec::new();
    for i in (0..months).rev() {
        // new Date(now.year, now.month - i, 1) — JS month arithmetic wraps.
        let mut year = now.year() as i64;
        let mut month0 = now.month0() as i64 - i;
        while month0 < 0 {
            month0 += 12;
            year -= 1;
        }
        let label = format!("{}-{:02}", year, month0 + 1);
        let end = month_end(year, month0);
        let mut total = 0.0;
        for sub in subs
            .iter()
            .filter(|s| crate::domain::not_cancelled(&s.status))
        {
            // Skip if subscription was created after this month.
            if sub.created_at.as_str() > end.as_str() {
                continue;
            }
            total += sub.price * period_factor(&sub.cycle, "monthly");
        }
        results.push(MonthTotal {
            label,
            year,
            month: month0 as usize,
            total,
        });
    }
    results
}

struct CategoryMonth {
    category: String,
    months: Vec<f64>,
}

/// Port of `calcMonthlyTotalsByCategory` (first tag, or "Other").
fn calc_monthly_totals_by_category(
    subs: &[Subscription],
    months: i64,
) -> (Vec<MonthTotal>, Vec<CategoryMonth>) {
    let totals = calc_monthly_totals(subs, months);

    let mut cat_set: Vec<String> = Vec::new();
    for sub in subs
        .iter()
        .filter(|s| crate::domain::not_cancelled(&s.status))
    {
        let cat = sub
            .tags
            .first()
            .cloned()
            .unwrap_or_else(|| "Other".to_string());
        if !cat_set.contains(&cat) {
            cat_set.push(cat);
        }
    }
    cat_set.sort();

    let now = today();
    let mut cat_data: Vec<CategoryMonth> = cat_set
        .iter()
        .map(|category| CategoryMonth {
            category: category.clone(),
            months: vec![0.0; months as usize],
        })
        .collect();

    for i in 0..months {
        let mut year = now.year() as i64;
        let mut month0 = now.month0() as i64 - (months - 1 - i);
        while month0 < 0 {
            month0 += 12;
            year -= 1;
        }
        let end = month_end(year, month0);
        for sub in subs
            .iter()
            .filter(|s| crate::domain::not_cancelled(&s.status))
        {
            if sub.created_at.as_str() > end.as_str() {
                continue;
            }
            let cat = sub
                .tags
                .first()
                .cloned()
                .unwrap_or_else(|| "Other".to_string());
            if let Some(idx) = cat_data.iter().position(|cd| cd.category == cat) {
                cat_data[idx].months[i as usize] +=
                    sub.price * period_factor(&sub.cycle, "monthly");
            }
        }
    }

    (totals, cat_data)
}

/// Port of `renderBarChart` (shared with the yearly report).
pub fn render_bar_chart(totals: &[MonthTotal], currency: &str) -> String {
    let max = totals
        .iter()
        .map(|t| t.total)
        .fold(1.0_f64, |a, b| a.max(b))
        .max(1.0);
    let bar_width = 40;
    let label_width = 4;
    let mut lines: Vec<String> = Vec::new();

    lines.push(ansi::bold(&format!("Monthly spending ({currency})")));
    lines.push("─".repeat(bar_width + label_width + 16));
    lines.push(String::new());

    for t in totals {
        let short_mon = SHORT_MONTH_NAMES.get(t.month).copied().unwrap_or("Jan");
        let mut label = format!("{} {:02}", short_mon, t.year % 100);
        label = format!("{:<7}", label);
        let bar_len = (t.total / max * bar_width as f64).round() as usize;
        let bar = "█".repeat(bar_len) + &"░".repeat(bar_width - bar_len);
        let price = format_price(t.total, currency);
        lines.push(format!(" {} {} {}", label, bar, ansi::dim(&price)));
    }

    lines.push(String::new());
    lines.push("─".repeat(bar_width + label_width + 16));

    let avg = totals.iter().map(|t| t.total).sum::<f64>() / totals.len() as f64;
    let total_sum: f64 = totals.iter().map(|t| t.total).sum();
    lines.push(format!(
        " {}",
        ansi::dim(&format!(
            "Avg: {}/mo  │  Total: {}",
            format_price(avg, currency),
            format_price(total_sum, currency)
        ))
    ));

    lines.join("\n")
}

/// Port of `renderCategoryChart`.
fn render_category_chart(
    totals: &[MonthTotal],
    cat_data: &[CategoryMonth],
    currency: &str,
) -> String {
    let mut lines: Vec<String> = Vec::new();

    lines.push(ansi::bold(&format!(
        "Monthly spending by category ({currency})"
    )));
    lines.push(String::new());

    let max_cat_total = cat_data
        .iter()
        .map(|cd| cd.months.iter().cloned().fold(0.0_f64, f64::max))
        .fold(1.0_f64, f64::max);
    let bar_width = 30;

    for cd in cat_data {
        lines.push(ansi::bold(&format!(" {}", cd.category)));
        for (i, m) in cd.months.iter().enumerate() {
            if *m == 0.0 {
                continue;
            }
            let t = &totals[i];
            let short_mon = SHORT_MONTH_NAMES.get(t.month).copied().unwrap_or("Jan");
            let label = format!("  {}", short_mon);
            let label = format!("{:<6}", label);
            let bar_len = ((*m / max_cat_total) * bar_width as f64).round().max(1.0) as usize;
            let bar = "█".repeat(bar_len);
            lines.push(format!(
                " {} {} {}",
                label,
                bar,
                ansi::dim(&format_price(*m, currency))
            ));
        }
        lines.push(String::new());
    }

    lines.join("\n")
}

pub fn run_timeline(parsed: &ParsedArgs) -> i32 {
    let months = parsed
        .str("months")
        .and_then(|m| m.parse::<i64>().ok())
        .unwrap_or(12);
    if !(1..=120).contains(&months) {
        fail("months must be a positive integer (max 120)");
        return 1;
    }

    let mut subs = match get_subscriptions(&QueryOptions::default()) {
        Ok(s) => s,
        Err(e) => {
            logger::fail(&format!("Failed to read subscriptions: {}", e));
            return 1;
        }
    };

    if subs.is_empty() {
        info("No subscriptions found — try `subtrack add`");
        return 0;
    }

    subs.retain(|s| crate::domain::not_cancelled(&s.status));

    // Never sum different currencies. With `--currency` every row is
    // converted to that target; without it, each currency gets its own chart.
    let groups: Vec<(String, Vec<Subscription>)> = match parsed
        .str("currency")
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
    {
        Some(target) => {
            let rates = fetch_rates();
            let converted = crate::fx::try_convert_subs(&subs, &target, &rates);
            if converted.has_missing {
                warn(&format!(
                    "Some prices could not be converted to {}; they are excluded from the timeline",
                    target
                ));
            }
            let list: Vec<Subscription> = converted
                .list
                .into_iter()
                .filter(|s| s.currency == target)
                .collect();
            vec![(target.to_string(), list)]
        }
        None => crate::fx::group_by_currency(&subs),
    };

    let groups: Vec<(String, Vec<Subscription>)> = groups
        .into_iter()
        .filter(|(_, subs)| !subs.is_empty())
        .collect();
    if groups.is_empty() {
        info("No subscriptions to include in the timeline — try `subtrack add`");
        return 0;
    }

    let categories = parsed.flag("categories");
    let json = parsed.flag("json");

    if json {
        let out = if groups.len() == 1 {
            let (ccy, group_subs) = &groups[0];
            let (totals, cat_data) = calc_monthly_totals_by_category(group_subs, months);
            timeline_value(
                months,
                ccy,
                &totals,
                categories.then_some(cat_data.as_slice()),
            )
        } else {
            let entries: Vec<serde_json::Value> = groups
                .iter()
                .map(|(ccy, group_subs)| {
                    let (totals, cat_data) = calc_monthly_totals_by_category(group_subs, months);
                    timeline_value(
                        months,
                        ccy,
                        &totals,
                        categories.then_some(cat_data.as_slice()),
                    )
                })
                .collect();
            serde_json::json!({
                "months": months,
                "currency": serde_json::Value::Null,
                "currencies": entries,
            })
        };
        logger::out_str(&format!(
            "{}\n",
            serde_json::to_string_pretty(&out).unwrap_or_default()
        ));
        return 0;
    }

    for (i, (ccy, group_subs)) in groups.iter().enumerate() {
        if i > 0 {
            log("");
        }
        if categories {
            let (totals, cat_data) = calc_monthly_totals_by_category(group_subs, months);
            log(&render_bar_chart(&totals, ccy));
            log("");
            log(&render_category_chart(&totals, &cat_data, ccy));
        } else {
            let totals = calc_monthly_totals(group_subs, months);
            log(&render_bar_chart(&totals, ccy));
        }
    }
    0
}

fn timeline_value(
    months: i64,
    currency: &str,
    totals: &[MonthTotal],
    cat_data: Option<&[CategoryMonth]>,
) -> serde_json::Value {
    let total: f64 = totals.iter().map(|t| t.total).sum();
    let average = if totals.is_empty() {
        0.0
    } else {
        total / totals.len() as f64
    };
    let entries: Vec<serde_json::Value> = totals
        .iter()
        .map(|t| {
            serde_json::json!({
                "month": t.label,
                "total": num_value(round_currency(t.total)),
            })
        })
        .collect();
    let mut value = serde_json::json!({
        "months": months,
        "currency": currency,
        "total": num_value(round_currency(total)),
        "average": num_value(round_currency(average)),
        "entries": entries,
    });
    if let Some(cats) = cat_data {
        let mut map = serde_json::Map::new();
        for cd in cats {
            map.insert(
                cd.category.clone(),
                serde_json::Value::Array(
                    cd.months
                        .iter()
                        .map(|v| num_value(round_currency(*v)))
                        .collect(),
                ),
            );
        }
        value["categories"] = serde_json::Value::Object(map);
    }
    value
}
