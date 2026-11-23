use super::Config;

/// Order mirrors `CONFIG_KEYS` in config.ts.
pub const CONFIG_KEYS: [&str; 19] = [
    "language",
    "logLevel",
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

pub fn display_value(key: &str, cfg: &Config) -> String {
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

pub(super) fn parse_budgets(value: &str) -> Result<Vec<serde_json::Value>, String> {
    const BAD_ARRAY: &str = "budgets must be a valid JSON array of budget entries";
    let parsed: serde_json::Value =
        serde_json::from_str(value).map_err(|_| BAD_ARRAY.to_string())?;
    let arr = parsed
        .as_array()
        .ok_or_else(|| BAD_ARRAY.to_string())?
        .clone();
    for item in &arr {
        let Some(obj) = item.as_object() else {
            return Err("Each budget entry must be an object with name, amount, currency".into());
        };
        let name = obj.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.trim().is_empty() {
            return Err("Each budget entry needs a non-empty name".into());
        }
        let amount = obj.get("amount").and_then(|v| v.as_f64());
        match amount {
            Some(a) if a >= 0.0 && a.is_finite() => {}
            _ => {
                return Err(format!(
                    "Budget \"{}\": amount must be a non-negative number",
                    name
                ));
            }
        }
        let currency = obj.get("currency").and_then(|v| v.as_str()).unwrap_or("");
        if !(currency.len() == 3 && currency.bytes().all(|b| b.is_ascii_uppercase())) {
            return Err(format!(
                "Budget \"{}\": currency must be a 3-letter code (e.g. JPY)",
                name
            ));
        }
        if let Some(period) = obj.get("period") {
            if let Some(p) = period.as_str() {
                if p != "monthly" && p != "yearly" {
                    return Err(format!(
                        "Budget \"{}\": period must be \"monthly\" or \"yearly\"",
                        name
                    ));
                }
            } else {
                return Err(format!(
                    "Budget \"{}\": period must be \"monthly\" or \"yearly\"",
                    name
                ));
            }
        }
        if let Some(cats) = obj.get("categories") {
            let valid = cats
                .as_array()
                .map(|a| a.iter().all(|c| c.is_string()))
                .unwrap_or(false);
            if !valid {
                return Err(format!(
                    "Budget \"{}\": categories must be an array of tag names",
                    name
                ));
            }
        }
    }
    Ok(arr)
}
