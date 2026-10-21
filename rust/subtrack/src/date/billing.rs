use chrono::{Datelike, NaiveDate};

use super::{cycle_days, date_with_clamped_day, is_day_cycle, next_day_cycle_date, to_date, today};

// ── next-billing-date helpers (domain/billing.ts) ─────────
pub fn get_billing_day(billing_day: Option<i64>, created_at: &str) -> u32 {
    if let Some(d) = billing_day {
        return d as u32;
    }
    to_date(created_at).map(|d| d.day()).unwrap_or(1)
}

/// Date of the k-th period occurrence anchored on `anchor_date`,
/// billed on `day` (clamped to the month length).
fn period_date(anchor_date: NaiveDate, period_months: i64, k: i64, day: u32) -> NaiveDate {
    let month_index = anchor_date.month0() as i64 + k * period_months;
    let year = anchor_date.year() + (month_index.div_euclid(12)) as i32;
    let month = month_index.rem_euclid(12);
    date_with_clamped_day(year, month as u32, day)
}

pub fn next_date_for_cycle(
    anchor_day: u32,
    anchor_date: NaiveDate,
    cycle: &str,
    from_date: NaiveDate,
) -> NaiveDate {
    next_date_for_cycle_impl(anchor_day, anchor_date, cycle, from_date, true)
}

/// Inclusive variant used by `upcomingWithinDays` (from truncated to midnight).
pub fn next_date_for_cycle_inclusive(
    anchor_day: u32,
    anchor_date: NaiveDate,
    cycle: &str,
    from_date: NaiveDate,
) -> NaiveDate {
    next_date_for_cycle_impl(anchor_day, anchor_date, cycle, from_date, false)
}

fn next_date_for_cycle_impl(
    anchor_day: u32,
    anchor_date: NaiveDate,
    cycle: &str,
    from_date: NaiveDate,
    strict: bool,
) -> NaiveDate {
    if is_day_cycle(cycle) || cycle == "weekly" || cycle == "bi-weekly" {
        let days = match cycle_days(cycle) {
            Some(d) => d,
            None => {
                if cycle == "weekly" {
                    7
                } else {
                    14
                }
            }
        };
        return next_day_cycle_date(anchor_date, anchor_day, days, from_date, strict);
    }
    let ge = |candidate: NaiveDate| {
        if strict {
            candidate > from_date
        } else {
            candidate >= from_date
        }
    };
    match cycle {
        "monthly" => {
            let candidate = date_with_clamped_day(from_date.year(), from_date.month0(), anchor_day);
            if ge(candidate) {
                return candidate;
            }
            date_with_clamped_day(from_date.year(), from_date.month0() + 1, anchor_day)
        }
        "yearly" => {
            let candidate =
                date_with_clamped_day(from_date.year(), anchor_date.month0(), anchor_day);
            if ge(candidate) {
                return candidate;
            }
            date_with_clamped_day(from_date.year() + 1, anchor_date.month0(), anchor_day)
        }
        "quarterly" | "semi-annual" => {
            let period_months = if cycle == "quarterly" { 3 } else { 6 };
            let mut k: i64 = 0;
            loop {
                let candidate = period_date(anchor_date, period_months, k, anchor_day);
                if ge(candidate) {
                    return candidate;
                }
                k += 1
            }
        }
        _ => from_date,
    }
}

pub fn calculate_next_billing(
    billing_day: Option<i64>,
    created_at: &str,
    cycle: &str,
    from_date: NaiveDate,
) -> NaiveDate {
    let anchor_date = to_date(created_at).unwrap_or(from_date);
    let day = get_billing_day(billing_day, created_at);
    next_date_for_cycle(day, anchor_date, cycle, from_date)
}

pub fn calculate_next_billing_inclusive(
    billing_day: Option<i64>,
    created_at: &str,
    cycle: &str,
    from_date: NaiveDate,
) -> NaiveDate {
    let anchor_date = to_date(created_at).unwrap_or(from_date);
    let day = get_billing_day(billing_day, created_at);
    next_date_for_cycle_inclusive(day, anchor_date, cycle, from_date)
}
/// JS Math.ceil(a / b) for i64 (ceil of a possibly-negative fraction).
///
/// Mirrors JS semantics exactly (division truncates toward zero, ceiling
/// rounds toward +inf). All call sites pass `b > 0`, but the function keeps
/// the general mapping:
/// - exact quotient (`a % b == 0`) yields `a / b`
/// - negative dividends already truncate toward zero, so `a / b` *is* ceil
/// - positive remainder of a positive dividend needs one extra unit
pub fn div_ceil_js(a: i64, b: i64) -> i64 {
    let q = a / b;
    if a % b == 0 || a < 0 { q } else { q + 1 }
}

/// Today minus `days`, formatted YYYY-MM-DD (UTC).
pub fn today_string_days_ago(days: i64) -> String {
    (today() - chrono::Duration::days(days))
        .format("%Y-%m-%d")
        .to_string()
}

/// Last day of 0-based month `month0` as "YYYY-MM-DD" (new Date(y, m+1, 0)).
pub fn last_day_of_month_string(year: i64, month0: i64) -> String {
    let (ny, nm) = if month0 + 1 == 12 {
        (year + 1, 1)
    } else {
        (year, month0 + 2)
    };
    match chrono::NaiveDate::from_ymd_opt(ny as i32, nm as u32, 1) {
        Some(d) => (d - chrono::Duration::days(1))
            .format("%Y-%m-%d")
            .to_string(),
        None => format!("{:04}-12-31", year),
    }
}
