//! `profile` command — port of `profile.ts` (list/show/switch + filter).

use crate::ansi;
use crate::config::load_config;
use crate::logger::{self, log};
use crate::parser::ParsedArgs;

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
    if let Some(status) = v.get("status").and_then(|s| s.as_str())
        && !status.is_empty()
    {
        parts.push(format!("status: {}", status));
    }
    if let Some(method) = v.get("paymentMethod").and_then(|m| m.as_str())
        && !method.is_empty()
    {
        parts.push(format!("method: {}", method));
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
            let mut it = parsed
                .positionals
                .iter()
                .skip_while(|p| *p != "show")
                .skip(1);
            let name = it.next().cloned().unwrap_or_default();
            if name.is_empty() {
                logger::fail("Profile name required");
                return 1;
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
        return;
    };
    if profiles.is_empty() {
        logger::info("No saved profiles");
        return;
    }

    log(&ansi::bold("Saved profiles:"));
    let active = active_profile(&config);
    for (name, filter) in profiles {
        if let Some(v) = filter.as_object() {
            let parts = parts_for(&serde_json::Value::Object(v.clone()));
            let is_active = active.as_deref() == Some(name.as_str());
            let marker = if is_active {
                ansi::green("●")
            } else {
                " ".to_string()
            };
            let line = format!(
                "  {} {}  {}{}",
                marker,
                ansi::bold(name),
                ansi::dim(&parts.join(" | ")),
                if is_active {
                    ansi::green(" (active)")
                } else {
                    String::new()
                }
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
        return;
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
        tags.filter(|t| !t.is_empty())
            .unwrap_or_else(|| ansi::dim("none"))
    ));
    log(&format!(
        "  status:        {}",
        status
            .filter(|s| !s.is_empty())
            .map_or_else(|| ansi::dim("any"), |s| s.to_string())
    ));
    log(&format!(
        "  payment method: {}",
        method
            .filter(|m| !m.is_empty())
            .map_or_else(|| ansi::dim("any"), |m| m.to_string())
    ));
}
fn profile_names() -> Vec<String> {
    load_config()
        .raw
        .get("profiles")
        .and_then(|p| p.as_object())
        .map(|p| p.keys().cloned().collect())
        .unwrap_or_default()
}

fn pick_profile(message: &str) -> Option<String> {
    let names = profile_names();
    if names.is_empty() {
        logger::info("No saved profiles");
        return None;
    }
    let choices: Vec<crate::prompt::Choice> = names
        .iter()
        .map(|n| crate::prompt::Choice::plain(n))
        .collect();
    crate::prompt::select(message, &choices, None)
}

/// Port of `switchProfile`.
pub fn run_profile_switch(parsed: &ParsedArgs) -> i32 {
    let name = match parsed.str("name") {
        Some(n) if !n.trim().is_empty() => n,
        _ if crate::prompt::interactive() => match pick_profile("select profile to switch to") {
            Some(n) => n,
            None => return 0,
        },
        _ => {
            logger::fail("Profile switching requires a valid profile name");
            return 1;
        }
    };
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
    if let Err(e) = crate::config::save_config_mut(|raw| {
        raw.insert(
            "activeProfile".into(),
            serde_json::Value::String(name.clone()),
        );
    }) {
        logger::fail(&format!("Failed to save profile: {}", e));
        return 1;
    }
    logger::success(&format!("Switched to profile \"{}\"", name));
    0
}

/// Port of `saveProfile`.
pub fn run_profile_save(parsed: &ParsedArgs) -> i32 {
    let name = parsed.str("name").unwrap_or_default();
    if name.is_empty() {
        logger::fail("Profile name required");
        return 1;
    }
    let valid = name.len() <= 50
        && !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !valid {
        logger::fail(
            "Profile name must be 1-50 characters, using only letters, numbers, hyphens, and underscores",
        );
        return 1;
    }

    let raw_tag = parsed.str_array("tag");
    let tags: Option<Vec<String>> = raw_tag.and_then(|v| {
        let flat: Vec<String> = v
            .iter()
            .flat_map(|t| {
                t.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            })
            .collect();
        if flat.is_empty() { None } else { Some(flat) }
    });

    let mut filter = serde_json::Map::new();
    if let Some(tags) = &tags {
        filter.insert(
            "tags".into(),
            serde_json::Value::Array(
                tags.iter()
                    .cloned()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }
    if let Some(status) = parsed.str("status")
        && !status.is_empty()
    {
        filter.insert("status".into(), serde_json::Value::String(status));
    }
    if let Some(method) = parsed.str("payment-method")
        && !method.is_empty()
    {
        filter.insert("paymentMethod".into(), serde_json::Value::String(method));
    }

    if let Err(e) = crate::config::save_config_mut(|raw| {
        let entry = raw
            .entry("profiles")
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
        if let serde_json::Value::Object(map) = entry {
            map.insert(name.clone(), serde_json::Value::Object(filter.clone()));
        }
    }) {
        logger::fail(&format!("Failed to save profile: {}", e));
        return 1;
    }
    logger::success(&format!("Profile \"{}\" saved", name));
    0
}

/// Port of `deleteProfile` (not-found path).
pub fn run_profile_delete(parsed: &ParsedArgs) -> i32 {
    let name = match parsed.str("name") {
        Some(n) if !n.trim().is_empty() => n,
        _ if crate::prompt::interactive() => match pick_profile("select profile to delete") {
            Some(n) => n,
            None => return 0,
        },
        _ => {
            logger::fail("Profile name required");
            return 1;
        }
    };
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
    if crate::prompt::confirm(&format!("Delete profile \"{}\"?", name), false) != Some(true) {
        logger::info("Cancelled");
        return 0;
    }
    let _ = crate::config::save_config_mut(|raw| {
        if let Some(serde_json::Value::Object(map)) = raw.get_mut("profiles") {
            map.remove(&name);
        }
        if raw.get("activeProfile").and_then(|v| v.as_str()) == Some(name.as_str()) {
            raw.remove("activeProfile");
        }
    });
    logger::success(&format!("Profile \"{}\" deleted", name));
    0
}
