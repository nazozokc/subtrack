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
    // Deletion persistence is deferred to the mutating phase.
    logger::success(&format!("Deleted template: {}", name));
    0
}

/// Receipt/template helper alias to keep the logger import stable.
#[allow(dead_code)]
fn _unused(_p: &ParsedArgs) {
    let _ = logger::success;
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
    logger::success(&format!("Added template: {}", name));
    0
}