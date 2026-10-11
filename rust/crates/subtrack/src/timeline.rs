//! `timeline` command — port of `timeline.ts` (handleTimeline + charts).
//! `render_bar_chart` is shared with report.ts.

use chrono::Datelike;
use crate::ansi;
use crate::db::{get_subscriptions, QueryOptions, Subscription};
use crate::date::{period_factor, today};
use crate::fx::{fetch_rates, FxRates};
use crate::logger::{self, fail, info, log, warn};
use crate::parser::ParsedArgs;
use crate::price::{format_price, round_currency};
use crate::json::quote_string;

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
            if let Some(converted) = crate::fx::try_convert(s.price, &s.currency, target, &rates.rates)
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
        for sub in subs {
            if sub.status == "cancelled" {
                continue;
            }
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
    for sub in subs {
        if sub.status == "cancelled" {
            continue;
        }
        let cat = sub.tags.first().cloned().unwrap_or_else(|| "Other".to_string());
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
        for sub in subs {
            if sub.status == "cancelled" {
                continue;
            }
            if sub.created_at.as_str() > end.as_str() {
                continue;
            }
            let cat = sub.tags.first().cloned().unwrap_or_else(|| "Other".to_string());
            if let Some(idx) = cat_data.iter().position(|cd| cd.category == cat) {
                cat_data[idx].months[i as usize] += sub.price * period_factor(&sub.cycle, "monthly");
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

    lines.push(ansi::bold("Monthly spending"));
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
fn render_category_chart(totals: &[MonthTotal], cat_data: &[CategoryMonth], currency: &str) -> String {
    let mut lines: Vec<String> = Vec::new();

    lines.push(ansi::bold("Monthly spending by category"));
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
    let months = parsed.str("months").and_then(|m| m.parse::<i64>().ok()).unwrap_or(12);
    if months < 1 || months > 120 {
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
        info("No subscriptions found");
        return 0;
    }

    subs.retain(|s| s.status != "cancelled");

    let mut display_currency = "USD".to_string();
    if let Some(currency) = parsed.str("currency") {
        if !currency.is_empty() {
            display_currency = currency.clone();
            let rates = fetch_rates();
            subs = convert_subs(&subs, &display_currency, &rates);
        }
    }

    let ccy = display_currency.clone();
    let categories = parsed.bool("categories") == Some(true);
    let json = parsed.bool("json") == Some(true);

    if json {
        let totals = calc_monthly_totals(&subs, months);
        let total: f64 = totals.iter().map(|t| t.total).sum();
        let average = if totals.is_empty() {
            0.0
        } else {
            total / totals.len() as f64
        };
        let mut out = String::from("{\n");
        out.push_str(&format!("  \"months\": {},\n", months));
        out.push_str(&format!("  \"currency\": {},\n", quote_string(&ccy)));
        out.push_str(&format!("  \"total\": {},\n", crate::json::number(round_currency(total))));
        out.push_str(&format!(
            "  \"average\": {},\n",
            crate::json::number(round_currency(average))
        ));
        out.push_str("  \"entries\": [\n");
        for (i, t) in totals.iter().enumerate() {
            out.push_str("    {\n");
            out.push_str(&format!("      \"month\": {},\n", quote_string(&t.label)));
            out.push_str(&format!("      \"total\": {}\n", crate::json::number(round_currency(t.total))));
            out.push_str(if i + 1 == totals.len() { "    }\n" } else { "    },\n" });
        }
        out.push_str("  ]");
        if categories {
            let (_, cat_data) = calc_monthly_totals_by_category(&subs, months);
            out.push_str(",\n  \"categories\": {\n");
            for (i, cd) in cat_data.iter().enumerate() {
                let values: Vec<String> = cd
                    .months
                    .iter()
                    .map(|v| crate::json::number(round_currency(*v)))
                    .collect();
                out.push_str(&format!(
                    "    {}: [{}]",
                    quote_string(&cd.category),
                    values.join(", ")
                ));
                out.push_str(if i + 1 == cat_data.len() { "\n" } else { ",\n" });
            }
            out.push_str("  }\n");
        } else {
            out.push('\n');
        }
        out.push_str("}\n");
        logger::out_str(&out);
        return 0;
    }

    if categories {
        let (totals, cat_data) = calc_monthly_totals_by_category(&subs, months);
        log(&render_bar_chart(&totals, &ccy));
        log("");
        log(&render_category_chart(&totals, &cat_data, &ccy));
    } else {
        let totals = calc_monthly_totals(&subs, months);
        log(&render_bar_chart(&totals, &ccy));
    }
    0
}

/// Keep the `warn` symbol referenced (fetch fallback parity).
pub fn _unused() {
    let _ = warn;
}