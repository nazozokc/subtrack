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
