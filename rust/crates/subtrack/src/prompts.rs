//! Prompt helpers — flag paths plus the interactive fallbacks ported from
//! `packages/subtrack/src/prompts.ts` (`promptString`, `promptSelect`,
//! `promptCycle`).
//!
//! When a flag is supplied the value is validated exactly as before. When it
//! is missing and stdin is a terminal (or `SUBTRACK_ASSUME_TTY=1`), the user
//! is prompted instead of the command silently exiting.

use crate::logger::fail;
use crate::prompt::{self, Choice};
use crate::validation::{CYCLE_VALUES, is_valid_cycle, validate_cycle_days};

/// Port of `promptString`.
pub fn prompt_string(
    flag: Option<&str>,
    message: &str,
    validate: fn(&str) -> Result<(), String>,
) -> Option<String> {
    if let Some(value) = flag {
        return match validate(value) {
            Ok(()) => Some(value.to_string()),
            Err(msg) => {
                fail(&msg);
                None
            }
        };
    }
    prompt::input(message, None, Some(validate))
}

/// Port of `promptSelect`.
pub fn prompt_select(
    flag: Option<&str>,
    message: &str,
    values: &[&str],
    is_valid: fn(&str) -> bool,
) -> Option<String> {
    if let Some(value) = flag {
        if !is_valid(value) {
            fail(&format!(
                "Invalid \"{}\". Valid: {}",
                value,
                values.join(", ")
            ));
            return None;
        }
        return Some(value.to_string());
    }

    let choices: Vec<Choice> = values.iter().map(|v| Choice::plain(v)).collect();
    let index = prompt::select(message, &choices, None)?;
    if !is_valid(&index) {
        return None;
    }
    Some(index)
}

/// Port of `promptCycle` — named presets or a custom `Nd` day cycle.
pub fn prompt_cycle(flag: Option<&str>, message: &str) -> Option<String> {
    if let Some(value) = flag {
        if !is_valid_cycle(value) {
            fail(&format!(
                "Invalid cycle \"{}\". Valid: {}, or \"Nd\" (days)",
                value,
                CYCLE_VALUES.join(", ")
            ));
            return None;
        }
        return Some(value.to_string());
    }

    let mut choices: Vec<Choice> = CYCLE_VALUES.iter().map(|v| Choice::plain(v)).collect();
    choices.push(Choice::new("custom (every N days)", "__custom__"));
    let value = prompt::select(message, &choices, None)?;
    if value != "__custom__" {
        return Some(value);
    }

    let days = prompt::input("days per cycle (e.g. 3)", None, Some(validate_cycle_days))?;
    Some(format!("{}d", days.trim().parse::<i64>().unwrap_or(0)))
}

/// Currency choices with descriptive labels, mirroring `CURRENCY_CHOICES`.
pub const CURRENCY_CHOICES: [(&str, &str); 37] = [
    ("AED (UAE Dirham)", "AED"),
    ("ARS (Argentine Peso)", "ARS"),
    ("AUD (Australian Dollar)", "AUD"),
    ("BRL (Brazilian Real)", "BRL"),
    ("CAD (Canadian Dollar)", "CAD"),
    ("CHF (Swiss Franc)", "CHF"),
    ("CLP (Chilean Peso)", "CLP"),
    ("CNY (Chinese Yuan)", "CNY"),
    ("COP (Colombian Peso)", "COP"),
    ("CZK (Czech Koruna)", "CZK"),
    ("DKK (Danish Krone)", "DKK"),
    ("EGP (Egyptian Pound)", "EGP"),
    ("EUR (Euro)", "EUR"),
    ("GBP (British Pound)", "GBP"),
    ("HKD (Hong Kong Dollar)", "HKD"),
    ("HUF (Hungarian Forint)", "HUF"),
    ("IDR (Indonesian Rupiah)", "IDR"),
    ("ILS (Israeli Shekel)", "ILS"),
    ("INR (Indian Rupee)", "INR"),
    ("JPY (Japanese Yen)", "JPY"),
    ("KRW (South Korean Won)", "KRW"),
    ("MXN (Mexican Peso)", "MXN"),
    ("MYR (Malaysian Ringgit)", "MYR"),
    ("NGN (Nigerian Naira)", "NGN"),
    ("NOK (Norwegian Krone)", "NOK"),
    ("NZD (New Zealand Dollar)", "NZD"),
    ("PHP (Philippine Peso)", "PHP"),
    ("PLN (Polish Zloty)", "PLN"),
    ("SAR (Saudi Riyal)", "SAR"),
    ("SEK (Swedish Krona)", "SEK"),
    ("SGD (Singapore Dollar)", "SGD"),
    ("THB (Thai Baht)", "THB"),
    ("TRY (Turkish Lira)", "TRY"),
    ("TWD (New Taiwan Dollar)", "TWD"),
    ("USD (US Dollar)", "USD"),
    ("VND (Vietnamese Dong)", "VND"),
    ("ZAR (South African Rand)", "ZAR"),
];

/// Currency prompt with labels and the configured default pre-selected.
pub fn prompt_currency(flag: Option<&str>, message: &str) -> Option<String> {
    if let Some(value) = flag {
        if !crate::validation::is_valid_currency(value) {
            fail(&format!(
                "Invalid \"{}\". Valid: {}",
                value,
                crate::validation::CURRENCY_VALUES.join(", ")
            ));
            return None;
        }
        return Some(value.to_string());
    }

    let choices: Vec<Choice> = CURRENCY_CHOICES
        .iter()
        .map(|(label, value)| Choice::new(label, value))
        .collect();
    let default = crate::config::load_config().default_currency.to_uppercase();
    let default_index = choices.iter().position(|c| c.value == default);
    let value = prompt::select(message, &choices, default_index)?;
    if !crate::validation::is_valid_currency(&value) {
        return None;
    }
    Some(value)
}

/// Port of `promptString` for optional values: an empty answer returns `None`
/// for the field while a missing answer aborts the workflow (`None` outer).
pub fn prompt_optional(
    flag: Option<&str>,
    message: &str,
    validate: fn(&str) -> Result<(), String>,
) -> Option<Option<String>> {
    if let Some(value) = flag {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Some(None);
        }
        return match validate(trimmed) {
            Ok(()) => Some(Some(trimmed.to_string())),
            Err(msg) => {
                fail(&msg);
                None
            }
        };
    }

    let value = prompt::input(message, Some(""), Some(validate))?;
    let trimmed = value.trim();
    Some(if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    })
}
