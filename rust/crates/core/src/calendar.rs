//! Monthly billing-calendar calculation — port of the billing day math in
//! `commands/calendar.ts` (the rendering stays in the CLI crate). Pure over
//! subscriptions: no DB access.

use chrono::Datelike;

use crate::date::{
    add_day_periods, clamp_day, cycle_days, date_with_clamped_day, days_in_month, div_ceil_js,
    is_day_cycle, to_date, whole_days_between,
};
use crate::domain::not_cancelled;
use crate::models::Subscription;

pub struct CalendarSub {
    pub name: String,
    pub price: f64,
    pub currency: String,
    pub status: String,
    pub id: i64,
}

pub struct CalendarEntry {
    pub day: u32,
    pub subs: Vec<CalendarSub>,
}

/// Billing days of a subscription within a given month (1-31).
fn billing_days_in_month(sub: &Subscription, year: i32, month: u32) -> Vec<u32> {
    let anchor_date = to_date(&sub.created_at).unwrap_or(date_with_clamped_day(year, month - 1, 1));
    let anchor_month = anchor_date.month0() as i32;
    let day = sub.billing_day.unwrap_or(anchor_date.day() as i64) as u32;
    let clamped_day = clamp_day(day, year, month);

    if is_day_cycle(&sub.cycle) || sub.cycle == "weekly" || sub.cycle == "bi-weekly" {
        let days = cycle_days(&sub.cycle)
            .or(if sub.cycle == "weekly" {
                Some(7)
            } else {
                Some(14)
            })
            .unwrap_or(7);
        return day_cycle_days_in_month(anchor_date, day, days, year, month);
    }
    match sub.cycle.as_str() {
        "monthly" => vec![clamped_day],
        "yearly" => {
            if anchor_month == month as i32 - 1 {
                vec![clamped_day]
            } else {
                Vec::new()
            }
        }
        "quarterly" => {
            let diff = ((month as i32 - 1 - anchor_month) % 12 + 12) % 12;
            if diff % 3 == 0 {
                vec![clamped_day]
            } else {
                Vec::new()
            }
        }
        "semi-annual" => {
            let diff = ((month as i32 - 1 - anchor_month) % 12 + 12) % 12;
            if diff % 6 == 0 {
                vec![clamped_day]
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

fn day_cycle_days_in_month(
    anchor_date: chrono::NaiveDate,
    anchor_day: u32,
    days: i64,
    year: i32,
    month: u32,
) -> Vec<u32> {
    let anchor = date_with_clamped_day(anchor_date.year(), anchor_date.month() - 1, anchor_day);
    let start = chrono::NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let end = chrono::NaiveDate::from_ymd_opt(year, month, days_in_month(year, month)).unwrap();
    let k_start = 0.max(div_ceil_js(whole_days_between(anchor, start), days));
    let k_end = whole_days_between(anchor, end).div_euclid(days);
    let mut result = Vec::new();
    for k in k_start..=k_end {
        result.push(add_day_periods(anchor, days, k).day());
    }
    result
}

/// Calendar entries (billing day → rows) for a month, from a pre-fetched
/// subscription list (cancelled statuses are filtered here).
pub fn entries_from_subs(subs: &[Subscription], month: u32, year: i32) -> Vec<CalendarEntry> {
    let mut day_map: std::collections::BTreeMap<u32, Vec<CalendarSub>> =
        std::collections::BTreeMap::new();
    for sub in subs.iter().filter(|s| not_cancelled(&s.status)) {
        for day in billing_days_in_month(sub, year, month) {
            day_map.entry(day).or_default().push(CalendarSub {
                name: sub.name.clone(),
                price: sub.price,
                currency: sub.currency.clone(),
                status: sub.status.clone(),
                id: sub.id,
            });
        }
    }
    day_map
        .into_iter()
        .map(|(day, subs)| CalendarEntry { day, subs })
        .collect()
}
