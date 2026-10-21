use crate::logger::{self, info};
use crate::parser::ParsedArgs;

// ── import ───────────────────────────────────────────────

/// A log entry parsed from an API response object (`parseResponseJson`).
struct ParsedLogEntry {
    generation_id: String,
    provider: String,
    model: String,
    input_tokens: i64,
    output_tokens: i64,
    cost_cents: Option<f64>,
    date: String,
}

fn num_json(v: Option<&serde_json::Value>) -> Option<f64> {
    match v {
        Some(serde_json::Value::Number(n)) => n.as_f64(),
        Some(serde_json::Value::String(s)) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}
fn unix_ts_to_date(ts: Option<f64>) -> String {
    match ts {
        Some(t) if t != 0.0 => match chrono::DateTime::from_timestamp(t as i64, 0) {
            Some(dt) => dt.date_naive().format("%Y-%m-%d").to_string(),
            None => crate::date::today_string(),
        },
        _ => crate::date::today_string(),
    }
}

/// Port of `parseResponseJson`.
fn parse_response_json(obj: &serde_json::Value) -> Option<ParsedLogEntry> {
    let usage = obj.get("usage")?.as_object()?;
    let id = obj
        .get("id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())?;
    let model = obj
        .get("model")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())?;

    let is_openrouter = usage.get("cost").map(|v| v.is_number()).unwrap_or(false);
    let is_openai = !is_openrouter
        && usage
            .get("prompt_tokens")
            .map(|v| v.is_number())
            .unwrap_or(false);
    let is_anthropic = !is_openrouter
        && !is_openai
        && usage
            .get("input_tokens")
            .map(|v| v.is_number())
            .unwrap_or(false);

    if is_openrouter {
        let (provider, model_name) = match model.find('/') {
            Some(i) => (&model[..i], &model[i + 1..]),
            None => ("unknown", model),
        };
        let input_tokens = num_json(usage.get("prompt_tokens")).unwrap_or(0.0) as i64;
        let output_tokens = num_json(usage.get("completion_tokens")).unwrap_or(0.0) as i64;
        let cost_cents = num_json(usage.get("cost")).unwrap_or(0.0);
        if input_tokens == 0 && output_tokens == 0 && cost_cents == 0.0 {
            return None;
        }
        return Some(ParsedLogEntry {
            generation_id: id.to_string(),
            provider: provider.to_string(),
            model: model_name.to_string(),
            input_tokens,
            output_tokens,
            cost_cents: Some(cost_cents),
            date: crate::date::today_string(),
        });
    }

    if is_openai {
        let input_tokens = num_json(usage.get("prompt_tokens")).unwrap_or(0.0) as i64;
        let output_tokens = num_json(usage.get("completion_tokens")).unwrap_or(0.0) as i64;
        if input_tokens == 0 && output_tokens == 0 {
            return None;
        }
        return Some(ParsedLogEntry {
            generation_id: id.to_string(),
            provider: "openai".to_string(),
            model: model.to_string(),
            input_tokens,
            output_tokens,
            cost_cents: None,
            date: unix_ts_to_date(num_json(obj.get("created"))),
        });
    }

    if is_anthropic {
        let input_tokens = num_json(usage.get("input_tokens")).unwrap_or(0.0) as i64;
        let output_tokens = num_json(usage.get("output_tokens")).unwrap_or(0.0) as i64;
        if input_tokens == 0 && output_tokens == 0 {
            return None;
        }
        return Some(ParsedLogEntry {
            generation_id: id.to_string(),
            provider: "anthropic".to_string(),
            model: model.to_string(),
            input_tokens,
            output_tokens,
            cost_cents: None,
            date: crate::date::today_string(),
        });
    }

    None
}

/// Load the local LiteLLM pricing cache if present (no network access).
fn load_pricing_cache() -> Option<serde_json::Map<String, serde_json::Value>> {
    let path = crate::crypto::config_dir().join("litellm_cache.json");
    let data = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&data).ok()?;
    value.as_object().cloned()
}

fn lookup_model_key(
    cache: &serde_json::Map<String, serde_json::Value>,
    model: &str,
) -> Option<String> {
    if cache.contains_key(model) {
        return Some(model.to_string());
    }
    let lower = model.to_lowercase();
    for key in cache.keys() {
        if key.to_lowercase() == lower {
            return Some(key.clone());
        }
    }
    None
}

fn calculate_cost_cents(pricing: &serde_json::Value, input_tokens: i64, output_tokens: i64) -> f64 {
    let get = |k: &str| -> f64 { pricing.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0) };
    let input_cost = get("input_cost_per_token") * input_tokens as f64;
    let output_cost = get("output_cost_per_token") * output_tokens as f64;
    let reasoning_cost = get("output_cost_per_reasoning_token") * output_tokens as f64;
    (input_cost + output_cost + reasoning_cost) * 100.0
}

const MAX_USAGE_FILE_BYTES: u64 = 50 * 1024 * 1024;
const MAX_USAGE_STDIN_BYTES: u64 = 10 * 1024 * 1024;

