//! Config loading — port of `packages/subtrack/src/config.ts`.
//! The config file is `config.json` in the DB directory (possibly encrypted
//! with the SUBCCFG magic header).

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;

#[derive(Clone, Debug)]
/// A named budget entry from `config.budgets` (port of resolveBudget).
pub struct BudgetEntry {
    pub name: String,
    pub amount: f64,
    pub currency: Option<String>,
    pub categories: Vec<String>,
    pub period: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub theme: String,
    pub table_min_width: i64,
    pub list_show_notes: bool,
    pub list_show_method: bool,
    pub default_currency: String,
    pub monthly_budget: f64,
    pub yearly_budget: Option<f64>,
    /// Individual color overrides (empty string when unset).
    pub table_border_color: String,
    pub table_header_color: String,
    pub table_zebra_color: String,
    pub accent_color: String,
    pub table_zebra: String,
    /// Raw parsed config object (for `config list`/`get` display).
    pub raw: serde_json::Map<String, serde_json::Value>,
    pub notify_days: i64,
    pub notify_channels: Vec<String>,
    pub slack_webhook: Option<String>,
    pub webhook_url: Option<String>,
    pub date_format: String,
    pub budgets: Vec<BudgetEntry>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            theme: "default".to_string(),
            table_min_width: 40,
            list_show_notes: false,
            list_show_method: false,
            default_currency: "USD".to_string(),
            monthly_budget: 0.0,
            yearly_budget: None,
            table_border_color: String::new(),
            table_header_color: String::new(),
            table_zebra_color: String::new(),
            accent_color: String::new(),
            table_zebra: String::new(),
            raw: serde_json::Map::new(),
            notify_days: 7,
            notify_channels: Vec::new(),
            slack_webhook: None,
            webhook_url: None,
            date_format: "iso".to_string(),
            budgets: Vec::new(),
        }
    }
}

// Cached per-process; config is read-only for the list/reads scope.
static CONFIG: OnceLock<Mutex<Config>> = OnceLock::new();

pub fn load_config() -> Config {
    CONFIG
        .get_or_init(|| Mutex::new(read_config()))
        .lock()
        .unwrap()
        .clone()
}

fn read_config() -> Config {
    let mut cfg = Config::default();
    let path = crate::crypto::config_dir().join("config.json");
    let Ok(decrypted) = crate::crypto::decrypt_file_if_needed(&path) else {
        return cfg;
    };
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&decrypted) else {
        return cfg;
    };
    let obj = json.as_object();
    let get = |k: &str| -> Option<&serde_json::Value> { obj?.get(k) };
    let get_str = |k: &str| -> Option<String> {
        get(k).and_then(|v| v.as_str()).map(|s| s.to_string())
    };
    cfg.theme = get_str("theme").unwrap_or_else(|| cfg.theme.clone());
    cfg.default_currency = get_str("defaultCurrency").unwrap_or(cfg.default_currency);
    cfg.monthly_budget = get("monthlyBudget").and_then(|v| v.as_f64()).unwrap_or(0.0);
    cfg.yearly_budget = get("yearlyBudget").and_then(|v| v.as_f64());
    cfg.list_show_notes = get_str("listShowNotes").as_deref() == Some("on");
    cfg.list_show_method = get_str("listShowMethod").as_deref() == Some("on");
    if let Some(v) = get("tableMinWidth").and_then(|v| v.as_i64()) {
        cfg.table_min_width = v
    }
    cfg.table_border_color = get_str("tableBorderColor").unwrap_or_default();
    cfg.table_header_color = get_str("tableHeaderColor").unwrap_or_default();
    cfg.table_zebra_color = get_str("tableZebraColor").unwrap_or_default();
    cfg.accent_color = get_str("accentColor").unwrap_or_default();
    cfg.table_zebra = get_str("tableZebra").unwrap_or_default();
    cfg.notify_days = get("notifyDays").and_then(|v| v.as_i64()).unwrap_or(7);
    cfg.notify_channels = get("notifyChannels")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    cfg.slack_webhook = get_str("slackWebhook");
    cfg.webhook_url = get_str("webhookUrl");
    cfg.date_format = get_str("dateFormat").unwrap_or_else(|| "iso".to_string());
    if let Some(arr) = get("budgets").and_then(|v| v.as_array()) {
        for item in arr {
            let Some(entry) = item.as_object() else { continue };
            let name = entry.get("name").and_then(|n| n.as_str()).unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            cfg.budgets.push(BudgetEntry {
                name: name.to_string(),
                amount: entry.get("amount").and_then(|a| a.as_f64()).unwrap_or(0.0),
                currency: entry
                    .get("currency")
                    .and_then(|c| c.as_str())
                    .map(|s| s.to_string()),
                categories: entry
                    .get("categories")
                    .and_then(|c| c.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default(),
                period: entry
                    .get("period")
                    .and_then(|p| p.as_str())
                    .map(|s| s.to_string()),
            });
        }
    }
    cfg.raw = obj.cloned().unwrap_or_default();
    cfg
}

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

