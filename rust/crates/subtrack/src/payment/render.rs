//! Payment formatting only; no database or FX reads.

use super::calculate::{Entry, MethodTotals, TextMethods};
use super::{ApiUsage, Conversion};
use crate::json::{number, quote_string};
use crate::logger::{info, log};
use crate::price::{format_price, format_usd_cost, round_currency};
use std::collections::BTreeMap;
use subtrack_core::models::Subscription;

fn fmt_period(period: &str) -> &str {
    match period {
        "weekly" => "week",
        "bi-weekly" => "2 weeks",
        "monthly" => "month",
        "quarterly" => "quarter",
        "semi-annual" => "6 months",
        "yearly" => "year",
        other => other,
    }
}

/// Render the deliberately minimal empty JSON shape (regardless of flags).
pub(super) fn empty_json(period: &str) -> String {
    format!(
        "{{\n  \"period\": {},\n  \"total\": 0,\n  \"subscriptions\": []\n}}\n",
        quote_string(period)
    )
}

/// Render text from loaded API/FX values and precomputed, sorted groups.
pub(super) fn text(
    period: &str,
    api: Option<&ApiUsage>,
    conversion: Option<&Conversion>,
    groups: &BTreeMap<String, f64>,
    method_groups: Option<&TextMethods>,
) {
    let include_api = api.is_some();
    let api_total = api.map_or(0, |usage| usage.total);
    let api_by_provider = api.map_or(&[][..], |usage| usage.by_provider.as_slice());
    let fp = fmt_period(period);

    if let Some(conversion) = conversion {
        let target = conversion.currency.as_str();
        let sub_total = conversion.total;

        if include_api && api_total > 0 {
            let api_converted = conversion.api_total;
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

    for (ccy, total) in groups {
        log(&format!(
            "{} {}/{fp}",
            ccy,
            format_price(round_currency(*total), ccy)
        ));
    }

    if let Some(method_groups) = method_groups {
        log("");
        log(&crate::ansi::bold("By payment method:"));
        for (method, totals) in method_groups {
            let price_str = totals
                .iter()
                .map(|(ccy, t)| format_price(round_currency(*t), ccy))
                .collect::<Vec<_>>()
                .join(" + ");
            log(&format!("  {} {}/{fp}", pad_end(method, 16), price_str));
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

/// Serialize JSON without reordering subscriptions, methods, currencies, or providers.
pub(super) fn json(
    subs: &[Subscription],
    period: &str,
    currency: Option<&str>,
    api: Option<&ApiUsage>,
    entries: &[Entry],
    sub_total: f64,
    mut by_method_entries: Vec<(String, MethodTotals)>,
) -> String {
    let mut out: Vec<String> = Vec::new();
    let kv = |k: &str, v: String| format!("{}: {}", quote_string(k), v);
    out.push(kv("period", quote_string(period)));
    out.push(kv("total", number(round_currency(sub_total))));
    out.push(kv(
        "currency",
        currency
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

    if let Some(api) = api.filter(|usage| usage.total > 0) {
        let providers: Vec<String> = api
            .by_provider
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
                api.total,
                providers.join(",\n")
            ),
        ));
    }
    if !by_method_entries.is_empty() {
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

    format!("{{\n  {}\n}}\n", out.join(",\n  "))
}
