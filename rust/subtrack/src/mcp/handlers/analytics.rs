use std::collections::HashMap;

use super::summary::mcp_summary_json;
use crate::db::{self, Subscription};
use crate::json::{number, quote_string as js_str};
use crate::mcp::compact::price_change_compact;
use crate::mcp::response::{Resp, arg_num, arg_str};

pub(super) fn handle_get_history(
    args: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Resp {
    let id = arg_num(args, "id").map(|n| n as i64);
    let days = arg_num(args, "days").map(|n| n as i64);
    let entries = if let Some(id) = id {
        db::get_price_history(id)
    } else {
        db::get_all_price_changes()
    };
    let mut entries = match entries {
        Ok(e) => e,
        Err(e) => return Resp::err(e.to_string()),
    };
    if id.is_none()
        && let Some(days) = days
        && days > 0
    {
        let cutoff = crate::date::today_string_days_ago(days);
        entries.retain(|c| c.changed_at.as_str() >= cutoff.as_str());
    }
    let items: Vec<String> = entries.iter().map(price_change_compact).collect();
    Resp::ok(format!("[{}]", items.join(",")))
}

pub(super) fn handle_get_analytics(
    _args: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Resp {
    let all = match db::get_subscriptions(&db::QueryOptions {
        include_archived: true,
        ..Default::default()
    }) {
        Ok(s) => s,
        Err(e) => return Resp::err(e.to_string()),
    };
    let active: Vec<Subscription> = all
        .iter()
        .filter(|s| crate::domain::listed(&s.status))
        .cloned()
        .collect();
    let count = |st: &str| all.iter().filter(|s| s.status == st).count();
    let summary = mcp_summary_json(&active);
    // Splice statusBreakdown onto the end of the summary object.
    let breakdown = format!(
        "\"statusBreakdown\":{{\"active\":{},\"paused\":{},\"cancelled\":{},\"archived\":{}}}",
        count("active"),
        count("paused"),
        count("cancelled"),
        count("archived"),
    );
    let inner = &summary[1..summary.len() - 1];
    let body = if inner.is_empty() {
        breakdown
    } else {
        format!("{},{}", inner, breakdown)
    };
    Resp::ok(format!("{{{}}}", body))
}

pub(super) fn handle_get_forecast(
    args: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Resp {
    let months = arg_num(args, "months").unwrap_or(12.0) as i64;
    let target = arg_str(args, "currency").map(|s| s.to_string());
    let cancel_names: Vec<String> = arg_str(args, "cancel")
        .map(|t| {
            t.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let rates = target.as_ref().map(|_| crate::fx::fetch_rates());

    let subs = match db::get_subscriptions(&db::QueryOptions::default()) {
        Ok(s) => s,
        Err(e) => return Resp::err(e.to_string()),
    };
    let active: Vec<&Subscription> = subs
        .iter()
        .filter(|s| crate::domain::not_cancelled(&s.status) && !cancel_names.contains(&s.name))
        .collect();

    let mut entries: Vec<String> = Vec::new();
    let mut raw_total = 0.0f64;
    for sub in &active {
        let monthly = sub.price * crate::date::period_factor(&sub.cycle, "monthly");
        let mut monthly_converted_raw: Option<f64> = None;
        if let (Some(tc), Some(r)) = (&target, &rates)
            && let Some(c) = crate::fx::try_convert(monthly, &sub.currency, tc, &r.rates)
        {
            monthly_converted_raw = Some(c);
        }
        raw_total += monthly_converted_raw.unwrap_or(monthly);
        let mut fields = vec![
            format!("\"name\":{}", js_str(&sub.name)),
            format!("\"price\":{}", number(sub.price)),
            format!("\"currency\":{}", js_str(&sub.currency)),
            format!("\"cycle\":{}", js_str(&sub.cycle)),
            format!(
                "\"monthly\":{}",
                number(crate::price::round_currency(monthly))
            ),
        ];
        if let Some(c) = monthly_converted_raw {
            fields.push(format!(
                "\"monthlyConverted\":{}",
                number(crate::price::round_currency(c))
            ));
        }
        entries.push(format!("{{{}}}", fields.join(",")));
    }

    let display = target.clone().unwrap_or_else(|| "mixed".to_string());
    let body = format!(
        "\"months\":{},\"currency\":{},\"monthlyTotal\":{},\"yearlyTotal\":{},\"totalSubscriptions\":{},\"entries\":[{}]",
        months,
        js_str(&display),
        number(crate::price::round_currency(raw_total)),
        number(crate::price::round_currency(raw_total * 12.0)),
        entries.len(),
        entries.join(","),
    );
    Resp::ok(format!("{{{}}}", body))
}

fn compare_totals(
    subs: &[Subscription],
    rates: &Option<crate::fx::FxRates>,
    target: Option<&str>,
    period: &str,
    use_previous: bool,
) -> Vec<(String, f64)> {
    let price_before: HashMap<i64, (f64, String)> = if use_previous {
        let mut m: HashMap<i64, (f64, String)> = HashMap::new();
        if let Ok(changes) = db::get_all_price_changes() {
            for c in &changes {
                if let Some(old) = c.old_price {
                    m.entry(c.subscription_id).or_insert_with(|| {
                        (
                            old,
                            c.old_currency
                                .clone()
                                .or_else(|| c.new_currency.clone())
                                .unwrap_or_default(),
                        )
                    });
                }
            }
        }
        m
    } else {
        HashMap::new()
    };

    let mut totals: Vec<(String, f64)> = Vec::new();
    let mut add = |ccy: String, amount: f64| match totals.iter_mut().find(|(c, _)| *c == ccy) {
        Some((_, v)) => *v += amount,
        None => totals.push((ccy, amount)),
    };

    for sub in subs
        .iter()
        .filter(|s| crate::domain::not_cancelled(&s.status))
    {
        let (price, currency) = if use_previous {
            match price_before.get(&sub.id) {
                Some((p, c)) => (*p, c.clone()),
                None => (sub.price, sub.currency.clone()),
            }
        } else {
            (sub.price, sub.currency.clone())
        };
        let monthly = price * crate::date::period_factor(&sub.cycle, period);
        match (target, rates) {
            (Some(tc), Some(r)) => match crate::fx::try_convert(monthly, &currency, tc, &r.rates) {
                Some(c) => add(tc.to_string(), c),
                None => add(currency, monthly),
            },
            _ => add(currency, monthly),
        }
    }
    totals
}

pub(super) fn handle_compare(args: Option<&serde_json::Map<String, serde_json::Value>>) -> Resp {
    let period = arg_str(args, "period").unwrap_or("monthly").to_string();
    if crate::date::cycle_days(&period).is_some()
        || !crate::validation::CYCLE_VALUES.contains(&period.as_str())
    {
        return Resp::err(format!("Invalid period: {}", period));
    }
    let target = arg_str(args, "currency").map(|s| s.to_string());
    let rates = target.as_ref().map(|_| crate::fx::fetch_rates());

    let subs = match db::get_subscriptions(&db::QueryOptions::default()) {
        Ok(s) => s,
        Err(e) => return Resp::err(e.to_string()),
    };
    let current = compare_totals(&subs, &rates, target.as_deref(), &period, false);
    let previous = compare_totals(&subs, &rates, target.as_deref(), &period, true);

    let mut currencies: Vec<String> = Vec::new();
    for (c, _) in current.iter().chain(previous.iter()) {
        if !currencies.contains(c) {
            currencies.push(c.clone());
        }
    }
    currencies.sort();

    let rows: Vec<String> = currencies
        .iter()
        .map(|c| {
            let cur = current
                .iter()
                .find(|(k, _)| k == c)
                .map(|(_, v)| *v)
                .unwrap_or(0.0);
            let prev = previous
                .iter()
                .find(|(k, _)| k == c)
                .map(|(_, v)| *v)
                .unwrap_or(0.0);
            format!(
                "{{\"currency\":{},\"current\":{},\"previous\":{}}}",
                js_str(c),
                number(crate::price::round_currency(cur)),
                number(crate::price::round_currency(prev)),
            )
        })
        .collect();

    let grand_current: f64 = currencies
        .iter()
        .map(|c| {
            current
                .iter()
                .find(|(k, _)| k == c)
                .map(|(_, v)| *v)
                .unwrap_or(0.0)
        })
        .map(crate::price::round_currency)
        .sum();
    let grand_previous: f64 = currencies
        .iter()
        .map(|c| {
            previous
                .iter()
                .find(|(k, _)| k == c)
                .map(|(_, v)| *v)
                .unwrap_or(0.0)
        })
        .map(crate::price::round_currency)
        .sum();
    let change = grand_current - grand_previous;
    let change_percent = if grand_previous > 0.0 {
        ((change / grand_previous) * 10000.0).round() / 100.0
    } else {
        0.0
    };

    let body = format!(
        "\"period\":{},\"currency\":{},\"rows\":[{}],\"grandTotal\":{{\"current\":{},\"previous\":{},\"change\":{},\"changePercent\":{}}}",
        js_str(&period),
        target
            .as_deref()
            .map(js_str)
            .unwrap_or_else(|| "null".to_string()),
        rows.join(","),
        number(grand_current),
        number(grand_previous),
        number(change),
        number(change_percent),
    );
    Resp::ok(format!("{{{}}}", body))
}
