//! FX conversion — port of `packages/subtrack/src/fx.ts`, with rates embedded
//! from `../data/fx-rates.json` (no network in Rust).

use std::collections::HashMap;

const MIN_SANE_RATE: f64 = 0.0001;
const MAX_SANE_RATE: f64 = 1_000_000.0;

pub struct FxRates {
    pub rates: HashMap<String, f64>,
}

fn embedded_rates() -> FxRates {
    let json = include_str!("../data/fx-rates.json");
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or(serde_json::Value::Null);
    let mut rates = HashMap::new();
    if let Some(map) = v.get("rates").and_then(|r| r.as_object()) {
        for (k, val) in map {
            if let Some(n) = val.as_f64() {
                rates.insert(k.clone(), n);
            }
        }
    }
    FxRates { rates }
}

fn validate_rate(rate: f64, currency: &str) {
    if !rate.is_finite() || rate < MIN_SANE_RATE || rate > MAX_SANE_RATE {
        crate::logger::warn(&format!(
            "Suspicious exchange rate for {}: {} (expected between {} and {})",
            currency, rate, MIN_SANE_RATE, MAX_SANE_RATE
        ))
    }
}

/// Convert a single price; None when no rate is available.
pub fn try_convert(price: f64, from: &str, to: &str, rates: &HashMap<String, f64>) -> Option<f64> {
    if from == to {
        return Some(price);
    }
    if let (Some(from_rate), Some(to_rate)) = (rates.get(from), rates.get(to)) {
        validate_rate(*from_rate, from);
        validate_rate(*to_rate, to);
        // Convert via USD base: source → USD → target
        let in_usd = if from == "USD" {
            price
        } else {
            price / from_rate
        };
        return Some(if to == "USD" {
            in_usd
        } else {
            in_usd * to_rate
        });
    }
    None
}

pub struct Converted {
    pub list: Vec<crate::db::Subscription>,
    pub has_missing: bool,
}

/// Convert each subscription's price to the target currency, keeping
/// subscriptions whose rate is missing in their original currency.
pub fn try_convert_subs(
    subs: &[crate::db::Subscription],
    target_currency: &str,
    rates: &FxRates,
) -> Converted {
    let mut has_missing = false;
    let list: Vec<crate::db::Subscription> = subs
        .iter()
        .map(|s| {
            if let Some(amount) = try_convert(s.price, &s.currency, target_currency, &rates.rates) {
                let mut conv = s.clone();
                conv.price = crate::price::round_currency(amount);
                conv.currency = target_currency.to_string();
                conv
            } else {
                has_missing = true;
                s.clone()
            }
        })
        .collect();
    Converted { list, has_missing }
}

/// All-or-nothing fetch equivalent: embedded rates never fail.
pub fn fetch_converted_subs(
    subs: &[crate::db::Subscription],
    target_currency: &str,
) -> Option<Converted> {
    let rates = embedded_rates();
    Some(try_convert_subs(subs, target_currency, &rates))
}

/// Embedded-rate equivalent of `fetchFxRates()` (never fails).
pub fn fetch_rates() -> FxRates {
    embedded_rates()
}

/// Port of `convertAmounts`: convert each `(amount, currency)` to target,
/// returning `None` for entries with a missing rate.
pub fn convert_amounts(
    amounts: &[(f64, String)],
    target_currency: &str,
    rates: &FxRates,
) -> (Vec<Option<f64>>, bool) {
    let converted: Vec<Option<f64>> = amounts
        .iter()
        .map(|(amount, currency)| try_convert(*amount, currency, target_currency, &rates.rates))
        .collect();
    let has_missing = converted.iter().any(|v| v.is_none());
    (converted, has_missing)
}
