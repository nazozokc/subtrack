use super::{ApiUsage, calculate, render};
use subtrack_core::models::Subscription;

fn subscription(id: i64, price: f64, currency: &str, method: Option<&str>) -> Subscription {
    Subscription {
        id,
        name: format!("Service {id}"),
        price,
        currency: currency.to_string(),
        cycle: "monthly".to_string(),
        status: "active".to_string(),
        billing_day: None,
        created_at: "2026-10-08".to_string(),
        notes: None,
        payment_method: method.map(str::to_string),
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

#[test]
fn period_amounts_keep_row_order_indices_and_unrounded_values() {
    let cycles = [
        ("weekly", 52.0),
        ("bi-weekly", 26.0),
        ("monthly", 12.0),
        ("quarterly", 4.0),
        ("semi-annual", 2.0),
        ("yearly", 1.0),
    ];
    let subs: Vec<_> = cycles
        .iter()
        .enumerate()
        .map(|(i, (cycle, _))| {
            let mut sub = subscription(i as i64, 0.01, "USD", None);
            sub.cycle = cycle.to_string();
            sub
        })
        .collect();
    for (period, occurrences) in cycles {
        let entries = calculate::entries(&subs, period);
        for (i, entry) in entries.iter().enumerate() {
            assert_eq!(entry.idx, i);
            assert_eq!(entry.converted_price, 0.01 * (cycles[i].1 / occurrences));
        }
    }
}

#[test]
fn currency_totals_sort_keys_and_exclude_only_cancelled() {
    let mut subs = vec![
        subscription(1, 3.0, "USD", None),
        subscription(2, 2.0, "EUR", None),
        subscription(3, 100.0, "JPY", None),
        subscription(4, 0.004, "USD", None),
        subscription(5, 0.004, "USD", None),
    ];
    subs[1].status = "paused".to_string();
    subs[2].status = "cancelled".to_string();
    let totals = calculate::currency_totals(&subs, "monthly");
    assert_eq!(
        totals.keys().map(String::as_str).collect::<Vec<_>>(),
        ["EUR", "USD"]
    );
    assert_eq!(totals["EUR"], 2.0);
    assert!((totals["USD"] - 3.008).abs() < 1e-12);
}

#[test]
fn text_sorts_methods_and_currencies_but_json_keeps_encounter_order() {
    let subs = vec![
        subscription(1, 1.0, "USD", Some("z card")),
        subscription(2, 2.0, "EUR", Some("z card")),
        subscription(3, 3.0, "JPY", Some("a cash")),
        subscription(4, 4.0, "USD", None),
        subscription(5, 5.0, "EUR", Some("unspecified")),
        subscription(6, 6.0, "USD", Some("")),
    ];
    let entries = calculate::entries(&subs, "monthly");
    let text = calculate::text_methods(&entries);
    assert_eq!(
        text.iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        ["", "a cash", "unspecified", "z card"]
    );
    assert_eq!(
        text[3].1,
        [("EUR".to_string(), 2.0), ("USD".to_string(), 1.0)]
    );

    let json = calculate::json_methods(&entries, None, None);
    assert_eq!(
        json.iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        ["z card", "a cash", "unspecified", ""]
    );
    assert_eq!(json[0].1.currencies, ["USD", "EUR"]);
    assert_eq!(
        json[0].1.by_currency,
        [("USD".to_string(), 1.0), ("EUR".to_string(), 2.0)]
    );
    assert_eq!(json[2].1.total, 9.0);
    assert_eq!(json[3].1.total, 6.0);
}

#[test]
fn json_total_preserves_currency_grouped_float_summation() {
    let subs = vec![
        subscription(1, 1e16, "AAA", None),
        subscription(2, 1.0, "BBB", None),
        subscription(3, -1e16, "AAA", None),
    ];
    let entries = calculate::entries(&subs, "monthly");
    assert_eq!(calculate::json_total(&entries), 1.0);
    assert_eq!(entries.iter().map(|e| e.converted_price).sum::<f64>(), 0.0);
}

#[test]
fn missing_conversions_are_excluded_without_dropping_zero_or_reordering_methods() {
    let subs = vec![
        subscription(1, 100.0, "XXX", Some("later")),
        subscription(2, 0.0, "USD", Some("zero")),
        subscription(3, 2.0, "EUR", Some("later")),
        subscription(4, 100.0, "XXX", Some("missing")),
    ];
    let entries = calculate::entries(&subs, "monthly");
    let converted = [None, Some(0.0), Some(2.345), None];
    assert_eq!(calculate::converted_total(&converted), 2.345);
    let methods = calculate::json_methods(&entries, Some("JPY"), Some(&converted));
    assert_eq!(
        methods.iter().map(|(m, _)| m.as_str()).collect::<Vec<_>>(),
        ["zero", "later"]
    );
    assert_eq!(methods[0].1.total, 0.0);
    assert_eq!(methods[1].1.total, 2.345);
    assert_eq!(methods[1].1.currencies, ["JPY"]);
    assert_eq!(methods[1].1.by_currency, [("JPY".to_string(), 2.345)]);
}

#[test]
fn json_bytes_keep_original_subscription_units_and_raw_api_cents() {
    let subs = [subscription(7, 10.0, "USD", Some("card"))];
    let entries = calculate::entries(&subs, "monthly");
    let methods = calculate::json_methods(&entries, Some("JPY"), Some(&[Some(1500.0)]));
    let api = ApiUsage {
        total: 251,
        by_provider: vec![
            ("z provider".to_string(), 200),
            ("a provider".to_string(), 51),
        ],
    };
    let output = render::json(
        &subs,
        "monthly",
        Some("JPY"),
        Some(&api),
        &entries,
        1500.0,
        methods,
    );
    assert_eq!(
        output,
        concat!(
            "{\n",
            "  \"period\": \"monthly\",\n",
            "  \"total\": 1500,\n",
            "  \"currency\": \"JPY\",\n",
            "  \"subscriptions\": [\n",
            "    {\n",
            "      \"id\": 7,\n",
            "      \"name\": \"Service 7\",\n",
            "      \"price\": 10,\n",
            "      \"currency\": \"USD\",\n",
            "      \"cycle\": \"monthly\",\n",
            "      \"status\": \"active\",\n",
            "      \"periodPrice\": 10\n",
            "    }\n",
            "  ],\n",
            "  \"apiUsage\": {\n",
            "    \"total\": 251,\n",
            "    \"byProvider\": [\n",
            "      {\n",
            "        \"provider\": \"z provider\",\n",
            "        \"total\": 200\n",
            "      },\n",
            "      {\n",
            "        \"provider\": \"a provider\",\n",
            "        \"total\": 51\n",
            "      }\n",
            "    ]\n",
            "  },\n",
            "  \"byMethod\": {\n",
            "    \"card\": {\n",
            "      \"total\": 1500,\n",
            "      \"currencies\": [\"JPY\"],\n",
            "      \"byCurrency\": {\n",
            "        \"JPY\": 1500\n",
            "      }\n",
            "    }\n",
            "  }\n",
            "}\n",
        )
    );
}

#[test]
fn json_rounds_after_aggregation_not_before() {
    let subs = [
        subscription(1, 0.004, "USD", None),
        subscription(2, 0.004, "USD", None),
    ];
    let entries = calculate::entries(&subs, "monthly");
    let methods = calculate::json_methods(&entries, None, None);
    let output = render::json(
        &subs,
        "monthly",
        None,
        None,
        &entries,
        calculate::json_total(&entries),
        methods,
    );
    let value: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(value["total"], 0.01);
    assert_eq!(value["byMethod"]["unspecified"]["total"], 0.01);
    assert_eq!(value["byMethod"]["unspecified"]["byCurrency"]["USD"], 0.01);
    assert_eq!(value["subscriptions"][0]["periodPrice"], 0);
    assert_eq!(value["subscriptions"][1]["periodPrice"], 0);
    assert!(value["currency"].is_null());
}

#[test]
fn json_omits_nonpositive_api_usage_and_fully_unconvertible_methods() {
    let subs = [subscription(1, 10.0, "XXX", Some("missing"))];
    let entries = calculate::entries(&subs, "monthly");
    for total in [0, -100] {
        let api = ApiUsage {
            total,
            by_provider: vec![("provider".to_string(), total)],
        };
        let methods = calculate::json_methods(&entries, Some("USD"), Some(&[None]));
        let output = render::json(
            &subs,
            "monthly",
            Some("USD"),
            Some(&api),
            &entries,
            0.0,
            methods,
        );
        let value: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(value["total"], 0);
        assert_eq!(value["subscriptions"].as_array().unwrap().len(), 1);
        assert!(value.get("apiUsage").is_none());
        assert!(value.get("byMethod").is_none());
    }
}

#[test]
fn empty_json_has_only_period_total_and_subscriptions() {
    assert_eq!(
        render::empty_json("yearly"),
        "{\n  \"period\": \"yearly\",\n  \"total\": 0,\n  \"subscriptions\": []\n}\n"
    );
}
