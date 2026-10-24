//! Upcoming-billing calculation — port of `domain/billing.ts`
//! (upcomingWithinDays) + `calcUpcomingWithCurrency` (embedded rates never
//! fail). Pure over subscriptions: no DB access, no rendering.

use crate::date::{calculate_next_billing_inclusive, today};
use crate::fx::{fetch_rates, try_convert};
use crate::models::Subscription;
use crate::price::round_currency;
use chrono::NaiveDate;

#[derive(Clone)]
pub struct Entry {
    pub idx: usize,
    pub next_date: NaiveDate,
    pub amount: f64,
}

/// Port of `upcomingWithinDays(listActive(), days)`.
pub fn upcoming_within_days(subs: &[Subscription], days: i64) -> Vec<Entry> {
    if subs.is_empty() {
        return Vec::new();
    }
    let now = today();
    let end = now + chrono::Duration::days(days);
    let mut entries: Vec<Entry> = Vec::new();
    for (idx, sub) in subs.iter().enumerate() {
        let next =
            calculate_next_billing_inclusive(sub.billing_day, &sub.created_at, &sub.cycle, now);
        if next >= now && next <= end {
            entries.push(Entry {
                idx,
                next_date: next,
                amount: sub.price,
            });
        }
    }
    entries.sort_by_key(|e| e.next_date);
    entries
}

/// Port of `calcUpcomingWithCurrency` (embedded rates never fail).
pub fn upcoming_with_currency_internal(
    subs: &[Subscription],
    days: i64,
    target: Option<&str>,
) -> (Vec<Entry>, Vec<Subscription>) {
    upcoming_with_currency(subs, days, target)
}

/// Currency-convert upcoming entries to `target` (no-op when `target` is
/// `None`); keeps `display` aligned with the input order via `idx`.
pub fn upcoming_with_currency(
    subs: &[Subscription],
    days: i64,
    target: Option<&str>,
) -> (Vec<Entry>, Vec<Subscription>) {
    let mut entries = upcoming_within_days(subs, days);
    let mut display: Vec<Subscription> = subs.to_vec();
    let Some(target) = target else {
        return (entries, display);
    };
    if entries.is_empty() {
        return (entries, display);
    }
    let rates = fetch_rates();
    for entry in entries.iter_mut() {
        let sub = &subs[entry.idx];
        if let Some(converted) = try_convert(entry.amount, &sub.currency, target, &rates.rates) {
            entry.amount = round_currency(converted);
            let mut s = sub.clone();
            s.price = round_currency(converted);
            s.currency = target.to_string();
            display[entry.idx] = s;
        }
    }
    (entries, display)
}
