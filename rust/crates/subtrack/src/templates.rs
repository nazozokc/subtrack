//! `template` command (reads) — port of `template.ts` handleTemplate("list").

use crate::config::load_config;
use crate::logger::{self, info, log};
use crate::parser::ParsedArgs;

pub fn run_template_list(parsed: &ParsedArgs) -> i32 {
    let _ = parsed;
    let config = load_config();
    let templates = config
        .raw
        .get("templates")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    if templates.is_empty() {
        info("No templates found");
        return 0;
    }
    for (key, template) in &templates {
        let price = template
            .get("price")
            .and_then(|p| p.as_f64())
            .map(|p| crate::json::number(p))
            .unwrap_or_else(|| "0".to_string());
        let currency = template
            .get("currency")
            .and_then(|c| c.as_str())
            .unwrap_or("USD")
            .to_string();
        let cycle = template
            .get("cycle")
            .and_then(|c| c.as_str())
            .unwrap_or("monthly")
            .to_string();
        log(&format!("{}: {} {}/{}", key, price, currency, cycle));
    }
    0
}

/// Port of `handleTemplate("delete")`.
pub fn run_template_delete(parsed: &ParsedArgs) -> i32 {
    let name = parsed.str("name").unwrap_or_default();
    let config = load_config();
    let exists = config
        .raw
        .get("templates")
        .and_then(|v| v.as_object())
        .map(|t| t.contains_key(&name))
        .unwrap_or(false);
    if !exists {
        logger::fail(&format!("Template not found: {}", name));
        return 1;
    }
    let _ = crate::config::save_config_mut(|raw| {
        if let Some(serde_json::Value::Object(map)) = raw.get_mut("templates") {
            map.remove(&name);
        }
    });
    logger::success(&format!("Deleted template: {}", name));
    0
}

/// Port of `handleTemplate("use")`.
pub fn run_template_use(parsed: &ParsedArgs) -> i32 {
    let name = parsed.str("name").unwrap_or_default();
    let config = load_config();
    let templates = config
        .raw
        .get("templates")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let Some(template) = templates.get(&name).and_then(|v| v.as_object()) else {
        logger::fail(&format!("Template not found: {}", name));
        return 1;
    };

    let str_field = |k: &str| template.get(k).and_then(|v| v.as_str()).map(|s| s.to_string());
    let sub_name = str_field("name").unwrap_or_else(|| name.clone());
    let price = parsed
        .str("price")
        .and_then(|v| v.trim().parse::<f64>().ok())
        .or_else(|| template.get("price").and_then(|v| v.as_f64()))
        .unwrap_or(0.0);
    let currency = parsed
        .str("currency")
        .or_else(|| str_field("currency"))
        .unwrap_or_else(|| "USD".to_string());
    let cycle = parsed
        .str("cycle")
        .or_else(|| str_field("cycle"))
        .unwrap_or_else(|| "monthly".to_string());
    let tags: Vec<String> = match parsed.str("tags") {
        Some(v) => v
            .split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect(),
        None => template
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default(),
    };
    let billing_day = parsed
        .str("billingDay")
        .and_then(|v| v.trim().parse::<i64>().ok())
        .or_else(|| template.get("billingDay").and_then(|v| v.as_i64()));

    if !price.is_finite() || price <= 0.0 {
        logger::fail("Template price must be a positive number");
        return 1;
    }
    if !crate::validation::is_valid_currency(&currency) {
        logger::fail(&format!("Invalid currency: {}", currency));
        return 1;
    }
    if !crate::validation::is_valid_cycle(&cycle) {
        logger::fail(&format!("Invalid cycle: {}", cycle));
        return 1;
    }

    let conn = match crate::db::open_db() {
        Ok(c) => c,
        Err(e) => {
            logger::fail(&format!("Failed to add subscription: {}", e));
            return 1;
        }
    };
    let new = crate::db::NewSubscription {
        name: sub_name.clone(),
        price,
        currency,
        cycle,
        status: "active".to_string(),
        billing_day,
        notes: parsed.str("notes").or_else(|| str_field("notes")),
        payment_method: parsed.str("paymentMethod").or_else(|| str_field("paymentMethod")),
        contract_start: None,
        contract_end: None,
        auto_renewal: true,
        vendor_name: parsed.str("vendorName").or_else(|| str_field("vendorName")),
        vendor_url: parsed.str("vendorUrl").or_else(|| str_field("vendorUrl")),
        plan_tier: parsed.str("planTier").or_else(|| str_field("planTier")),
        discount_amount: None,
        discount_type: None,
        tags,
    };
    match crate::db::write_subscription(&conn, &new, true) {
        Ok(_) => {
            let _ = crate::db::add_audit_log(
                &conn,
                "template.use",
                None,
                None,
                Some(&name),
                true,
            );
            logger::success(&format!("Added subscription from template: {}", name));
            0
        }
        Err(e) => {
            logger::fail(&format!("Failed to add subscription: {}", e));
            1
        }
    }
}

