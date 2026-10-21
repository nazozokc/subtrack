use chrono::Datelike;

use super::{days_in_month, occurrences_per_year, today};

pub fn period_factor(from: &str, to: &str) -> f64 {
    occurrences_per_year(from) / occurrences_per_year(to)
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
