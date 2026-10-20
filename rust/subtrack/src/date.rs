//! Date helpers — port of `@subtrack/lib/date.ts` + `domain/billing.ts`
//! next-billing logic, backed by chrono NaiveDate (UTC/local fixed).

use chrono::{Datelike, Local, NaiveDate};

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
                format!(
                    "{:04}-{:02}-{:02}",
                    prev_mon.year(),
                    prev_mon.month(),
                    prev_mon.day()
                ),
                format!(
                    "{:04}-{:02}-{:02}",
                    prev_sun.year(),
                    prev_sun.month(),
                    prev_sun.day()
                ),
            )
        }
        "bi-weekly" => {
            let day = now.weekday().num_days_from_sunday() as i32;
            let diff = if day == 0 { 6 } else { day - 1 };
            let this_mon = now - chrono::Duration::days(diff as i64);
            let prev_start = this_mon - chrono::Duration::days(14);
            let prev_end = this_mon - chrono::Duration::days(1);
            (
                format!(
                    "{:04}-{:02}-{:02}",
                    prev_start.year(),
                    prev_start.month(),
                    prev_start.day()
                ),
                format!(
                    "{:04}-{:02}-{:02}",
                    prev_end.year(),
                    prev_end.month(),
                    prev_end.day()
                ),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn days_in_month_counts() {
        assert_eq!(days_in_month(2026, 1), 31);
        assert_eq!(days_in_month(2026, 2), 28);
        assert_eq!(days_in_month(2024, 2), 29); // leap year
        assert_eq!(days_in_month(2026, 4), 30);
        assert_eq!(days_in_month(2026, 12), 31);
    }

    #[test]
    fn clamp_day_caps_to_month_length() {
        assert_eq!(clamp_day(31, 2026, 2), 28);
        assert_eq!(clamp_day(30, 2024, 2), 29);
        assert_eq!(clamp_day(15, 2026, 4), 15);
    }

    #[test]
    fn date_with_clamped_day_rolls_like_js() {
        // JS `new Date(2026, 12, 31)` = 2027-01-31
        assert_eq!(date_with_clamped_day(2026, 12, 31), d(2027, 1, 31));
        // clamping still applies after the rollover
        assert_eq!(date_with_clamped_day(2026, 12, 32), d(2027, 1, 31));
        assert_eq!(date_with_clamped_day(2026, 1, 31), d(2026, 2, 28));
    }

    #[test]
    fn to_date_rejects_bogus_values() {
        assert_eq!(to_date("2026-10-08"), Some(d(2026, 10, 8)));
        assert_eq!(to_date("2026-02-30"), None);
        assert_eq!(to_date("not-a-date"), None);
        assert_eq!(to_date(""), None);
    }

    #[test]
    fn format_short_date_uses_month_names() {
        assert_eq!(format_short_date(d(2026, 10, 5)), "Oct 5");
        assert_eq!(format_short_date(d(2026, 1, 31)), "Jan 31");
    }

    #[test]
    fn whole_days_between_is_calendar_exact() {
        assert_eq!(whole_days_between(d(2026, 10, 8), d(2026, 10, 8)), 0);
        // Oct 8 → Feb 28 next year
        assert_eq!(whole_days_between(d(2026, 10, 8), d(2027, 2, 28)), 143);
    }

    #[test]
    fn cycle_days_parses_1_to_365() {
        assert_eq!(cycle_days("3d"), Some(3));
        assert_eq!(cycle_days("365d"), Some(365));
        assert_eq!(cycle_days("366d"), None);
        assert_eq!(cycle_days("0d"), None);
        assert_eq!(cycle_days("xd"), None);
        assert_eq!(cycle_days("monthly"), None);
        assert!(is_day_cycle("7d"));
        assert!(!is_day_cycle("weekly"));
    }

    #[test]
    fn format_cycle_labels_day_cycles() {
        assert_eq!(format_cycle("3d"), "every 3 days");
        assert_eq!(format_cycle("1d"), "every 1 day");
        assert_eq!(format_cycle("monthly"), "monthly");
    }

    #[test]
    fn occurrences_per_year_and_period_factor() {
        assert_eq!(occurrences_per_year("weekly"), 52.0);
        assert_eq!(occurrences_per_year("bi-weekly"), 26.0);
        assert_eq!(occurrences_per_year("monthly"), 12.0);
        assert_eq!(occurrences_per_year("quarterly"), 4.0);
        assert_eq!(occurrences_per_year("semi-annual"), 2.0);
        assert_eq!(occurrences_per_year("yearly"), 1.0);
        // day cycles: 365 / n
        assert!((occurrences_per_year("7d") - 365.0 / 7.0).abs() < 1e-9);
        // unknown cycles fall back to monthly
        assert_eq!(occurrences_per_year("???"), 12.0);
        assert!((period_factor("monthly", "yearly") - 12.0).abs() < 1e-9);
        assert!((period_factor("7d", "weekly") - (365.0 / 7.0) / 52.0).abs() < 1e-9);
    }

    #[test]
    fn next_day_cycle_strict_vs_inclusive() {
        let anchor = d(2026, 9, 10);
        let from_on_billing = d(2026, 10, 10);
        // strict (list view with clock time): a billing date bills the NEXT period
        assert_eq!(
            next_day_cycle_date(anchor, 10, 30, from_on_billing, true),
            d(2026, 11, 9)
        );
        // inclusive (upcoming view at midnight): shows today's billing date
        assert_eq!(
            next_day_cycle_date(anchor, 10, 30, from_on_billing, false),
            d(2026, 10, 10)
        );
        // mid-period rolls to the next boundary
        assert_eq!(
            next_day_cycle_date(anchor, 10, 30, d(2026, 10, 5), false),
            d(2026, 10, 10)
        );
    }

    #[test]
    fn next_day_cycle_anchors_day_in_created_month() {
        // anchor 2026-09-20 + 30d cycle: next boundary after Oct 5 is Oct 20
        assert_eq!(
            next_day_cycle_date(d(2026, 9, 20), 20, 30, d(2026, 10, 5), true),
            d(2026, 10, 20)
        );
    }

    #[test]
    fn monthly_next_billing_defaults_to_created_day() {
        assert_eq!(
            calculate_next_billing(None, "2026-01-12", "monthly", d(2026, 10, 8)),
            d(2026, 10, 12)
        );
    }

    #[test]
    fn monthly_next_billing_clamps_to_month_end() {
        assert_eq!(
            calculate_next_billing(Some(31), "2026-01-01", "monthly", d(2026, 11, 1)),
            d(2026, 11, 30)
        );
        // December rolls to January like `new Date(y, m + 1, d)`
        assert_eq!(
            calculate_next_billing(Some(15), "2026-05-04", "monthly", d(2026, 12, 20)),
            d(2027, 1, 15)
        );
    }

    #[test]
    fn yearly_next_billing_keeps_created_month() {
        assert_eq!(
            calculate_next_billing(None, "2025-07-20", "yearly", d(2026, 10, 8)),
            d(2027, 7, 20)
        );
        assert_eq!(
            calculate_next_billing(None, "2025-07-20", "yearly", d(2026, 6, 1)),
            d(2026, 7, 20)
        );
    }

    #[test]
    fn quarterly_next_billing_steps_anchor_quarters() {
        assert_eq!(
            calculate_next_billing(None, "2026-01-05", "quarterly", d(2026, 10, 8)),
            d(2027, 1, 5)
        );
        assert_eq!(
            calculate_next_billing(None, "2026-01-05", "semi-annual", d(2026, 10, 8)),
            d(2027, 1, 5)
        );
    }

    #[test]
    fn weekly_next_billing_advances_seven_days() {
        assert_eq!(
            calculate_next_billing(None, "2026-10-05", "weekly", d(2026, 10, 8)),
            d(2026, 10, 12)
        );
        assert_eq!(
            calculate_next_billing(None, "2026-10-05", "bi-weekly", d(2026, 10, 8)),
            d(2026, 10, 19)
        );
    }

    #[test]
    fn div_ceil_js_rounds_like_javascript() {
        assert_eq!(div_ceil_js(7, 3), 3);
        assert_eq!(div_ceil_js(-7, 3), -2);
        assert_eq!(div_ceil_js(6, 3), 2);
        assert_eq!(div_ceil_js(-6, 3), -2);
        assert_eq!(div_ceil_js(0, 3), 0);
    }

    #[test]
    fn last_day_of_month_string_handles_year_rollover() {
        assert_eq!(last_day_of_month_string(2026, 0), "2026-01-31");
        assert_eq!(last_day_of_month_string(2026, 1), "2026-02-28");
        assert_eq!(last_day_of_month_string(2024, 1), "2024-02-29");
        assert_eq!(last_day_of_month_string(2026, 11), "2026-12-31");
    }

    #[test]
    fn get_billing_day_falls_back_to_created_day() {
        assert_eq!(get_billing_day(Some(5), "2026-01-12"), 5);
        assert_eq!(get_billing_day(None, "2026-01-12"), 12);
        assert_eq!(get_billing_day(None, "bogus"), 1);
        assert_eq!(get_billing_day(None, "2026-02-30"), 1);
    }
}
