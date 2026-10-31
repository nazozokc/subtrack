use super::*;
use chrono::Duration;

fn subscription(id: i64, name: &str, price: f64, currency: &str, cycle: &str) -> Subscription {
    Subscription {
        id,
        name: name.to_string(),
        price,
        currency: currency.to_string(),
        cycle: cycle.to_string(),
        status: "active".to_string(),
        billing_day: None,
        created_at: "2020-01-01 00:00:00".to_string(),
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
        tags: Vec::new(),
    }
}

fn change(subscription_id: i64, changed_at: &str) -> PriceChange {
    PriceChange {
        id: subscription_id,
        subscription_id,
        old_price: None,
        new_price: 10.0,
        old_currency: None,
        new_currency: Some("USD".to_string()),
        changed_at: changed_at.to_string(),
        name: String::new(),
    }
}

fn date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 8).unwrap()
}

#[test]
fn empty_analysis_has_no_suggestions_or_totals() {
    let report = analyze(&[], &[], 15.0, date());
    assert!(report.cycle.is_empty());
    assert!(report.duplicates.is_empty());
    assert!(report.inactive.is_empty());
    assert!(report.cancelled.is_empty());
    assert!(report.total_yearly_savings.is_empty());
    assert!(report.cancelled_monthly.is_empty());
}

#[test]
fn cycle_optimization_preserves_status_filter_order_and_rounding() {
    let mut subs: Vec<_> = ["active", "cancelled", "paused", "archived", "unknown"]
        .iter()
        .enumerate()
        .map(|(i, status)| {
            let mut sub = subscription(i as i64, status, 9.99, "USD", "monthly");
            sub.status = status.to_string();
            sub
        })
        .collect();
    subs.push(subscription(5, "Yearly", 120.0, "USD", "yearly"));
    subs.push(subscription(6, "Free", 0.0, "USD", "monthly"));

    let report = analyze(&subs, &[], 15.0, date());
    assert_eq!(
        report
            .cycle
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        ["active", "archived", "unknown"]
    );
    for s in &report.cycle {
        assert_eq!(s.currency, "USD");
        assert_eq!(s.current_monthly, 9.99);
        assert_eq!(s.suggested_monthly, 8.49);
        assert_eq!(s.yearly_savings, 17.98);
    }
    assert_eq!(report.total_yearly_savings, [("USD".to_string(), 53.94)]);
}

#[test]
fn cycle_discount_endpoints_and_rounded_zero_savings_are_preserved() {
    let subs = [subscription(1, "Music", 10.0, "USD", "monthly")];
    let zero = analyze(&subs, &[], 0.0, date());
    assert!(zero.cycle.is_empty());
    assert!(zero.total_yearly_savings.is_empty());

    let full = analyze(&subs, &[], 100.0, date());
    assert_eq!(full.cycle[0].suggested_monthly, 0.0);
    assert_eq!(full.cycle[0].yearly_savings, 120.0);

    let tiny = analyze(&subs, &[], 0.001, date());
    // Filtering happens before rounding, so a positive saving can display as zero.
    assert_eq!(tiny.cycle.len(), 1);
    assert_eq!(tiny.cycle[0].yearly_savings, 0.0);
    assert_eq!(tiny.total_yearly_savings, [("USD".to_string(), 0.0)]);
}

#[test]
fn savings_totals_sum_rounded_suggestions_and_sort_currencies() {
    let subs = [
        subscription(1, "First", 0.03, "USD", "monthly"),
        subscription(2, "Second", 1200.0, "JPY", "monthly"),
        subscription(3, "Third", 0.03, "USD", "monthly"),
    ];
    let report = analyze(&subs, &[], 15.0, date());
    assert_eq!(
        report.total_yearly_savings,
        [("JPY".to_string(), 2160.0), ("USD".to_string(), 0.10)]
    );
}

#[test]
fn duplicate_totals_round_after_summing_and_category_uses_first_tags_only() {
    let mut subs = vec![
        subscription(1, "Music (Pro)", 10.0, "USD", "yearly"),
        subscription(2, "MUSIC-Basic", 10.0, "USD", "yearly"),
        subscription(3, "Music Plus", 20.0, "EUR", "monthly"),
    ];
    subs[0].tags = vec!["zeta".to_string(), "aardvark".to_string()];
    subs[1].tags = vec!["alpha".to_string()];
    for (i, status) in ["cancelled", "paused", "archived", "unknown"]
        .iter()
        .enumerate()
    {
        let mut sub = subscription(i as i64 + 4, "Music Premium", 99.0, "USD", "monthly");
        sub.status = status.to_string();
        subs.push(sub);
    }

    let report = analyze(&subs, &[], 15.0, date());
    assert_eq!(report.duplicates.len(), 1);
    let group = &report.duplicates[0];
    assert_eq!(group.names, ["Music (Pro)", "MUSIC-Basic", "Music Plus"]);
    assert_eq!(group.category, "alpha");
    assert_eq!(
        group.by_currency,
        [("EUR".to_string(), 20.0), ("USD".to_string(), 1.67)]
    );
}

