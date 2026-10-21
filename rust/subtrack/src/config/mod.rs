//! Config loading — port of `packages/subtrack/src/config.ts`.
//! The config file is `config.json` in the DB directory (possibly encrypted
//! with the SUBCCFG magic header).

use std::sync::Mutex;
use std::sync::OnceLock;

mod keys;
mod theme;

pub use keys::CONFIG_KEYS;
pub use theme::display_theme;

use keys::{display_value, is_known_key, mask_secret, parse_budgets, stringify};

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
    // `DEFAULT_CONFIG` in config.ts carries these two non-nullable defaults, so
    // they are present in `raw` even before the file sets them (and are thereby
    // persisted on the next save).
    cfg.raw
        .insert("theme".into(), serde_json::Value::String("default".into()));
    cfg.raw.insert(
        "defaultCurrency".into(),
        serde_json::Value::String("USD".into()),
    );
    let path = crate::crypto::config_dir().join("config.json");
    let Ok(decrypted) = crate::crypto::decrypt_file_if_needed(&path) else {
        return cfg;
    };
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&decrypted) else {
        return cfg;
    };
    let obj = json.as_object();
    let get = |k: &str| -> Option<&serde_json::Value> { obj?.get(k) };
    let get_str =
        |k: &str| -> Option<String> { get(k).and_then(|v| v.as_str()).map(|s| s.to_string()) };
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
            let Some(entry) = item.as_object() else {
                continue;
            };
            let name = entry
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or_default();
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
    if let Some(o) = obj {
        for (k, v) in o {
            cfg.raw.insert(k.clone(), v.clone());
        }
    }
    cfg
}

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
                    raw.insert(key.into(), crate::json::num_value(n));
                }
                _ => bad!("monthlyBudget must be a non-negative number"),
            }
        }
        "yearlyBudget" => {
            let n = crate::validation::js_number(value);
            match n {
                Some(n) if n >= 0.0 && n.is_finite() => {
                    raw.insert(key.into(), crate::json::num_value(n));
                }
                _ => bad!("yearlyBudget must be a non-negative number"),
            }
        }
        "budgets" => match parse_budgets(value) {
            Some(entries) => {
                raw.insert(key.into(), serde_json::Value::Array(entries));
            }
            None => return 1,
        },
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
                    raw.insert(key.into(), crate::json::num_value(n));
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
                    raw.insert(key.into(), crate::json::num_value(n));
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
    if let Ok(conn) = crate::db::open_db() {
        let _ = crate::db::add_audit_log(
            &conn,
            "config.set",
            None,
            None,
            Some(&format!("{} = {}", key, display)),
            true,
        );
    }
    crate::logger::success(&format!("Set {} = {}", key, display));
    0
}

/// Write the config JSON back to disk (encrypted when a key is available).
/// Mutate the on-disk config object and persist it.
pub fn save_config_mut(
    f: impl FnOnce(&mut serde_json::Map<String, serde_json::Value>),
) -> std::io::Result<()> {
    let mut raw = load_config().raw;
    f(&mut raw);
    save_config_raw(&raw)
}

fn save_config_raw(raw: &serde_json::Map<String, serde_json::Value>) -> std::io::Result<()> {
    let dir = crate::crypto::config_dir();
    std::fs::create_dir_all(&dir)?;
    let json = format!(
        "{}\n",
        serde_json::to_string_pretty(&serde_json::Value::Object(raw.clone()))
            .map_err(|e| std::io::Error::other(e.to_string()))?
    );
    // Always encrypt, mirroring `save_db`: the DB is always encrypted and the
    // captured config carries the SUBCCFG header. Writing plaintext here (when
    // no key exists yet) produced a file that `decrypt_file_if_needed` then
    // misdetected as an encrypted database, silently discarding the config.
    let key = crate::crypto::get_or_create_key()?;
    let mut bytes = crate::crypto::CONFIG_MAGIC.to_vec();
    bytes.extend(crate::crypto::encrypt_buffer(&key, json.as_bytes()));
    std::fs::write(crate::crypto::config_dir().join("config.json"), bytes)
}

/// Port of `handleConfigReset`.
pub fn handle_config_reset() -> i32 {
    let dir = crate::crypto::config_dir();
    let _ = std::fs::remove_file(dir.join("config.json"));
    let _ = std::fs::remove_file(dir.join("config.json.sha256"));
    crate::logger::success("Config reset to defaults");
    0
}
