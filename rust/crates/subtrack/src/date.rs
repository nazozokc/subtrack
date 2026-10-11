//! Date helpers — port of `@subtrack/lib/date.ts` + `domain/billing.ts`
//! next-billing logic, backed by chrono NaiveDate (UTC/local fixed).

use chrono::{Datelike, Local, NaiveDate};

pub const SHORT_MONTH_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Today as a NaiveDate. When SUBTRACK_TODAY=YYYY-MM-DD is set (Rust-only
/// harness env), that value is used instead of the real clock.
pub fn today() -> NaiveDate {
    if let Ok(v) = std::env::var("SUBTRACK_TODAY") {
        if let Ok(d) = NaiveDate::parse_from_str(&v, "%Y-%m-%d") {
            return d;
        }
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
    let (y, m) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    let first_next = NaiveDate::from_ymd_opt(y, m, 1).unwrap();
    (first_next - chrono::Duration::days(1)).day()
}

/// Clamp a day to the length of the given month. `month` is 1-12.
pub fn clamp_day(day: u32, year: i32, month: u32) -> u32 {
    day.min(days_in_month(year, month))
}

/// Build a Date with the given day clamped to the month length.
/// `month` is 0-based (JS convention).
pub fn date_with_clamped_day(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month + 1, clamp_day(day, year, month + 1)).unwrap()
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
    if let Some(rest) = cycle.strip_suffix('d') {
        if let Ok(n) = rest.parse::<i64>() {
            if (1..=365).contains(&n) {
                return Some(n)
            }
        }
    }
    None
}

pub fn is_day_cycle(cycle: &str) -> bool {
    cycle_days(cycle).is_some()
}

/// Days of `month` (1-12) on which an every-N-days cycle bills
/// (port of `dayCycleDaysInMonth`).
pub fn day_cycle_days_in_month(
    anchor_date: NaiveDate,
    anchor_day: u32,
    days: i64,
    year: i32,
    month: u32,
) -> Vec<u32> {
    let anchor = date_with_clamped_day(anchor_date.year(), anchor_date.month(), anchor_day);
    let start = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(year, month, days_in_month(year, month)).unwrap();
    let k_start = 0.max(div_ceil_js(whole_days_between(anchor, start), days));
    let k_end = whole_days_between(anchor, end).div_euclid(days);
    let mut result = Vec::new();
    for k in k_start..=k_end {
        result.push(add_day_periods(anchor, days, k).day());
    }
    result
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

/// Multiplier to convert a price from one cycle to another.
pub fn period_factor(from: &str, to: &str) -> f64 {
    occurrences_per_year(from) / occurrences_per_year(to)
}

/// Days until a target date relative to today. Returns 0 for today,
/// negative for past dates (mirror of `daysUntil`).
pub fn days_until(target: NaiveDate) -> i64 {
    (target - today()).num_days()
}

/// Port of `getPeriodDateRange`: [from, to] YYYY-MM-DD for a named period.
pub fn get_period_date_range(period: &str) -> (String, String) {
    let now = today();
    let y = now.year();
    let m = now.month0() as i32; // 0-based
    let d = now.day() as i32;
    let to = format!("{:04}-{:02}-{:02}", y, m + 1, d);

    match period {
        "monthly" => (format!("{:04}-{:02}-01", y, m + 1), to),
        "yearly" => (format!("{:04}-01-01", y), to),
        "weekly" => {
            let day = now.weekday().num_days_from_sunday() as i32;
            let diff = if day == 0 { 6 } else { day - 1 };
            let mon = now - chrono::Duration::days(diff as i64);
            (
                format!("{:04}-{:02}-{:02}", mon.year(), mon.month(), mon.day()),
                to,
            )
        }
        "bi-weekly" => {
            let two = now - chrono::Duration::days(13);
            (
                format!("{:04}-{:02}-{:02}", two.year(), two.month(), two.day()),
                to,
            )
        }
        "quarterly" => {
            let qs = (m / 3) * 3;
            (format!("{:04}-{:02}-01", y, qs + 1), to)
        }
        "semi-annual" => {
            let hs = (m / 6) * 6;
            (format!("{:04}-{:02}-01", y, hs + 1), to)
        }
        _ => (format!("{:04}-{:02}-01", y, m + 1), to),
    }
}

/// [from, to] range for the complete period before the current one
/// (port of `getPreviousPeriodDateRange`).
pub fn get_previous_period_date_range(period: &str) -> (String, String) {
    let now = today();
    let y = now.year();
    let m = now.month0() as i32; // 0-based

    match period {
        "monthly" => {
            let prev_m = if m == 0 { 11 } else { m - 1 };
            let prev_y = if m == 0 { y - 1 } else { y };
            let last_day = days_in_month(prev_y, (prev_m + 1) as u32);
            (
                format!("{:04}-{:02}-01", prev_y, prev_m + 1),
                format!("{:04}-{:02}-{:02}", prev_y, prev_m + 1, last_day),
            )
        }
        "yearly" => (format!("{:04}-01-01", y - 1), format!("{:04}-12-31", y - 1)),
        "weekly" => {
            let day = now.weekday().num_days_from_sunday() as i32;
            let diff = if day == 0 { 6 } else { day - 1 };
            let this_mon = now - chrono::Duration::days(diff as i64);
            let prev_mon = this_mon - chrono::Duration::days(7);
            let prev_sun = this_mon - chrono::Duration::days(1);
            (
                format!("{:04}-{:02}-{:02}", prev_mon.year(), prev_mon.month(), prev_mon.day()),
                format!("{:04}-{:02}-{:02}", prev_sun.year(), prev_sun.month(), prev_sun.day()),
            )
        }
        "bi-weekly" => {
            let day = now.weekday().num_days_from_sunday() as i32;
            let diff = if day == 0 { 6 } else { day - 1 };
            let this_mon = now - chrono::Duration::days(diff as i64);
            let prev_start = this_mon - chrono::Duration::days(14);
            let prev_end = this_mon - chrono::Duration::days(1);
            (
                format!("{:04}-{:02}-{:02}", prev_start.year(), prev_start.month(), prev_start.day()),
                format!("{:04}-{:02}-{:02}", prev_end.year(), prev_end.month(), prev_end.day()),
            )
        }
        "quarterly" => {
            let current_q = (m / 3) * 3;
            let prev_q_start = current_q - 3;
            let q_y = if prev_q_start < 0 { y - 1 } else { y };
            let q_m = ((prev_q_start % 12) + 12) % 12;
            let last_day_q = days_in_month(q_y, (q_m + 3) as u32);
            (
                format!("{:04}-{:02}-01", q_y, q_m + 1),
                format!("{:04}-{:02}-{:02}", q_y, q_m + 3, last_day_q),
            )
        }
        "semi-annual" => {
            let current_h = (m / 6) * 6;
            let prev_h_start = current_h - 6;
            let h_y = if prev_h_start < 0 { y - 1 } else { y };
            let h_m = ((prev_h_start % 12) + 12) % 12;
            let last_day_h = days_in_month(h_y, (h_m + 6) as u32);
            (
                format!("{:04}-{:02}-01", h_y, h_m + 1),
                format!("{:04}-{:02}-{:02}", h_y, h_m + 6, last_day_h),
            )
        }
        _ => {
            let prev_m = if m == 0 { 11 } else { m - 1 };
            let prev_y = if m == 0 { y - 1 } else { y };
            let last_day = days_in_month(prev_y, (prev_m + 1) as u32);
            (
                format!("{:04}-{:02}-01", prev_y, prev_m + 1),
                format!("{:04}-{:02}-{:02}", prev_y, prev_m + 1, last_day),
            )
        }
    }
}

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
        return next_day_cycle_date(anchor_date, anchor_day, days, from_date, strict)
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
            let candidate =
                date_with_clamped_day(from_date.year(), from_date.month0(), anchor_day);
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
pub fn div_ceil_js(a: i64, b: i64) -> i64 {
    if a % b == 0 {
        a / b
    } else if a < 0 {
        a / b
    } else {
        a / b + 1
    }
}

/// Today minus `days`, formatted YYYY-MM-DD (UTC).
pub fn today_string_days_ago(days: i64) -> String {
    (today() - chrono::Duration::days(days)).format("%Y-%m-%d").to_string()
}

/// Last day of 0-based month `month0` as "YYYY-MM-DD" (new Date(y, m+1, 0)).
pub fn last_day_of_month_string(year: i64, month0: i64) -> String {
    let (ny, nm) = if month0 + 1 == 12 { (year + 1, 1) } else { (year, month0 + 2) };
    match chrono::NaiveDate::from_ymd_opt(ny as i32, nm as u32, 1) {
        Some(d) => (d - chrono::Duration::days(1)).format("%Y-%m-%d").to_string(),
        None => format!("{:04}-12-31", year),
    }
}