/// Port of `handleTemplate("edit")`.
pub fn run_template_edit(parsed: &ParsedArgs) -> i32 {
    let name = parsed.str("name").unwrap_or_default();
    let config = load_config();
    let templates = config
        .raw
        .get("templates")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let Some(existing) = templates.get(&name).and_then(|v| v.as_object()) else {
        logger::fail(&format!("Template not found: {}", name));
        return 1;
    };

    let mut t = existing.clone();
    t.insert("name".into(), serde_json::Value::String(name.clone()));
    if let Some(v) = parsed.str("price") {
        if let Ok(n) = v.trim().parse::<f64>() {
            t.insert("price".into(), crate::json::num_value(n));
        }
    }
    for (key, field) in [
        ("currency", "currency"),
        ("cycle", "cycle"),
        ("notes", "notes"),
        ("paymentMethod", "paymentMethod"),
        ("vendorName", "vendorName"),
        ("vendorUrl", "vendorUrl"),
        ("planTier", "planTier"),
    ] {
        if let Some(v) = parsed.str(field) {
            if !v.trim().is_empty() {
                t.insert(key.into(), serde_json::Value::String(v.trim().to_string()));
            }
        }
    }
    if let Some(v) = parsed.str("tags") {
        let tags: Vec<serde_json::Value> = v
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .map(serde_json::Value::String)
            .collect();
        t.insert("tags".into(), serde_json::Value::Array(tags));
    }
    if let Some(v) = parsed.str("billingDay") {
        if let Ok(n) = v.trim().parse::<i64>() {
            t.insert("billingDay".into(), serde_json::Value::Number(n.into()));
        }
    }

    if let Err(e) = crate::config::save_config_mut(|raw| {
        let entry = raw
            .entry("templates")
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
        if let serde_json::Value::Object(map) = entry {
            map.insert(name.clone(), serde_json::Value::Object(t.clone()));
        }
    }) {
        logger::fail(&format!("Failed to save template: {}", e));
        return 1;
    }
    logger::success(&format!("Updated template: {}", name));
    0
}

/// Port of `handleTemplate("add")`.
pub fn run_template_add(parsed: &ParsedArgs) -> i32 {
    let name = parsed.str("name").unwrap_or_default();
    let config = load_config();
    let templates = config
        .raw
        .get("templates")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    if templates.contains_key(&name) {
        logger::fail(&format!("Template already exists: {}", name));
        return 1;
    }

    let price = parsed
        .str("price")
        .and_then(|v| v.trim().parse::<f64>().ok())
        .unwrap_or(0.0);
    let currency = parsed.str("currency").unwrap_or_else(|| "USD".to_string());
    let cycle = parsed.str("cycle").unwrap_or_else(|| "monthly".to_string());

    if !price.is_finite() || price <= 0.0 {
        logger::fail("Template price must be a positive number");
        return 1;
    }
    if !crate::validation::is_valid_currency(&currency) {
        logger::fail(&format!("Invalid currency: {}", currency));
        return 1;
    }
    if !crate::validation::is_valid_cycle(&cycle) {
        logger::fail(&format!("Invalid cycle: {}", cycle));
        return 1;
    }
    let billing_day = parsed.str("billingDay").and_then(|v| v.trim().parse::<i64>().ok());
    if let Some(d) = billing_day {
        if !(1..=31).contains(&d) {
            logger::fail("billingDay must be an integer between 1 and 31");
            return 1;
        }
    }

    let mut t = serde_json::Map::new();
    t.insert("name".into(), serde_json::Value::String(name.clone()));
    t.insert("price".into(), crate::json::num_value(price));
    t.insert("currency".into(), serde_json::Value::String(currency));
    t.insert("cycle".into(), serde_json::Value::String(cycle));
    let tags: Vec<serde_json::Value> = parsed
        .str("tags")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(serde_json::Value::String)
        .collect();
    t.insert("tags".into(), serde_json::Value::Array(tags));
    for (key, field) in [
        ("billingDay", "billingDay"),
        ("notes", "notes"),
        ("paymentMethod", "paymentMethod"),
        ("vendorName", "vendorName"),
        ("vendorUrl", "vendorUrl"),
        ("planTier", "planTier"),
    ] {
        if let Some(v) = parsed.str(field) {
            let trimmed = v.trim();
            if !trimmed.is_empty() {
                if key == "billingDay" {
                    if let Ok(n) = trimmed.parse::<i64>() {
                        t.insert(key.into(), serde_json::Value::Number(n.into()));
                    }
                } else {
                    t.insert(key.into(), serde_json::Value::String(trimmed.to_string()));
                }
            }
        }
    }
    if let Some(v) = parsed.str("autoRenewal") {
        t.insert("autoRenewal".into(), serde_json::Value::Bool(v == "true"));
    }

    if let Err(e) = crate::config::save_config_mut(|raw| {
        let entry = raw
            .entry("templates")
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
        if let serde_json::Value::Object(map) = entry {
            map.insert(name.clone(), serde_json::Value::Object(t.clone()));
        }
    }) {
        logger::fail(&format!("Failed to save template: {}", e));
        return 1;
    }
    if let Ok(conn) = crate::db::open_db() {
        let _ = crate::db::add_audit_log(&conn, "template.add", None, None, Some(&name), true);
    }
    logger::success(&format!("Added template: {}", name));
    0
}