//! `payment` command — port of `payment.ts` (showPayment + handlePayment).

use crate::date::{get_period_date_range, period_factor};
use crate::db::{
    QueryOptions, Subscription, get_llm_usage_total, get_llm_usage_total_by_provider,
    get_subscriptions,
};
use crate::json::{number, quote_string};
use crate::logger::{self, info, log, warn};
use crate::parser::ParsedArgs;
use crate::price::{format_price, format_usd_cost, round_currency};
use std::collections::BTreeMap;

const VALID_PERIODS: [&str; 6] = [
    "weekly",
    "bi-weekly",
    "monthly",
    "quarterly",
    "semi-annual",
    "yearly",
];

fn fmt_period(period: &str) -> &str {
    match period {
        "monthly" => "month",
        "bi-weekly" => "bi-week",
        "semi-annual" => "6 months",
        other => other,
    }
}

struct Entry {
    converted_price: f64,
    currency: String,
    payment_method: Option<String>,
    idx: usize,
}

fn active_subs() -> std::io::Result<Vec<Subscription>> {
    Ok(get_subscriptions(&QueryOptions::default())?
        .into_iter()
        .filter(|s| s.status != "cancelled")
        .collect())
}

pub fn run_payment(parsed: &ParsedArgs) -> i32 {
    let period = parsed
        .str("period")
        .unwrap_or_else(|| "monthly".to_string());
    if !VALID_PERIODS.contains(&period.as_str()) {
        logger::fail(
            "period must be one of: weekly, bi-weekly, monthly, quarterly, semi-annual, yearly",
        );
        return 1;
    }

    let subs = match active_subs() {
        Ok(s) => s,
        Err(e) => {
            logger::fail(&format!("Failed to read subscriptions: {}", e));
            return 1;
        }
    };

    let currency = parsed.str("currency");
    let api = parsed.bool("api") == Some(true);
    let method = parsed.bool("method") == Some(true);

    if parsed.bool("json") == Some(true) {
        return payment_json(&subs, &period, currency.as_deref(), api, method);
    }

    show_payment(&subs, &period, currency.as_deref(), api, method);
    0
}

fn calculate_totals(subs: &[Subscription], period: &str) -> BTreeMap<String, f64> {
    let mut totals: BTreeMap<String, f64> = BTreeMap::new();
    for sub in subs {
        if sub.status == "cancelled" {
            continue;
        }
        let amount = sub.price * period_factor(&sub.cycle, period);
        *totals.entry(sub.currency.clone()).or_insert(0.0) += amount;
    }
    totals
}

