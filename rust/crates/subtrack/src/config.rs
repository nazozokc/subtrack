//! Config loading — port of `packages/subtrack/src/config.ts`.
//! The config file is `config.json` in the DB directory (possibly encrypted
//! with the SUBCCFG magic header).

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;

#[derive(Clone, Debug)]
pub struct Config {
    pub theme: String,
    pub table_min_width: i64,
    pub list_show_notes: bool,
    pub list_show_method: bool,
    pub default_currency: String,
    /// Individual color overrides (empty string when unset).
    pub table_border_color: String,
    pub table_header_color: String,
    pub table_zebra_color: String,
    pub accent_color: String,
    pub table_zebra: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            theme: "default".to_string(),
            table_min_width: 40,
            list_show_notes: false,
            list_show_method: false,
            default_currency: "USD".to_string(),
            table_border_color: String::new(),
            table_header_color: String::new(),
            table_zebra_color: String::new(),
            accent_color: String::new(),
            table_zebra: String::new(),
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
    cfg
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