#[test]
fn duplicate_groups_are_greedy_not_transitive_and_use_each_id_once() {
    let subs = [
        subscription(1, "alpha bravo", 10.0, "USD", "monthly"),
        subscription(2, "alpha bravo charlie", 10.0, "USD", "monthly"),
        subscription(3, "bravo charlie", 10.0, "USD", "monthly"),
        subscription(4, "bravo charlie duo", 10.0, "USD", "monthly"),
    ];
    let groups = analyze_duplicates(&subs);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].names, ["alpha bravo", "alpha bravo charlie"]);
    assert_eq!(groups[0].category, "alpha");
    assert_eq!(groups[1].names, ["bravo charlie", "bravo charlie duo"]);
    assert_eq!(groups[1].category, "bravo");
}

#[test]
fn duplicate_similarity_preserves_tokenization_and_inclusive_threshold() {
    assert_eq!(
        tokenize_name("MUSIC-(Pro) TV 日本 日本語"),
        ["music", "日本語"]
    );
    assert_eq!(tokenize_name("Music\tVideo"), ["music\tvideo"]);
    assert_eq!(jaccard_similarity(&[], &[]), 0.0);
    assert_eq!(
        jaccard_similarity(&tokenize_name("music music"), &tokenize_name("music")),
        1.0
    );
    let subs = [
        subscription(1, "Music Video", 10.0, "USD", "monthly"),
        subscription(2, "Music", 10.0, "USD", "monthly"),
        subscription(3, "Pro Plan", 10.0, "USD", "monthly"),
        subscription(4, "Basic Plus", 10.0, "USD", "monthly"),
    ];
    let groups = analyze_duplicates(&subs);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].names, ["Music Video", "Music"]);
}

#[test]
fn inactive_uses_explicit_today_and_inclusive_540_day_boundary() {
    let subs = [subscription(1, "Storage", 100.0, "EUR", "yearly")];
    let last = (date() - Duration::days(540)).to_string();
    let changes = [change(1, &format!("{last} 23:59:59"))];

    assert!(
        analyze(&subs, &changes, 15.0, date() - Duration::days(1))
            .inactive
            .is_empty()
    );
    let report = analyze(&subs, &changes, 15.0, date());
    assert_eq!(report.inactive.len(), 1);
    let s = &report.inactive[0];
    assert_eq!(s.id, 1);
    assert_eq!(s.name, "Storage");
    assert_eq!(s.price, 100.0);
    assert_eq!(s.currency, "EUR");
    assert_eq!(s.cycle, "yearly");
    assert_eq!(s.monthly, 8.33);
    assert_eq!(s.last_changed, last);
    assert_eq!(
        analyze(&subs, &changes, 15.0, date() + Duration::days(1))
            .inactive
            .len(),
        1
    );
}

#[test]
fn inactive_uses_latest_history_and_skips_missing_invalid_future_and_nonactive_rows() {
    let mut subs: Vec<_> = (1..=10)
        .map(|id| subscription(id, "Storage", 10.0, "USD", "monthly"))
        .collect();
    for (sub, status) in subs[6..]
        .iter_mut()
        .zip(["paused", "cancelled", "archived", "unknown"])
    {
        sub.status = status.to_string();
    }
    let changes = [
        change(1, "2026-10-01 00:00:00"),
        change(1, "2020-01-01 00:00:00"),
        change(2, "2020-01-01 00:00:00"),
        change(2, "2021-01-01 12:00:00"),
        // ID 3 has no history: created_at must not be used as a fallback.
        change(4, "2020-01-01 00:00:00"),
        change(4, "invalid"),
        change(5, "2027-01-01 00:00:00"),
        change(6, "2020-01-01T00:00:00Z"),
        change(7, "2020-01-01 00:00:00"),
        change(8, "2020-01-01 00:00:00"),
        change(9, "2020-01-01 00:00:00"),
        change(10, "2020-01-01 00:00:00"),
        change(999, "2020-01-01 00:00:00"),
    ];
    let report = analyze(&subs, &changes, 15.0, date());
    assert_eq!(report.inactive.len(), 1);
    assert_eq!(report.inactive[0].id, 2);
    assert_eq!(report.inactive[0].last_changed, "2021-01-01");
}

#[test]
fn cancelled_and_paused_normalize_monthly_costs_without_adding_to_savings() {
    let mut subs = [
        subscription(1, "Cancelled", 10.0, "USD", "yearly"),
        subscription(2, "Paused", 10.0, "USD", "yearly"),
        subscription(3, "Other currency", 3000.0, "JPY", "quarterly"),
    ];
    subs[0].status = "cancelled".to_string();
    subs[1].status = "paused".to_string();
    subs[2].status = "cancelled".to_string();
    let report = analyze(&subs, &[], 15.0, date());
    assert_eq!(report.cancelled.len(), 3);
    assert_eq!(report.cancelled[0].name, "Cancelled");
    assert_eq!(report.cancelled[0].price, 10.0);
    assert_eq!(report.cancelled[0].cycle, "yearly");
    assert_eq!(report.cancelled[0].monthly, 0.83);
    assert_eq!(report.cancelled[1].name, "Paused");
    assert_eq!(report.cancelled[1].monthly, 0.83);
    assert_eq!(report.cancelled[2].monthly, 1000.0);
    assert_eq!(
        report.cancelled_monthly,
        [("USD".to_string(), 1.66), ("JPY".to_string(), 1000.0)]
    );
    assert!(report.total_yearly_savings.is_empty());
    assert!(report.cycle.is_empty());
    assert!(report.duplicates.is_empty());
    assert!(report.inactive.is_empty());
}