fn show_payment(
    list: &[Subscription],
    period: &str,
    currency: Option<&str>,
    include_api: bool,
    by_method: bool,
) {
    if list.is_empty() {
        info("No subscriptions found — try `subtrack add`");
        return;
    }

    let entries: Vec<Entry> = list
        .iter()
        .enumerate()
        .map(|(idx, sub)| Entry {
            converted_price: sub.price * period_factor(&sub.cycle, period),
            currency: sub.currency.clone(),
            payment_method: sub.payment_method.clone(),
            idx,
        })
        .collect();

    let fp = fmt_period(period);

    let mut api_total: i64 = 0;
    let mut api_by_provider: Vec<(String, i64)> = Vec::new();
    if include_api {
        let (from, to) = get_period_date_range(period);
        api_total = get_llm_usage_total(&from, &to).unwrap_or(0);
        api_by_provider = get_llm_usage_total_by_provider(&from, &to).unwrap_or_default();
    }

    if let Some(target) = currency {
        let rates = crate::fx::fetch_rates();
        let amounts: Vec<(f64, String)> = entries
            .iter()
            .map(|e| (e.converted_price, e.currency.clone()))
            .collect();
        let (converted, has_missing) = crate::fx::convert_amounts(&amounts, target, &rates);
        if has_missing {
            warn("Some prices could not be converted (missing rate)");
        }
        let sub_total: f64 = converted.iter().map(|v| v.unwrap_or(0.0)).sum();

        if include_api && api_total > 0 {
            let api_converted =
                crate::fx::try_convert(api_total as f64 / 100.0, "USD", target, &rates.rates);
            if api_converted.is_none() {
                warn("Could not convert API cost to target currency");
            }
            let grand_total = sub_total + api_converted.unwrap_or(0.0);
            let api_part = format!(
                "+ API {} = {}/{fp}",
                format_price(round_currency(api_converted.unwrap_or(0.0)), target),
                crate::ansi::bold(&crate::ansi::yellow(&format_price(
                    round_currency(grand_total),
                    target
                ))),
            );
            log(&format!(
                "{}/{fp}  {}",
                format_price(round_currency(sub_total), target),
                crate::ansi::dim(&api_part)
            ));
        } else {
            log(&format!(
                "{}/{fp}",
                format_price(round_currency(sub_total), target)
            ));
        }
        if include_api && api_total <= 0 {
            info("No API usage found for this period");
        }
        return;
    }

    let groups = calculate_totals(list, period);
    for (ccy, total) in &groups {
        log(&format!(
            "{} {}/{fp}",
            ccy,
            format_price(round_currency(*total), ccy)
        ));
    }

    if by_method {
        // method -> (currency -> total), currency insertion order.
        let mut method_groups: Vec<(String, Vec<(String, f64)>)> = Vec::new();
        for entry in &entries {
            let method = entry
                .payment_method
                .clone()
                .unwrap_or_else(|| "unspecified".to_string());
            let mg = match method_groups.iter_mut().find(|(m, _)| *m == method) {
                Some((_, mg)) => mg,
                None => {
                    method_groups.push((method, Vec::new()));
                    &mut method_groups.last_mut().unwrap().1
                }
            };
            match mg.iter_mut().find(|(c, _)| *c == entry.currency) {
                Some((_, t)) => *t += entry.converted_price,
                None => mg.push((entry.currency.clone(), entry.converted_price)),
            }
        }
        method_groups.sort_by(|a, b| a.0.cmp(&b.0));
        log("");
        log(&crate::ansi::bold("By payment method:"));
        for (method, totals) in method_groups {
            let mut totals = totals;
            totals.sort_by(|a, b| a.0.cmp(&b.0));
            let price_str = totals
                .iter()
                .map(|(ccy, t)| format_price(round_currency(*t), ccy))
                .collect::<Vec<_>>()
                .join(" + ");
            log(&format!("  {} {}/{fp}", pad_end(&method, 16), price_str));
        }
    }

    if include_api {
        if api_total <= 0 {
            info("No API usage found for this period");
        } else {
            let provider_details = api_by_provider
                .iter()
                .map(|(p, t)| format!("{}: {}", p, format_usd_cost(*t, 2)))
                .collect::<Vec<_>>()
                .join(", ");
            let inner = format!(
                "{} {}/{fp}  {}",
                crate::ansi::bold("API usage:"),
                format_usd_cost(api_total, 2),
                crate::ansi::dim(&format!("({})", provider_details))
            );
            log(&crate::ansi::dim(&inner));
        }
    }
}

fn pad_end(s: &str, len: usize) -> String {
    let n = s.chars().count();
    if n >= len {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(len - n))
    }
}

