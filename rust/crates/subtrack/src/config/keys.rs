use super::Config;

/// Order mirrors `CONFIG_KEYS` in config.ts.
pub const CONFIG_KEYS: [&str; 17] = [
    "defaultCurrency",
    "monthlyBudget",
    "yearlyBudget",
    "theme",
    "notifyDays",
    "notifyChannels",
    "slackWebhook",
    "webhookUrl",
    "tableBorderColor",
    "tableHeaderColor",
    "tableZebraColor",
    "accentColor",
    "tableZebra",
    "tableMinWidth",
    "dateFormat",
    "listShowNotes",
    "listShowMethod",
];

pub(super) fn mask_secret(value: &str) -> String {
    let Some(idx) = value.find("://") else {
        return "(set)".to_string();
    };
    let scheme = &value[..idx];
    let rest = &value[idx + 3..];
    let (host, path) = match rest.find('/') {
        Some(p) => (&rest[..p], &rest[p..]),
        None => (rest, ""),
    };
    if host.is_empty() {
        return "(set)".to_string();
    }
    let masked_path = if path.is_empty() || path == "/" {
        ""
    } else {
        "/***"
    };
    format!("{}://{}{}", scheme, host, masked_path)
}

pub(super) fn stringify(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => "(not set)".to_string(),
        other => other.to_string(),
    }
}

pub(super) fn display_value(key: &str, cfg: &Config) -> String {
    match key {
        "notifyChannels" => cfg
            .raw
            .get("notifyChannels")
            .and_then(|v| v.as_array())
            .filter(|a| !a.is_empty())
            .map(|a| {
                a.iter()
                    .map(|v| v.as_str().unwrap_or("").to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_else(|| "(not set)".to_string()),
        "slackWebhook" => mask_secret(
            cfg.raw
                .get("slackWebhook")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
        ),
        "webhookUrl" => mask_secret(
            cfg.raw
                .get("webhookUrl")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
        ),
        _ => match cfg.raw.get(key) {
            None | Some(serde_json::Value::Null) => "(not set)".to_string(),
            Some(serde_json::Value::String(s)) if s.is_empty() => "(not set)".to_string(),
            Some(v) => stringify(v),
        },
    }
}

pub(super) fn is_known_key(key: &str) -> bool {
    CONFIG_KEYS.contains(&key) || key == "budgets"
}

pub(super) fn parse_budgets(value: &str) -> Option<Vec<serde_json::Value>> {
    let parsed: serde_json::Value = match serde_json::from_str(value) {
        Ok(v) => v,
        Err(_) => {
            crate::logger::fail("budgets must be a valid JSON array of budget entries");
            return None;
        }
    };
    let arr = match parsed.as_array() {
        Some(a) => a.clone(),
        None => {
            crate::logger::fail("budgets must be a valid JSON array of budget entries");
            return None;
        }
    };
    for item in &arr {
        let Some(obj) = item.as_object() else {
            crate::logger::fail("Each budget entry must be an object with name, amount, currency");
            return None;
        };
        let name = obj.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.trim().is_empty() {
            crate::logger::fail("Each budget entry needs a non-empty name");
            return None;
        }
        let amount = obj.get("amount").and_then(|v| v.as_f64());
        match amount {
            Some(a) if a >= 0.0 && a.is_finite() => {}
            _ => {
                crate::logger::fail(&format!(
                    "Budget \"{}\": amount must be a non-negative number",
                    name
                ));
                return None;
            }
        }
        let currency = obj.get("currency").and_then(|v| v.as_str()).unwrap_or("");
        if !(currency.len() == 3 && currency.bytes().all(|b| b.is_ascii_uppercase())) {
            crate::logger::fail(&format!(
                "Budget \"{}\": currency must be a 3-letter code (e.g. JPY)",
                name
            ));
            return None;
        }
        if let Some(period) = obj.get("period") {
            if let Some(p) = period.as_str() {
                if p != "monthly" && p != "yearly" {
                    crate::logger::fail(&format!(
                        "Budget \"{}\": period must be \"monthly\" or \"yearly\"",
                        name
                    ));
                    return None;
                }
            } else {
                crate::logger::fail(&format!(
                    "Budget \"{}\": period must be \"monthly\" or \"yearly\"",
                    name
                ));
                return None;
            }
        }
        if let Some(cats) = obj.get("categories") {
            let valid = cats
                .as_array()
                .map(|a| a.iter().all(|c| c.is_string()))
                .unwrap_or(false);
            if !valid {
                crate::logger::fail(&format!(
                    "Budget \"{}\": categories must be an array of tag names",
                    name
                ));
                return None;
            }
        }
    }
    Some(arr)
}
