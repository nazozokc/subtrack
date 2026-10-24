use super::load_config;

pub fn display_theme() -> subtrack_ui::table::Theme {
    use subtrack_core::ansi::is_color_name;
    use subtrack_ui::table::Theme;
    let cfg = load_config();

    let preset = match cfg.theme.as_str() {
        "light" => preset_light(),
        "high-contrast" => preset_high_contrast(),
        "none" => preset_none(),
        _ => subtrack_ui::table::default_theme(),
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

fn preset_light() -> subtrack_ui::table::Theme {
    subtrack_ui::table::Theme {
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

fn preset_high_contrast() -> subtrack_ui::table::Theme {
    subtrack_ui::table::Theme {
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

fn preset_none() -> subtrack_ui::table::Theme {
    subtrack_ui::table::Theme::default()
}
