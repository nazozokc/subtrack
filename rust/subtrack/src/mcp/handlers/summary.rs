use super::subscriptions::McpMonthlyByTag;
use crate::db::{self, Subscription};
use crate::json::{number, quote_string as js_str};
use crate::mcp::compact::sub_compact;
use crate::mcp::response::{Resp, arg_num, arg_str};

pub(super) fn mcp_summary_json(subs: &[Subscription]) -> String {
    let mut monthly_by_currency: Vec<(String, f64)> = Vec::new();
    let mut monthly_by_tag: McpMonthlyByTag = Vec::new();
    for sub in subs {
        let monthly = sub.price * crate::date::period_factor(&sub.cycle, "monthly");
        match monthly_by_currency
            .iter_mut()
            .find(|(c, _)| *c == sub.currency)
        {
            Some((_, v)) => *v += monthly,
            None => monthly_by_currency.push((sub.currency.clone(), monthly)),
        }
        for tag in &sub.tags {
            if let Some(entry) = monthly_by_tag.iter_mut().find(|(t, _, _)| t == tag) {
                entry.1 += 1;
                match entry.2.iter_mut().find(|(c, _)| c == &sub.currency) {
                    Some((_, v)) => *v += monthly,
                    None => entry.2.push((sub.currency.clone(), monthly)),
                }
            } else {
                monthly_by_tag.push((tag.clone(), 1, vec![(sub.currency.clone(), monthly)]));
            }
        }
    }

    let mut f: Vec<String> = Vec::new();
    f.push(format!("\"totalCount\":{}", subs.len()));
    let cc: Vec<String> = monthly_by_currency
        .iter()
        .map(|(c, v)| format!("{}:{}", js_str(c), number(*v)))
        .collect();
    f.push(format!("\"monthlyByCurrency\":{{{}}}", cc.join(",")));
    let tt: Vec<String> = monthly_by_tag
        .iter()
        .map(|(tag, count, monthly)| {
            let mm: Vec<String> = monthly
                .iter()
                .map(|(c, v)| format!("{}:{}", js_str(c), number(*v)))
                .collect();
            format!(
                "{}:{{\"count\":{},\"monthly\":{{{}}}}}",
                js_str(tag),
                count,
                mm.join(",")
            )
        })
        .collect();
    f.push(format!("\"monthlyByTag\":{{{}}}", tt.join(",")));

    let mut most_expensive: Vec<(String, &Subscription)> = Vec::new();
    for sub in subs {
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
    match most_expensive.as_slice() {
        [] => {}
        [(_, me)] => f.push(format!("\"mostExpensive\":{}", sub_compact(me))),
        most => {
            let entries: Vec<String> = most
                .iter()
                .map(|(currency, me)| format!("{}:{}", js_str(currency), sub_compact(me)))
                .collect();
            f.push(format!(
                "\"mostExpensiveByCurrency\":{{{}}}",
                entries.join(",")
            ));
        }
    }
    format!("{{{}}}", f.join(","))
}

fn non_cancelled() -> std::io::Result<Vec<Subscription>> {
    Ok(db::get_subscriptions(&db::QueryOptions::default())?
        .into_iter()
        .filter(|s| s.status != "cancelled")
        .collect())
}

pub(super) fn handle_get_summary(
    _args: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Resp {
    match non_cancelled() {
        Ok(subs) => Resp::ok(mcp_summary_json(&subs)),
        Err(e) => Resp::err(e.to_string()),
    }
}

pub(super) fn handle_get_upcoming(
    args: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Resp {
    let days = arg_num(args, "days").unwrap_or(7.0) as i64;
    let subs = match non_cancelled() {
        Ok(s) => s,
        Err(e) => return Resp::err(e.to_string()),
    };
    let entries = crate::upcoming::upcoming_within_days(&subs, days);
    let items: Vec<String> = entries
        .iter()
        .map(|e| {
            let sub = &subs[e.idx];
            format!(
                "{{\"sub\":{},\"nextDate\":{},\"amount\":{}}}",
                sub_compact(sub),
                js_str(&format!("{}T00:00:00.000Z", e.next_date.format("%Y-%m-%d"))),
                number(e.amount),
            )
        })
        .collect();
    Resp::ok(format!("[{}]", items.join(",")))
}

pub(super) fn handle_get_calendar(
    args: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Resp {
    let today = crate::date::today();
    let month = arg_num(args, "month").map(|n| n as u32).unwrap_or_else(|| {
        use chrono::Datelike;
        today.month()
    });
    let year = arg_num(args, "year").map(|n| n as i32).unwrap_or_else(|| {
        use chrono::Datelike;
        today.year()
    });
    let entries = crate::calendar::calc_calendar_entries(month, year);
    let items: Vec<String> = entries
        .iter()
        .map(|e| {
            let subs: Vec<String> = e
                .subs
                .iter()
                .map(|s| {
                    format!(
                        "{{\"name\":{},\"price\":{},\"currency\":{},\"status\":{},\"id\":{}}}",
                        js_str(&s.name),
                        number(s.price),
                        js_str(&s.currency),
                        js_str(&s.status),
                        s.id
                    )
                })
                .collect();
            format!("{{\"day\":{},\"subs\":[{}]}}", e.day, subs.join(","))
        })
        .collect();
    Resp::ok(format!("[{}]", items.join(",")))
}

pub(super) fn handle_export_data(
    args: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Resp {
    let format = arg_str(args, "format").unwrap_or("json").to_string();
    let subs = match db::get_subscriptions(&db::QueryOptions::default()) {
        Ok(s) => s,
        Err(e) => return Resp::err(e.to_string()),
    };
    match format.as_str() {
        "csv" => Resp::ok(crate::export::export_csv(&subs)),
        "json" => Resp::ok(format!("{}\n", crate::json::list_json(&subs))),
        "md" => Resp::ok(crate::export::export_md(&subs)),
        _ => Resp::err(format!(
            "Unsupported format: {}. Supported formats: csv, json, md",
            format
        )),
    }
}
