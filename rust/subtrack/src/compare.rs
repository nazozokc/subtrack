//! `subtrack compare` — side-by-side current vs previous period totals.
//! Port of `commands/compare.ts`.

use chrono::Datelike;

use crate::date::{
    SHORT_MONTH_NAMES, get_period_date_range, get_previous_period_date_range, period_factor,
    to_date,
};
use crate::db::{QueryOptions, get_all_price_changes, get_subscriptions};
use crate::fx::{fetch_rates, try_convert};
use crate::logger;
use crate::parser::ParsedArgs;
use crate::price::{format_price, round_currency};
use crate::table::{ColumnConfig, TableBuilder, calc_column_widths};
use crate::{ansi, json};

const VALID_PERIODS: [&str; 6] = [
    "weekly",
    "bi-weekly",
    "monthly",
    "quarterly",
    "semi-annual",
    "yearly",
];

fn period_label(period: &str) -> &'static str {
    match period {
        "monthly" => "month",
        "yearly" => "year",
        "quarterly" => "quarter",
        "semi-annual" => "6 months",
        "weekly" => "week",
        "bi-weekly" => "2 weeks",
        _ => "month",
    }
}

/// "Jun 1–27, 2026" / "Jul 1 – Sep 30, 2026" / "Dec 15, 2025 – Jan 10, 2026".
fn format_date_range(from: &str, to: &str) -> String {
    let f = to_date(from).unwrap();
    let t = to_date(to).unwrap();
    if f.year() == t.year() {
        if f.month() == t.month() {
            format!(
                "{} {}–{}, {}",
                SHORT_MONTH_NAMES[(f.month0()) as usize],
                f.day(),
                t.day(),
                f.year()
            )
        } else {
            format!(
                "{} {} – {} {}, {}",
                SHORT_MONTH_NAMES[(f.month0()) as usize],
                f.day(),
                SHORT_MONTH_NAMES[(t.month0()) as usize],
                t.day(),
                f.year()
            )
        }
    } else {
        format!(
            "{} {}, {} – {} {}, {}",
            SHORT_MONTH_NAMES[(f.month0()) as usize],
            f.day(),
            f.year(),
            SHORT_MONTH_NAMES[(t.month0()) as usize],
            t.day(),
            t.year()
        )
    }
}

/// `$1 (+0.7%)` with the percentage colored red (>= 0) or green (< 0).
fn fmt_change(current: f64, previous: f64, currency: &str) -> String {
    let diff = current - previous;
    let pct = if previous > 0.0 {
        (diff / previous) * 100.0
    } else {
        0.0
    };
    let sign = if diff >= 0.0 { "+" } else { "" };
    let colored = if pct >= 0.0 {
        ansi::red(&format!("{}{:.1}%", sign, pct))
    } else {
        ansi::green(&format!("{}{:.1}%", sign, pct))
    };
    format!(
        "{} ({})",
        format_price(round_currency(diff), currency),
        colored
    )
}

struct CompareRow {
    label: String,
    current: String,
    previous: String,
    change: String,
    is_currency_total: bool,
    is_divider: bool,
    is_grand_total: bool,
}

fn render_compare_table(rows: &[CompareRow], current_label: &str, previous_label: &str) {
    let headers: Vec<String> = vec![
        "".into(),
        current_label.into(),
        previous_label.into(),
        "Change".into(),
    ];
    let cfg = ColumnConfig {
        headers,
        min_widths: vec![10, 12, 12, 16],
        max_widths: vec![40, 20, 20, 30],
        min_avail: None,
    };
    let data: Vec<Vec<String>> = rows
        .iter()
        .map(|r| {
            vec![
                r.label.clone(),
                r.current.clone(),
                r.previous.clone(),
                r.change.clone(),
            ]
        })
        .collect();
    let col_widths = calc_column_widths(&data, &cfg, crate::ui::term_width(), 40);

    let mut table = TableBuilder::new(crate::table::table_style(&crate::table::default_theme()));
    table.col_widths = col_widths;
    table.head = vec![
        String::new(),
        current_label.to_string(),
        previous_label.to_string(),
        "Change".to_string(),
    ];
    table.col_aligns = vec![
        "left".into(),
        "right".into(),
        "right".into(),
        "right".into(),
    ];

    for row in rows {
        if row.is_divider {
            table.push(vec![ansi::dim("─")]);
        } else if row.is_grand_total {
            let label = ansi::bold(&ansi::yellow(&ansi::bold("Grand Total")));
            table.push(vec![
                label,
                ansi::bold(&ansi::yellow(&row.current)),
                ansi::bold(&ansi::yellow(&row.previous)),
                ansi::bold(&ansi::yellow(&row.change)),
            ]);
        } else if row.is_currency_total {
            table.push(vec![
                ansi::bold(&row.label),
                ansi::bold(&row.current),
                ansi::bold(&row.previous),
                row.change.clone(),
            ]);
        } else {
            table.push(vec![
                row.label.clone(),
                row.current.clone(),
                row.previous.clone(),
                row.change.clone(),
            ]);
        }
    }

    logger::log(&table.to_string());
}

