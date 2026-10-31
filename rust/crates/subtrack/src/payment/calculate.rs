//! Pure period amounts and aggregation. Ordering intentionally differs by output format.

use crate::date::period_factor;
use std::collections::BTreeMap;
use subtrack_core::models::Subscription;

pub(super) struct Entry {
    pub(super) converted_price: f64,
    pub(super) currency: String,
    pub(super) payment_method: Option<String>,
    pub(super) idx: usize,
}

/// Scale subscription prices without rounding or changing their encounter order.
pub(super) fn entries(subs: &[Subscription], period: &str) -> Vec<Entry> {
    subs.iter()
        .enumerate()
        .map(|(idx, sub)| Entry {
            converted_price: sub.price * period_factor(&sub.cycle, period),
            currency: sub.currency.clone(),
            payment_method: sub.payment_method.clone(),
            idx,
        })
        .collect()
}

/// Text totals retain the cancellation filter and sort currencies by key.
pub(super) fn currency_totals(subs: &[Subscription], period: &str) -> BTreeMap<String, f64> {
    subs.iter()
        .filter(|sub| crate::domain::not_cancelled(&sub.status))
        .fold(BTreeMap::new(), |mut totals, sub| {
            let amount = sub.price * period_factor(&sub.cycle, period);
            *totals.entry(sub.currency.clone()).or_insert(0.0) += amount;
            totals
        })
}

/// Sum converted amounts, excluding missing rates without rounding individual rows.
pub(super) fn converted_total(amounts: &[Option<f64>]) -> f64 {
    amounts.iter().map(|v| v.unwrap_or(0.0)).sum()
}

/// JSON's unconverted total sums currency groups in key order, not row order.
pub(super) fn json_total(entries: &[Entry]) -> f64 {
    let mut by_currency: BTreeMap<String, f64> = BTreeMap::new();
    for e in entries {
        *by_currency.entry(e.currency.clone()).or_insert(0.0) += e.converted_price;
    }
    by_currency.values().sum()
}

pub(super) type TextMethods = Vec<(String, Vec<(String, f64)>)>;

/// Text sorts both methods and their currencies lexically.
pub(super) fn text_methods(entries: &[Entry]) -> TextMethods {
    // method -> (currency -> total), currency insertion order.
    let mut method_groups: Vec<(String, Vec<(String, f64)>)> = Vec::new();
    for entry in entries {
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
    for (_, totals) in &mut method_groups {
        totals.sort_by(|a, b| a.0.cmp(&b.0));
    }
    method_groups
}

pub(super) struct MethodTotals {
    pub(super) total: f64,
    pub(super) currencies: Vec<String>,
    pub(super) by_currency: Vec<(String, f64)>,
}

/// JSON retains first-appearance order and omits unconvertible method amounts.
pub(super) fn json_methods(
    entries: &[Entry],
    currency: Option<&str>,
    converted_amounts: Option<&[Option<f64>]>,
) -> Vec<(String, MethodTotals)> {
    // byMethod (only when requested)
    let mut by_method_entries: Vec<(String, MethodTotals)> = Vec::new();
    for (i, entry) in entries.iter().enumerate() {
        let m = entry
            .payment_method
            .clone()
            .unwrap_or_else(|| "unspecified".to_string());
        let currency_for = if let Some(fc) = currency {
            fc.to_string()
        } else {
            entry.currency.clone()
        };
        let mut amount = entry.converted_price;
        if let (Some(_), Some(conv)) = (currency, converted_amounts) {
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
    by_method_entries
}