/// Port of `handleUsageImport`.
pub fn run_usage_import(parsed: &ParsedArgs) -> i32 {
    let Some(file) = parsed.str("file") else {
        return 0;
    };
    let dry_run = parsed.bool("dryRun") == Some(true);

    let content = if file == "-" {
        // Read the whole of stdin, capped at 10 MB (port of the stdin branch).
        use std::io::Read;
        let mut buf: Vec<u8> = Vec::new();
        let stdin = std::io::stdin().lock();
        if let Err(e) = stdin.take(MAX_USAGE_STDIN_BYTES + 1).read_to_end(&mut buf) {
            logger::fail(&format!("Failed to read stdin: {}", e));
            return 1;
        }
        if buf.len() as u64 > MAX_USAGE_STDIN_BYTES {
            logger::fail(&format!(
                "Stdin input too large (max {} MB)",
                MAX_USAGE_STDIN_BYTES / 1024 / 1024
            ));
            return 1;
        }
        String::from_utf8_lossy(&buf).into_owned()
    } else {
        let Some(safe_file) = crate::safe_path::safe_path(&file) else {
            logger::fail(
                "File not found or path not allowed — must be within home or temp directory",
            );
            return 1;
        };
        if let Ok(meta) = std::fs::metadata(&safe_file)
            && meta.len() > MAX_USAGE_FILE_BYTES
        {
            logger::fail(&format!(
                "File too large ({:.1} MB). Maximum: {} MB",
                meta.len() as f64 / 1024.0 / 1024.0,
                MAX_USAGE_FILE_BYTES / 1024 / 1024
            ));
            return 1;
        }
        match std::fs::read_to_string(&safe_file) {
            Ok(c) => c,
            Err(e) => {
                logger::fail(&format!(
                    "Cannot read file: {} — {}",
                    safe_file.display(),
                    e
                ));
                return 1;
            }
        }
    };

    if content.trim().is_empty() {
        logger::warn("File is empty");
        return 0;
    }

    // Parse into individual objects.
    let trimmed = content.trim();
    let mut objects: Vec<serde_json::Value> = Vec::new();
    if trimmed.starts_with('[') {
        match serde_json::from_str::<serde_json::Value>(trimmed) {
            Ok(v) if v.is_array() => {
                objects = v.as_array().cloned().unwrap_or_default();
            }
            Ok(_) => {
                logger::fail("File contains a JSON object, expected an array or JSONL");
                return 1;
            }
            Err(_) => {
                logger::fail("Failed to parse JSON array");
                return 1;
            }
        }
    } else {
        for (i, line) in content.split('\n').enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<serde_json::Value>(line) {
                Ok(v) => objects.push(v),
                Err(_) => logger::warn(&format!("Line {}: Failed to parse JSON, skipping", i + 1)),
            }
        }
    }

    if objects.is_empty() {
        logger::warn("No JSON objects found in file");
        return 0;
    }

    info(&format!(
        "Found {} JSON object{} in file",
        objects.len(),
        if objects.len() != 1 { "s" } else { "" }
    ));

    let cache = if dry_run { None } else { load_pricing_cache() };

    let conn = if dry_run {
        None
    } else {
        match crate::db::open_db() {
            Ok(c) => Some(c),
            Err(e) => {
                logger::fail(&format!("Failed to open database: {}", e));
                return 1;
            }
        }
    };

    let mut added = 0usize;
    let mut skipped = 0usize;
    let mut no_cost = 0usize;
    let mut errors = 0usize;

    for (i, object) in objects.iter().enumerate() {
        let Some(entry) = parse_response_json(object) else {
            errors += 1;
            continue;
        };

        let mut cost_cents = entry.cost_cents;
        if cost_cents.is_none()
            && let Some(cache) = &cache
            && let Some(model_key) = lookup_model_key(cache, &entry.model)
            && let Some(pricing) = cache.get(&model_key)
        {
            cost_cents = Some(calculate_cost_cents(
                pricing,
                entry.input_tokens,
                entry.output_tokens,
            ));
        }

        let Some(cost) = cost_cents else {
            logger::warn(&format!(
                "Entry {} ({}): Cannot determine cost for \"{}/{}\", skipping",
                i + 1,
                entry.generation_id,
                entry.provider,
                entry.model
            ));
            no_cost += 1;
            continue;
        };

        if dry_run {
            added += 1;
            continue;
        }

        let Some(conn) = &conn else {
            return 1;
        };
        match crate::db::add_llm_usage_from_log(
            conn,
            &entry.provider,
            &entry.model,
            entry.input_tokens,
            entry.output_tokens,
            cost,
            &entry.date,
            None,
            &entry.generation_id,
            true,
        ) {
            Ok(true) => added += 1,
            Ok(false) => skipped += 1,
            Err(e) => {
                logger::fail(&format!("Failed to add usage entry: {}", e));
                return 1;
            }
        }
    }

    if dry_run {
        let mut msg = format!("Dry run: {} entries would be added", added);
        if skipped > 0 {
            msg.push_str(&format!(
                ", {} duplicate{}",
                skipped,
                if skipped != 1 { "s" } else { "" }
            ));
        }
        if no_cost > 0 {
            msg.push_str(&format!(", {} skipped (no cost data)", no_cost));
        }
        if errors > 0 {
            msg.push_str(&format!(
                ", {} unparsable {}",
                errors,
                if errors != 1 { "entries" } else { "entry" }
            ));
        }
        info(&msg);
    } else {
        let mut msg = format!("Imported: {} entries added", added);
        if skipped > 0 {
            msg.push_str(&format!(
                ", {} duplicate{} skipped",
                skipped,
                if skipped != 1 { "s" } else { "" }
            ));
        }
        if no_cost > 0 {
            msg.push_str(&format!(", {} skipped (no cost data)", no_cost));
        }
        if errors > 0 {
            msg.push_str(&format!(
                ", {} unparsable {}",
                errors,
                if errors != 1 { "entries" } else { "entry" }
            ));
        }
        logger::success(&msg);
    }

    0
}
