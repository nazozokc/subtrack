//! Non-interactive prompt helpers — port of the flag paths of
//! `packages/subtrack/src/prompts.ts` (`promptString`, `promptSelect`,
//! `promptCycle`). Interactive fallbacks return `None` (no TTY in tests).

use crate::logger::fail;
use crate::validation::{is_valid_cycle, CYCLE_VALUES};

/// Port of `promptString` when a flag is supplied.
///
/// `None` means "no flag and no interactive input available" — the caller
/// should abort the workflow quietly.
pub fn prompt_string(
    flag: Option<&str>,
    validate: fn(&str) -> Result<(), String>,
) -> Option<String> {
    let value = flag?;
    match validate(value) {
        Ok(()) => Some(value.to_string()),
        Err(msg) => {
            fail(&msg);
            None
        }
    }
}

/// Port of `promptSelect` when a flag is supplied.
pub fn prompt_select(
    flag: Option<&str>,
    is_valid: fn(&str) -> bool,
    values: &[&str],
) -> Option<String> {
    let value = flag?;
    if !is_valid(value) {
        fail(&format!("Invalid \"{}\". Valid: {}", value, values.join(", ")));
        return None;
    }
    Some(value.to_string())
}

/// Port of `promptCycle` when a flag is supplied.
pub fn prompt_cycle(flag: Option<&str>) -> Option<String> {
    let value = flag?;
    if !is_valid_cycle(value) {
        fail(&format!(
            "Invalid cycle \"{}\". Valid: {}, or \"Nd\" (days)",
            value,
            CYCLE_VALUES.join(", ")
        ));
        return None;
    }
    Some(value.to_string())
}
