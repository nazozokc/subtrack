//! Date helpers — port of `@subtrack/lib/date.ts` + `domain/billing.ts`
//! next-billing logic, backed by chrono NaiveDate (UTC/local fixed).

use chrono::{Datelike, Local, NaiveDate};

mod billing;
mod period;

#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub use billing::{
    calculate_next_billing, calculate_next_billing_inclusive, div_ceil_js, get_billing_day,
    last_day_of_month_string, next_date_for_cycle, next_date_for_cycle_inclusive,
    today_string_days_ago,
};
pub use period::{get_period_date_range, get_previous_period_date_range, period_factor};

pub const SHORT_MONTH_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Today as a NaiveDate. When SUBTRACK_TODAY=YYYY-MM-DD is set (Rust-only
/// harness env), that value is used instead of the real clock.
pub fn today() -> NaiveDate {
    if let Ok(v) = std::env::var("SUBTRACK_TODAY")
        && let Ok(d) = NaiveDate::parse_from_str(&v, "%Y-%m-%d")
    {
        return d;
    }
    Local::now().date_naive()
}

/// Today's date as YYYY-MM-DD string (mirror of date.ts `today()`).
pub fn today_string() -> String {
    today().format("%Y-%m-%d").to_string()
}

/// Parse YYYY-MM-DD to a local Date (mirror of `toDate`).
pub fn to_date(date_str: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()
}

/// Format a Date as YYYY-MM-DD (mirror of `formatDate`).
pub fn format_date(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

/// Format a Date as a short display string, e.g. "Jan 5".
pub fn format_short_date(d: NaiveDate) -> String {
    format!("{} {}", SHORT_MONTH_NAMES[(d.month0()) as usize], d.day())
}

/// Days in a month. `month` is 1-12 (human convention).
pub fn days_in_month(year: i32, month: u32) -> u32 {
    let (y, m) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let first_next = NaiveDate::from_ymd_opt(y, m, 1).unwrap();
    (first_next - chrono::Duration::days(1)).day()
}

/// Clamp a day to the length of the given month. `month` is 1-12.
pub fn clamp_day(day: u32, year: i32, month: u32) -> u32 {
    day.min(days_in_month(year, month))
}

/// Build a Date with the given day clamped to the month length.
///
/// `month` is 0-based (JS convention). In JS, `new Date(y, m, d)` rolls
/// month indices at 12 or greater into the following years — the monthly
/// next-billing branch relies on that when `month + 1` exceeds December,
/// so mirror the rollover here.
pub fn date_with_clamped_day(year: i32, month: u32, day: u32) -> NaiveDate {
    let (year, month0) = if month >= 12 {
        let total = year as i64 * 12 + month as i64;
        ((total / 12) as i32, (total % 12) as u32)
    } else {
        (year, month)
    };
    NaiveDate::from_ymd_opt(year, month0 + 1, clamp_day(day, year, month0 + 1)).unwrap()
}

/// Whole local-calendar days between two dates.
pub fn whole_days_between(from: NaiveDate, to: NaiveDate) -> i64 {
    (to - from).num_days()
}

/// Date `k` billing-periods after the anchor, advanced by calendar days.
pub fn add_day_periods(anchor: NaiveDate, days: i64, k: i64) -> NaiveDate {
    anchor + chrono::Duration::days(k * days)
}

/// Anchor date of an every-N-days cycle: the billing day of the createdAt
/// month (day clamped to the month length).
fn day_cycle_anchor(anchor_date: NaiveDate, anchor_day: u32) -> NaiveDate {
    date_with_clamped_day(anchor_date.year(), anchor_date.month0(), anchor_day)
}

/// Next occurrence of an every-N-days cycle at or after `from_date`.
///
/// `strict` mirrors the reference difference between call sites:
/// - `display.ts` (list) passes an untruncated `new Date()`, i.e. an instant
///   strictly after midnight, so a `from_date` on a billing date bills the
///   NEXT period.
/// - `upcomingWithinDays` truncates `from` to midnight (`setHours(0,0,0,0)`),
///   so it is inclusive of an exact billing date.
pub fn next_day_cycle_date(
    anchor_date: NaiveDate,
    anchor_day: u32,
    days: i64,
    from_date: NaiveDate,
    strict: bool,
) -> NaiveDate {
    let anchor = day_cycle_anchor(anchor_date, anchor_day);
    let diff = (from_date - anchor).num_days();
    let periods = if diff < 0 || (diff == 0 && !strict) {
        0
    } else if diff % days == 0 {
        diff / days + if strict { 1 } else { 0 }
    } else {
        (diff + days - 1) / days
    };
    add_day_periods(anchor, days, periods)
}

/// Named billing cycle presets.
pub fn occurrences_per_year(cycle: &str) -> f64 {
    if let Some(days) = cycle_days(cycle) {
        return 365.0 / days as f64;
    }
    match cycle {
        "weekly" => 52.0,
        "bi-weekly" => 26.0,
        "monthly" => 12.0,
        "quarterly" => 4.0,
        "semi-annual" => 2.0,
        "yearly" => 1.0,
        _ => 12.0,
    }
}

/// Days of a day-based cycle ("3d" → 3), or None.
pub fn cycle_days(cycle: &str) -> Option<i64> {
    if let Some(rest) = cycle.strip_suffix('d')
        && let Ok(n) = rest.parse::<i64>()
        && (1..=365).contains(&n)
    {
        return Some(n);
    }
    None
}

pub fn is_day_cycle(cycle: &str) -> bool {
    cycle_days(cycle).is_some()
}

/// Human-readable cycle label: "every 3 days", or the preset name.
pub fn format_cycle(cycle: &str) -> String {
    if let Some(days) = cycle_days(cycle) {
        if days == 1 {
            format!("every {} day", days)
        } else {
            format!("every {} days", days)
        }
    } else {
        cycle.to_string()
    }
}

/// Days until a target date relative to today. Returns 0 for today,
/// negative for past dates (mirror of `daysUntil`).
pub fn days_until(target: NaiveDate) -> i64 {
    (target - today()).num_days()
}
