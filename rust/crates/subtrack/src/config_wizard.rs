//! Interactive settings editor for `subtrack config` and the entry menu.
//!
//! The flag path (`config set/get/list/reset`) stays unchanged and scriptable;
//! this module only adds the guided flow used when a terminal is attached.

use crate::config::{self, CONFIG_KEYS};
use crate::i18n::tr;
use crate::logger;
use crate::prompt::{self, Choice, Validator};

const DONE: &str = "\0done";

const COLOR_NAMES: &[&str] = &[
    "black",
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "white",
    "gray",
    "brightBlack",
    "brightRed",
    "brightGreen",
    "brightYellow",
    "brightBlue",
    "brightMagenta",
    "brightCyan",
    "brightWhite",
];

const CHANNELS: &[&str] = &["os", "slack", "webhook"];

/// Webhooks are secrets: echoing them into a value prompt would leak them
/// into the terminal and scrollback. They remain editable via `config set`.
fn is_secret(key: &str) -> bool {
    key == "slackWebhook" || key == "webhookUrl"
}

fn choices_for(key: &str) -> Option<&'static [&'static str]> {
    match key {
        "language" => Some(&["en", "ja"]),
        "logLevel" => Some(&["quiet", "normal", "verbose"]),
        "theme" => Some(&["default", "light", "high-contrast", "none"]),
        "dateFormat" => Some(&["iso", "short"]),
        "tableZebra" | "listShowNotes" | "listShowMethod" => Some(&["on", "off"]),
        "tableBorderColor" | "tableHeaderColor" | "tableZebraColor" | "accentColor" => {
            Some(COLOR_NAMES)
        }
        _ => None,
    }
}

fn validator_for(key: &str) -> Option<Validator> {
    macro_rules! validator {
        ($name:literal) => {{
            let check: Validator = |value| config::validate_value($name, value);
            Some(check)
        }};
    }
    match key {
        "defaultCurrency" => validator!("defaultCurrency"),
        "monthlyBudget" => validator!("monthlyBudget"),
        "yearlyBudget" => validator!("yearlyBudget"),
        "notifyDays" => validator!("notifyDays"),
        "tableMinWidth" => validator!("tableMinWidth"),
        "notifyChannels" => validator!("notifyChannels"),
        "budgets" => validator!("budgets"),
        _ => None,
    }
}

fn key_choices(cfg: &config::Config) -> Vec<Choice> {
    let mut choices: Vec<Choice> = CONFIG_KEYS
        .iter()
        .filter(|key| !is_secret(key))
        .map(|key| {
            Choice::new(
                &format!("{} ({})", key, config::display_value(key, cfg)),
                key,
            )
        })
        .collect();
    choices.push(Choice::new(
        &format!("budgets ({})", config::display_value("budgets", cfg)),
        "budgets",
    ));
    choices.push(Choice::new(&tr("Done"), DONE));
    choices
}

fn current_value(key: &str, cfg: &config::Config) -> String {
    let value = config::display_value(key, cfg);
    if value == "(not set)" {
        String::new()
    } else {
        value
    }
}

/// Ask for a new value. `None` means the user backed out, so nothing is saved.
fn ask_value(key: &str, cfg: &config::Config) -> Option<String> {
    let current = current_value(key, cfg);
    let message = format!("{} {}", tr("New value for"), key);

    if let Some(options) = choices_for(key) {
        let choices: Vec<Choice> = options.iter().map(|value| Choice::plain(value)).collect();
        let default = options.iter().position(|value| *value == current);
        return prompt::select(&message, &choices, default);
    }

    if key == "notifyChannels" {
        let enabled: Vec<String> = cfg
            .raw
            .get(key)
            .and_then(|value| value.as_array())
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| value.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let choices: Vec<Choice> = CHANNELS.iter().map(|value| Choice::plain(value)).collect();
        let message = format!("{} [{}]", message, enabled.join(","));
        let selected = prompt::checkbox(&message, &choices)?;
        return if selected.is_empty() {
            None
        } else {
            Some(selected.join(","))
        };
    }

    let validate = validator_for(key)?;
    // With no stored value there is nothing to prefill: an empty answer is a
    // cancellation, not an attempt to validate the empty string as a value.
    let default = if current.is_empty() {
        None
    } else {
        Some(current.as_str())
    };
    prompt::input_optional_cancelable(&message, default, Some(validate))
}

/// Run the guided settings flow. Returns 0 unless a save failed.
pub fn run_config_wizard() -> i32 {
    let mut changed = 0usize;
    loop {
        let cfg = config::load_config();
        let Some(action) = prompt::select("Select a setting to change", &key_choices(&cfg), None)
        else {
            break;
        };
        if action == DONE {
            break;
        }
        let Some(value) = ask_value(&action, &cfg) else {
            continue;
        };
        if value == current_value(&action, &cfg) {
            continue;
        }
        if config::handle_config_set(&action, &value) == 0 {
            changed += 1;
        }
    }
    if changed == 0 {
        logger::info("No settings changed");
    } else {
        logger::success(&format!("{} setting(s) updated", changed));
    }
    0
}