fn mask_secret(value: &str) -> String {
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
    let masked_path = if path.is_empty() || path == "/" { "" } else { "/***" };
    format!("{}://{}{}", scheme, host, masked_path)
}

fn stringify(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => "(not set)".to_string(),
        other => other.to_string(),
    }
}

fn display_value(key: &str, cfg: &Config) -> String {
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

fn is_known_key(key: &str) -> bool {
    CONFIG_KEYS.contains(&key) || key == "budgets"
}

/// Port of `handleConfigList`.
pub fn handle_config_list() -> i32 {
    let cfg = load_config();
    for key in CONFIG_KEYS {
        crate::logger::log(&format!("{}: {}", key, display_value(key, &cfg)));
    }
    match cfg.raw.get("budgets").and_then(|v| v.as_array()) {
        Some(arr) if !arr.is_empty() => {
            for b in arr {
                let name = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let amount = b
                    .get("amount")
                    .map(stringify)
                    .unwrap_or_else(|| "0".to_string());
                let currency = b.get("currency").and_then(|v| v.as_str()).unwrap_or("");
                let period = b
                    .get("period")
                    .and_then(|v| v.as_str())
                    .unwrap_or("monthly");
                let cats = b
                    .get("categories")
                    .and_then(|v| v.as_array())
                    .filter(|a| !a.is_empty())
                    .map(|a| {
                        format!(
                            " (categories: {})",
                            a.iter()
                                .map(|v| v.as_str().unwrap_or("").to_string())
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    })
                    .unwrap_or_default();
                crate::logger::log(&format!(
                    "budgets: {} = {} {}/{}{}",
                    name, amount, currency, period, cats
                ));
            }
        }
        _ => crate::logger::log("budgets: (not set)"),
    }
    0
}

/// Port of `handleConfigGet`.
pub fn handle_config_get(key: &str) -> i32 {
    if !is_known_key(key) {
        crate::logger::fail(&format!(
            "Unknown config key: \"{}\". Valid: {}",
            key,
            CONFIG_KEYS.join(", ")
        ));
        return 1;
    }
    let cfg = load_config();
    crate::logger::log(&format!("{}: {}", key, display_value(key, &cfg)));
    0
}

/// Port of `handleConfigSet`. Returns 0 on success (config saved), 1 on the
/// first validation failure.
pub fn handle_config_set(key: &str, value: &str) -> i32 {
    let cfg = load_config();
    let mut raw = cfg.raw.clone();
    macro_rules! bad {
        ($msg:expr) => {{
            crate::logger::fail($msg);
            return 1;
        }};
    }
    match key {
        "defaultCurrency" => {
            if !(value.len() == 3 && value.bytes().all(|b| b.is_ascii_uppercase())) {
                bad!(&format!("Invalid currency code: \"{}\"", value));
            }
            raw.insert(key.into(), value.into());
        }
        "monthlyBudget" => {
            let n = crate::validation::js_number(value);
            match n {
                Some(n) if n >= 0.0 && n.is_finite() => {
                    raw.insert(key.into(), serde_json::json!(n));
                }
                _ => bad!("monthlyBudget must be a non-negative number"),
            }
        }
        "yearlyBudget" => {
            let n = crate::validation::js_number(value);
            match n {
                Some(n) if n >= 0.0 && n.is_finite() => {
                    raw.insert(key.into(), serde_json::json!(n));
                }
                _ => bad!("yearlyBudget must be a non-negative number"),
            }
        }
        "budgets" => {
            match parse_budgets(value) {
                Some(entries) => {
                    raw.insert(key.into(), serde_json::Value::Array(entries));
                }
                None => return 1,
            }
        }
        "theme" => {
            const THEMES: [&str; 4] = ["default", "light", "high-contrast", "none"];
            if !THEMES.contains(&value) {
                bad!(&format!("theme must be one of: {}", THEMES.join(", ")));
            }
            raw.insert(key.into(), value.into());
        }
        "tableBorderColor" | "tableHeaderColor" | "tableZebraColor" | "accentColor" => {
            if !crate::ansi::is_color_name(value) {
                bad!(&format!(
                    "\"{}\" must be a color name: black, red, green, yellow, blue, \
magenta, cyan, white, gray, brightRed, brightGreen, brightYellow, brightBlue, \
brightMagenta, brightCyan, brightWhite",
                    key
                ));
            }
            raw.insert(key.into(), value.into());
        }
        "tableZebra" => {
            if value != "on" && value != "off" {
                bad!("tableZebra must be 'on' or 'off'");
            }
            raw.insert(key.into(), value.into());
        }
        "tableMinWidth" => {
            let n = crate::validation::js_number(value);
            match n {
                Some(n) if (20.0..=200.0).contains(&n) && n.fract() == 0.0 => {
                    raw.insert(key.into(), serde_json::json!(n));
                }
                _ => bad!("tableMinWidth must be an integer between 20 and 200"),
            }
        }
        "dateFormat" => {
            if value != "iso" && value != "short" {
                bad!("dateFormat must be 'iso' or 'short'");
            }
            raw.insert(key.into(), value.into());
        }
        "listShowNotes" | "listShowMethod" => {
            if value != "on" && value != "off" {
                bad!(&format!("\"{}\" must be 'on' or 'off'", key));
            }
            raw.insert(key.into(), value.into());
        }
        "notifyDays" => {
            let n = crate::validation::js_number(value);
            match n {
                Some(n) if n >= 0.0 && n.fract() == 0.0 => {
                    raw.insert(key.into(), serde_json::json!(n));
                }
                _ => bad!("notifyDays must be a non-negative integer"),
            }
        }
        "notifyChannels" => {
            let channels: Vec<String> = value
                .split(',')
                .map(|c| c.trim().to_lowercase())
                .filter(|c| !c.is_empty())
                .collect();
            if channels.is_empty()
                || channels
                    .iter()
                    .any(|c| !["os", "slack", "webhook"].contains(&c.as_str()))
            {
                bad!("notifyChannels must be a comma-separated list of: os, slack, webhook");
            }
            raw.insert(
                key.into(),
                serde_json::Value::Array(channels.into_iter().map(|c| c.into()).collect()),
            );
        }
        "slackWebhook" => {
            if !value.starts_with("https://") {
                bad!("slackWebhook must be an https:// URL");
            }
            raw.insert(key.into(), value.into());
        }
        "webhookUrl" => {
            if !value.starts_with("https://") {
                bad!("webhookUrl must be an https:// URL");
            }
            raw.insert(key.into(), value.into());
        }
        _ => {
            bad!(&format!(
                "Unknown config key: \"{}\". Valid: {}",
                key,
                CONFIG_KEYS.join(", ")
            ));
        }
    }
    if let Err(e) = save_config_raw(&raw) {
        crate::logger::fail(&format!("Failed to save config: {}", e));
        return 1;
    }
    let display = if key == "slackWebhook" || key == "webhookUrl" {
        mask_secret(value)
    } else {
        value.to_string()
    };
    crate::logger::success(&format!("Set {} = {}", key, display));
    0
}

/// Validate + convert the `budgets` JSON value (port of `parseBudgets`).
fn parse_budgets(value: &str) -> Option<Vec<serde_json::Value>> {
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

/// Write the config JSON back to disk (encrypted when a key is available).
fn save_config_raw(raw: &serde_json::Map<String, serde_json::Value>) -> std::io::Result<()> {
    let dir = crate::crypto::config_dir();
    std::fs::create_dir_all(&dir)?;
    let json = format!(
        "{}\n",
        serde_json::to_string_pretty(&serde_json::Value::Object(raw.clone()))
            .map_err(|e| std::io::Error::other(e.to_string()))?
    );
    let bytes = if crate::crypto::has_encryption_key() {
        let key = crate::crypto::get_or_create_key()?;
        let mut out = crate::crypto::CONFIG_MAGIC.to_vec();
        out.extend(crate::crypto::encrypt_buffer(&key, json.as_bytes()));
        out
    } else {
        json.into_bytes()
    };
    std::fs::write(crate::crypto::config_dir().join("config.json"), bytes)
}

/// Resolve the display theme from config (mirror of getDisplayTheme).
pub fn display_theme() -> crate::table::Theme {
    use crate::table::Theme;
    use crate::ansi::is_color_name;
    let cfg = load_config();

    let preset = match cfg.theme.as_str() {
        "light" => preset_light(),
        "high-contrast" => preset_high_contrast(),
        "none" => preset_none(),
        _ => crate::table::default_theme(),
    };

    let pick = |cfg_val: &str, preset_val: Option<&'static str>| -> Option<&'static str> {
        if !cfg_val.is_empty() && is_color_name(cfg_val) {
            // leak for the static lifetime — config is immutable per run
            Some(Box::leak(cfg_val.to_string().into_boxed_str()))
        } else {
            preset_val
        }
    };

    Theme {
        border: pick(&cfg.table_border_color, preset.border),
        header: pick(&cfg.table_header_color, preset.header),
        zebra: if cfg.table_zebra == "off" {
            None
        } else {
            pick(&cfg.table_zebra_color, preset.zebra)
        },
        accent: pick(&cfg.accent_color, preset.accent),
        status_active: preset.status_active,
        status_paused: preset.status_paused,
        status_cancelled: preset.status_cancelled,
        status_archived: preset.status_archived,
    }
}

fn preset_light() -> crate::table::Theme {
    crate::table::Theme {
        border: Some("gray"),
        header: Some("blue"),
        zebra: Some("brightWhite"),
        accent: Some("blue"),
        status_active: Some("green"),
        status_paused: Some("yellow"),
        status_cancelled: Some("red"),
        status_archived: Some("gray"),
    }
}

fn preset_high_contrast() -> crate::table::Theme {
    crate::table::Theme {
        border: Some("white"),
        header: Some("white"),
        zebra: None,
        accent: Some("white"),
        status_active: Some("green"),
        status_paused: Some("yellow"),
        status_cancelled: Some("red"),
        status_archived: Some("white"),
    }
}

fn preset_none() -> crate::table::Theme {
    crate::table::Theme::default()
}

#[allow(dead_code)]
pub fn _unused(_: HashMap<String, String>) {}