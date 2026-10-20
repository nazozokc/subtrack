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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_rates_cover_reference_file() {
        let rates = fetch_rates();
        assert!(rates.rates.len() > 10);
        assert_eq!(rates.rates.get("USD"), Some(&1.0));
        assert!(rates.rates.contains_key("JPY"));
        assert!(rates.rates.contains_key("EUR"));
    }

    #[test]
    fn try_convert_passes_through_same_currency() {
        let mut rates = HashMap::new();
        rates.insert("USD".to_string(), 1.0);
        rates.insert("JPY".to_string(), 150.0);
        assert_eq!(try_convert(12.5, "USD", "USD", &rates), Some(12.5));
    }

    #[test]
    fn try_convert_goes_through_usd_base() {
        let mut rates = HashMap::new();
        rates.insert("USD".to_string(), 1.0);
        rates.insert("EUR".to_string(), 0.5);
        // USD → USD → EUR
        assert_eq!(try_convert(100.0, "USD", "EUR", &rates), Some(50.0));
        // EUR → USD → USD
        assert_eq!(try_convert(50.0, "EUR", "USD", &rates), Some(100.0));
        // EUR → USD → JPY
        rates.insert("JPY".to_string(), 150.0);
        assert!((try_convert(100.0, "EUR", "JPY", &rates).unwrap() - 30_000.0).abs() < 1e-6);
    }

    #[test]
    fn try_convert_rejects_unknown_currencies() {
        let rates = fetch_rates();
        assert_eq!(try_convert(1.0, "XXX", "USD", &rates.rates), None);
        assert_eq!(try_convert(1.0, "USD", "YYY", &rates.rates), None);
    }

    #[test]
    fn try_convert_subs_keeps_missing_rates_unconverted() {
        fn sub(price: f64, currency: &str) -> crate::db::Subscription {
            crate::db::Subscription {
                id: 1,
                name: "t".to_string(),
                price,
                currency: currency.to_string(),
                cycle: "monthly".to_string(),
                status: "active".to_string(),
                billing_day: None,
                created_at: "2026-10-08".to_string(),
                notes: None,
                payment_method: None,
                contract_start: None,
                contract_end: None,
                auto_renewal: true,
                vendor_name: None,
                vendor_url: None,
                plan_tier: None,
                discount_amount: None,
                discount_type: None,
                tags: vec![],
            }
        }
        let sub_bad = sub(10.0, "XXX");
        let sub_usd = sub(10.0, "USD");
        let rates = fetch_rates();
        let converted = try_convert_subs(&[sub_bad], "JPY", &rates);
        assert!(converted.has_missing);
        assert_eq!(converted.list[0].currency, "XXX");

        let converted = try_convert_subs(&[sub_usd], "USD", &rates);
        assert!(!converted.has_missing);
        assert_eq!(converted.list[0].price, 10.0);
    }

    #[test]
    fn convert_amounts_flags_missing_rates() {
        let rates = fetch_rates();
        let (values, has_missing) = convert_amounts(
            &[(1.0, "USD".to_string()), (2.0, "ZZZ".to_string())],
            "USD",
            &rates,
        );
        assert_eq!(values[0], Some(1.0));
        assert_eq!(values[1], None);
        assert!(has_missing);

        let (values, has_missing) = convert_amounts(&[(1.0, "USD".to_string())], "USD", &rates);
        assert_eq!(values[0], Some(1.0));
        assert!(!has_missing);
    }
}