fn payment_json(
    subs: &[Subscription],
    period: &str,
    currency: Option<&str>,
    api: bool,
    method: bool,
) -> i32 {
    if subs.is_empty() {
        logger::out_str(&format!(
            "{{\n  \"period\": {},\n  \"total\": 0,\n  \"subscriptions\": []\n}}\n",
            quote_string(period)
        ));
        return 0;
    }

    let entries: Vec<Entry> = subs
        .iter()
        .enumerate()
        .map(|(idx, sub)| Entry {
            converted_price: sub.price * period_factor(&sub.cycle, period),
            currency: sub.currency.clone(),
            payment_method: sub.payment_method.clone(),
            idx,
        })
        .collect();

    let mut api_total: i64 = 0;
    let mut api_by_provider: Vec<(String, i64)> = Vec::new();
    if api {
        let (from, to) = get_period_date_range(period);
        api_total = get_llm_usage_total(&from, &to).unwrap_or(0);
        api_by_provider = get_llm_usage_total_by_provider(&from, &to).unwrap_or_default();
    }

    let mut final_currency: Option<String> = None;
    let mut converted_amounts: Option<Vec<Option<f64>>> = None;
    let sub_total: f64;

    if let Some(target) = currency {
        let rates = crate::fx::fetch_rates();
        let amounts: Vec<(f64, String)> = entries
            .iter()
            .map(|e| (e.converted_price, e.currency.clone()))
            .collect();
        let (converted, has_missing) = crate::fx::convert_amounts(&amounts, target, &rates);
        if has_missing {
            warn("Some prices could not be converted (missing rate); excluded from totals");
        }
        sub_total = converted.iter().map(|v| v.unwrap_or(0.0)).sum();
        final_currency = Some(target.to_string());
        converted_amounts = Some(converted);
    } else {
        let mut by_currency: BTreeMap<String, f64> = BTreeMap::new();
        for e in &entries {
            *by_currency.entry(e.currency.clone()).or_insert(0.0) += e.converted_price;
        }
        sub_total = by_currency.values().sum();
    }

    // byMethod (only when requested)
    let mut by_method_entries: Vec<(String, MethodTotals)> = Vec::new();
    if method {
        for (i, entry) in entries.iter().enumerate() {
            let m = entry
                .payment_method
                .clone()
                .unwrap_or_else(|| "unspecified".to_string());
            let currency_for = if let Some(fc) = &final_currency {
                fc.clone()
            } else {
                entry.currency.clone()
            };
            let mut amount = entry.converted_price;
            if let (Some(_fc), Some(conv)) = (&final_currency, &converted_amounts) {
                match conv[i] {
                    Some(v) => amount = v,
                    None => continue,
                }
            }
            let slot = match by_method_entries.iter_mut().find(|(name, _)| *name == m) {
                Some((_, mt)) => mt,
                None => {
                    by_method_entries.push((
                        m,
                        MethodTotals {
                            total: 0.0,
                            currencies: Vec::new(),
                            by_currency: Vec::new(),
                        },
                    ));
                    &mut by_method_entries.last_mut().unwrap().1
                }
            };
            slot.total += amount;
            if !slot.currencies.contains(&currency_for) {
                slot.currencies.push(currency_for.clone());
            }
            match slot
                .by_currency
                .iter_mut()
                .find(|(c, _)| *c == currency_for)
            {
                Some((_, t)) => *t += amount,
                None => slot.by_currency.push((currency_for, amount)),
            }
        }
    }

    // ── emit ──
    let mut out: Vec<String> = Vec::new();
    let kv = |k: &str, v: String| format!("{}: {}", quote_string(k), v);
    out.push(kv("period", quote_string(period)));
    out.push(kv("total", number(round_currency(sub_total))));
    out.push(kv(
        "currency",
        final_currency
            .as_deref()
            .map(quote_string)
            .unwrap_or_else(|| "null".to_string()),
    ));
    let subs_json: Vec<String> = entries
        .iter()
        .map(|e| {
            let sub = &subs[e.idx];
            let mut f: Vec<String> = Vec::new();
            f.push(kv("id", sub.id.to_string()));
            f.push(kv("name", quote_string(&sub.name)));
            f.push(kv("price", number(sub.price)));
            f.push(kv("currency", quote_string(&sub.currency)));
            f.push(kv("cycle", quote_string(&sub.cycle)));
            f.push(kv("status", quote_string(&sub.status)));
            f.push(kv("periodPrice", number(round_currency(e.converted_price))));
            format!("    {{\n      {}\n    }}", f.join(",\n      "))
        })
        .collect();
    out.push(kv(
        "subscriptions",
        format!("[\n{}\n  ]", subs_json.join(",\n")),
    ));

    if api && api_total > 0 {
        let providers: Vec<String> = api_by_provider
            .iter()
            .map(|(p, t)| {
                format!(
                    "      {{\n        \"provider\": {},\n        \"total\": {}\n      }}",
                    quote_string(p),
                    t
                )
            })
            .collect();
        out.push(kv(
            "apiUsage",
            format!(
                "{{\n    \"total\": {},\n    \"byProvider\": [\n{}\n    ]\n  }}",
                api_total,
                providers.join(",\n")
            ),
        ));
    }
    if method && !by_method_entries.is_empty() {
        for (_, mt) in by_method_entries.iter_mut() {
            mt.total = round_currency(mt.total);
            for (_, t) in mt.by_currency.iter_mut() {
                *t = round_currency(*t);
            }
        }
        let methods: Vec<String> = by_method_entries
            .iter()
            .map(|(name, mt)| {
                let curs: Vec<String> = mt.currencies.iter().map(|c| quote_string(c)).collect();
                let by_cur: Vec<String> = mt
                    .by_currency
                    .iter()
                    .map(|(c, t)| format!("        {}: {}", quote_string(c), number(*t)))
                    .collect();
                format!(
                    "    {}: {{\n      \"total\": {},\n      \"currencies\": [{}],\n      \"byCurrency\": {{\n{}\n      }}\n    }}",
                    quote_string(name),
                    number(mt.total),
                    curs.join(", "),
                    by_cur.join(",\n")
                )
            })
            .collect();
        out.push(kv("byMethod", format!("{{\n{}\n  }}", methods.join(",\n"))));
    }

    logger::out_str(&format!("{{\n  {}\n}}\n", out.join(",\n  ")));
    0
}

struct MethodTotals {
    total: f64,
    currencies: Vec<String>,
    by_currency: Vec<(String, f64)>,
}
