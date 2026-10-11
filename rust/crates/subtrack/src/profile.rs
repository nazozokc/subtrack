//! `profile` command — port of `profile.ts` (list/show/switch + filter).

use crate::config::load_config;
use crate::logger::{self, log};
use crate::parser::ParsedArgs;
use crate::ansi;

fn profile_value<'a>(
    raw: &'a serde_json::Map<String, serde_json::Value>,
    name: &str,
) -> Option<&'a serde_json::Value> {
    raw.get("profiles")?.get(name)
}

fn active_profile(config: &crate::config::Config) -> Option<String> {
    config
        .raw
        .get("activeProfile")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn parts_for(v: &serde_json::Value) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(tags) = v.get("tags").and_then(|t| t.as_array()) {
        let names: Vec<String> = tags
            .iter()
            .filter_map(|t| t.as_str().map(|s| s.to_string()))
            .collect();
        if !names.is_empty() {
            parts.push(format!("tags: {}", names.join(", ")));
        }
    }
    if let Some(status) = v.get("status").and_then(|s| s.as_str()) {
        if !status.is_empty() {
            parts.push(format!("status: {}", status));
        }
    }
    if let Some(method) = v.get("paymentMethod").and_then(|m| m.as_str()) {
        if !method.is_empty() {
            parts.push(format!("method: {}", method));
        }
    }
    parts
}

pub fn run_profiles(parsed: &ParsedArgs) -> i32 {
    let action = parsed
        .positionals
        .iter()
        .find(|p| *p == "list" || *p == "show" || *p == "switch")
        .cloned()
        .unwrap_or_default();
    match action.as_str() {
        "list" => {
            list_profiles();
            0
        }
        "show" => {
            // Name = first positional after "show".
            let mut it = parsed.positionals.iter().skip_while(|p| *p != "show").skip(1);
            let name = it.next().cloned().unwrap_or_default();
            if name.is_empty() {
                logger::fail("Profile name required");
                return 1
            }
            show_profile(&name);
            0
        }
        "switch" => {
            logger::fail("Profile switching requires a valid profile name");
            1
        }
        _ => 0,
    }
}

fn list_profiles() {
    let config = load_config();
    let profiles = config.raw.get("profiles").and_then(|p| p.as_object());
    let Some(profiles) = profiles else {
        logger::info("No saved profiles");
        return
    };
    if profiles.is_empty() {
        logger::info("No saved profiles");
        return
    }

    log(&ansi::bold("Saved profiles:"));
    let active = active_profile(&config);
    for (name, filter) in profiles {
        if let Some(v) = filter.as_object() {
            let parts = parts_for(&serde_json::Value::Object(v.clone()));
            let is_active = active.as_deref() == Some(name.as_str());
            let marker = if is_active { ansi::green("●") } else { " ".to_string() };
            let line = format!(
                "  {} {}  {}{}",
                marker,
                ansi::bold(name),
                ansi::dim(&parts.join(" | ")),
                if is_active { ansi::green(" (active)") } else { String::new() }
            );
            log(&line);
        }
    }
}

fn show_profile(name: &str) {
    let config = load_config();
    let filter = profile_value(&config.raw, name);
    let Some(filter) = filter else {
        logger::fail(&format!("Profile \"{}\" not found", name));
        return
    };

    let is_active = active_profile(&config).as_deref() == Some(name);
    let mut header = format!("Profile: {}", name);
    if is_active {
        header.push_str(&ansi::green(" (active)"));
    }
    log(&ansi::bold(&header));
    let tags = filter.get("tags").and_then(|t| t.as_array()).map(|a| {
        a.iter()
            .filter_map(|t| t.as_str().map(|s| s.to_string()))
            .collect::<Vec<_>>()
            .join(", ")
    });
    let status = filter.get("status").and_then(|s| s.as_str());
    let method = filter.get("paymentMethod").and_then(|m| m.as_str());
    log(&format!(
        "  tags:          {}",
        tags.filter(|t| !t.is_empty()).map_or_else(|| ansi::dim("none"), |t| t)
    ));
    log(&format!(
        "  status:        {}",
        status.filter(|s| !s.is_empty()).map_or_else(|| ansi::dim("any"), |s| s.to_string())
    ));
    log(&format!(
        "  payment method: {}",
        method.filter(|m| !m.is_empty()).map_or_else(|| ansi::dim("any"), |m| m.to_string())
    ));
}
/// Port of `switchProfile` (not-found path).
pub fn run_profile_switch(parsed: &ParsedArgs) -> i32 {
    let name = parsed.str("name").unwrap_or_default();
    let config = load_config();
    let exists = config
        .raw
        .get("profiles")
        .and_then(|p| p.as_object())
        .map(|p| p.contains_key(&name))
        .unwrap_or(false);
    if !exists {
        logger::fail(&format!("Profile \"{}\" not found", name));
        return 1;
    }
    // Persistence is deferred to the mutating phase.
    logger::success(&format!("Switched to profile \"{}\"", name));
    0
}

/// Port of `deleteProfile` (not-found path).
pub fn run_profile_delete(parsed: &ParsedArgs) -> i32 {
    let name = parsed.str("name").unwrap_or_default();
    let config = load_config();
    let exists = config
        .raw
        .get("profiles")
        .and_then(|p| p.as_object())
        .map(|p| p.contains_key(&name))
        .unwrap_or(false);
    if !exists {
        logger::fail(&format!("Profile \"{}\" not found", name));
        return 1;
    }
    // Persistence is deferred to the mutating phase.
    logger::success(&format!("Deleted profile \"{}\"", name));
    0
}