pub fn run_compare(parsed: &ParsedArgs) -> i32 {
    let period = parsed
        .str("period")
        .unwrap_or_else(|| "monthly".to_string());
    if !VALID_PERIODS.contains(&period.as_str()) {
        logger::fail(
            "period must be one of: weekly, bi-weekly, monthly, quarterly, semi-annual, yearly",
        );
        return 1;
    }

    let json_out = parsed.bool("json") == Some(true);
    let target_currency = parsed.str("currency");
    let api = parsed.bool("api") == Some(true);

    let subs = get_subscriptions(&QueryOptions {
        ..Default::default()
    })
    .map(|mut v| {
        v.retain(|s| s.status != "cancelled");
        v
    })
    .unwrap_or_default();

    if json_out {
        if subs.is_empty() {
            logger::out_str(&format!(
                "{{\n  \"period\": {},\n  \"current\": {{}},\n  \"previous\": {{}},\n  \"change\": {{}}\n}}\n",
                json::quote_string(&period)
            ));
            return 0;
        }
        let mut totals: Vec<(&str, f64)> = Vec::new();
        for sub in &subs {
            let monthly = sub.price * period_factor(&sub.cycle, "monthly");
            match totals.iter_mut().find(|(c, _)| *c == sub.currency.as_str()) {
                Some((_, v)) => *v += monthly,
                None => totals.push((sub.currency.as_str(), monthly)),
            }
        }
        let mut lines = String::from("{\n  \"period\": ");
        lines.push_str(&json::quote_string(&period));
        lines.push_str(",\n  \"currentPeriod\": {\n");
        let mut first = true;
        for (ccy, total) in &totals {
            if !first {
                lines.push_str(",\n")
            }
            lines.push_str(&format!(
                "    {}: {}",
                json::quote_string(ccy),
                json::number(round_currency(*total))
            ));
            first = false;
        }
        lines.push_str("\n  },\n  \"subscriptions\": ");
        lines.push_str(&json::number(subs.len() as f64));
        lines.push_str("\n}\n");
        logger::out_str(&lines);
        return 0;
    }

    if subs.is_empty() {
        logger::info("No active subscriptions found — try `subtrack add` or change status");
        return 0;
    }

    let (current_from, current_to) = get_period_date_range(&period);
    let (previous_from, previous_to) = get_previous_period_date_range(&period);
    let current_label = format_date_range(&current_from, &current_to);
    let previous_label = format_date_range(&previous_from, &previous_to);

    // FX rates only fetched when a target currency is requested.
    let rates = target_currency.as_ref().map(|_| fetch_rates());

    let current_totals = calc_sub_total(&subs, &rates, target_currency.as_deref());

    // Previous period — estimate from the most recent price change per sub.
    let all_changes = get_all_price_changes().unwrap_or_default();
    let mut price_before: std::collections::HashMap<i64, (f64, String)> =
        std::collections::HashMap::new();
    for ch in &all_changes {
        if let Some(old_price) = ch.old_price {
            price_before.entry(ch.subscription_id).or_insert((
                old_price,
                ch.old_currency
                    .clone()
                    .unwrap_or_else(|| ch.new_currency.clone().unwrap_or_default()),
            ));
        }
    }
    let previous_subs: Vec<(f64, String, String)> = subs
        .iter()
        .map(|s| match price_before.get(&s.id) {
            Some((p, c)) => (*p, c.clone(), s.cycle.clone()),
            None => (s.price, s.currency.clone(), s.cycle.clone()),
        })
        .collect();
    let previous_totals = calc_totals_from(&previous_subs, &rates, target_currency.as_deref());

    logger::log("");
    logger::log(&ansi::bold(&format!(
        "Comparing {}: {} vs {}",
        period_label(&period),
        current_label,
        previous_label
    )));
    logger::log("");

    let mut rows: Vec<CompareRow> = Vec::new();
    let currencies: Vec<&str> = current_totals
        .keys()
        .chain(previous_totals.keys())
        .map(|s| s.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();

    let mut grand_current = 0.0;
    let mut grand_previous = 0.0;

    for ccy in currencies.clone() {
        let cur = round_currency(current_totals.get(ccy).copied().unwrap_or(0.0));
        let prev = round_currency(previous_totals.get(ccy).copied().unwrap_or(0.0));
        grand_current += cur;
        grand_previous += prev;
        rows.push(CompareRow {
            label: ccy.to_string(),
            current: format_price(cur, ccy),
            previous: format_price(prev, ccy),
            change: fmt_change(cur, prev, ccy),
            is_currency_total: true,
            is_divider: false,
            is_grand_total: false,
        });
    }

    // API usage
    if api {
        rows.push(CompareRow {
            label: String::new(),
            current: String::new(),
            previous: String::new(),
            change: String::new(),
            is_currency_total: false,
            is_divider: true,
            is_grand_total: false,
        });

        let cur_api_usd =
            crate::db::get_llm_usage_total(&current_from, &current_to).unwrap_or(0) as f64 / 100.0;
        let prev_api_usd = crate::db::get_llm_usage_total(&previous_from, &previous_to).unwrap_or(0)
            as f64
            / 100.0;

        let (cur_api_display, prev_api_display) = match (&rates, &target_currency) {
            (Some(r), Some(tc)) => {
                let c = try_convert(cur_api_usd, "USD", tc, &r.rates);
                let p = try_convert(prev_api_usd, "USD", tc, &r.rates);
                match (c, p) {
                    (Some(cv), Some(pv)) => (cv, pv),
                    _ => (cur_api_usd, prev_api_usd),
                }
            }
            _ => (cur_api_usd, prev_api_usd),
        };
        let api_ccy = match (&rates, &target_currency) {
            (Some(_), Some(tc)) => tc.clone(),
            _ => "USD".to_string(),
        };

        rows.push(CompareRow {
            label: ansi::dim("API Usage"),
            current: ansi::dim(&format_price(round_currency(cur_api_display), &api_ccy)),
            previous: ansi::dim(&format_price(round_currency(prev_api_display), &api_ccy)),
            change: ansi::dim(&fmt_change(
                round_currency(cur_api_display),
                round_currency(prev_api_display),
                &api_ccy,
            )),
            is_currency_total: false,
            is_divider: false,
            is_grand_total: false,
        });

        grand_current += round_currency(cur_api_display);
        grand_previous += round_currency(prev_api_display);
    }

    // Grand total — only when a target currency is set or a single currency.
    let can_grand_total = target_currency.is_some() || currencies.is_empty();
    if can_grand_total {
        rows.push(CompareRow {
            label: String::new(),
            current: String::new(),
            previous: String::new(),
            change: String::new(),
            is_currency_total: false,
            is_divider: true,
            is_grand_total: false,
        });
        let total_ccy = target_currency.clone().unwrap_or_else(|| {
            current_totals
                .keys()
                .next()
                .cloned()
                .unwrap_or_else(|| "USD".to_string())
        });
        rows.push(CompareRow {
            label: ansi::bold("Grand Total"),
            current: format_price(grand_current, &total_ccy),
            previous: format_price(grand_previous, &total_ccy),
            change: fmt_change(grand_current, grand_previous, &total_ccy),
            is_currency_total: false,
            is_divider: false,
            is_grand_total: true,
        });
    }

    render_compare_table(&rows, &current_label, &previous_label);
    logger::log("");
    0
}

/// Port of `calcSubTotal` (compare-totals.ts): per-currency monthly totals,
/// optionally converted to a target currency. Cancelled subs are skipped.
fn calc_sub_total(
    subs: &[crate::db::Subscription],
    rates: &Option<crate::fx::FxRates>,
    target_currency: Option<&str>,
) -> std::collections::BTreeMap<String, f64> {
    let mut totals: std::collections::BTreeMap<String, f64> = std::collections::BTreeMap::new();
    for sub in subs {
        if sub.status == "cancelled" {
            continue;
        }
        let normalized = sub.price * period_factor(&sub.cycle, "monthly");
        match target_currency {
            Some(tc) => {
                if let Some(r) = rates {
                    match try_convert(normalized, &sub.currency, tc, &r.rates) {
                        Some(converted) => {
                            *totals.entry(tc.to_string()).or_insert(0.0) += converted;
                        }
                        None => {
                            *totals.entry(sub.currency.clone()).or_insert(0.0) += normalized;
                        }
                    }
                } else {
                    *totals.entry(sub.currency.clone()).or_insert(0.0) += normalized;
                }
            }
            None => {
                *totals.entry(sub.currency.clone()).or_insert(0.0) += normalized;
            }
        }
    }
    totals
}

/// Same math over pre-resolved (price, currency, cycle) entries.
fn calc_totals_from(
    entries: &[(f64, String, String)],
    rates: &Option<crate::fx::FxRates>,
    target_currency: Option<&str>,
) -> std::collections::BTreeMap<String, f64> {
    let mut totals: std::collections::BTreeMap<String, f64> = std::collections::BTreeMap::new();
    for (price, currency, cycle) in entries {
        let monthly = price * period_factor(cycle, "monthly");
        match target_currency {
            Some(tc) => {
                if let Some(r) = rates {
                    match try_convert(monthly, currency, tc, &r.rates) {
                        Some(converted) => {
                            *totals.entry(tc.to_string()).or_insert(0.0) += converted;
                        }
                        None => {
                            *totals.entry(currency.clone()).or_insert(0.0) += monthly;
                        }
                    }
                } else {
                    *totals.entry(currency.clone()).or_insert(0.0) += monthly;
                }
            }
            None => {
                *totals.entry(currency.clone()).or_insert(0.0) += monthly;
            }
        }
    }
    totals
}
