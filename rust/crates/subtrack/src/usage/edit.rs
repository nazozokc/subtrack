use crate::logger::{self, info};
use crate::parser::ParsedArgs;

use super::{LLM_PROVIDERS, validate_cost_input};

/// Prompt for a usage entry id.
fn pick_usage(message: &str) -> Option<i64> {
    let entries = match crate::db::get_llm_usage(None, None, None, None, Some(50), None) {
        Ok(v) => v,
        Err(e) => {
            logger::fail(&format!("Failed to read usage entries: {}", e));
            return None;
        }
    };
    if entries.is_empty() {
        info("No usage entries found — try `subtrack usage add`");
        return None;
    }
    let choices: Vec<crate::prompt::Choice> = entries
        .iter()
        .map(|u| {
            crate::prompt::Choice::new(
                &format!(
                    "#{} {}/{} — {} on {}",
                    u.id,
                    u.provider,
                    u.model,
                    crate::price::format_usd_cost_f(u.cost, 4),
                    u.date
                ),
                &u.id.to_string(),
            )
        })
        .collect();
    crate::prompt::select(message, &choices, None)?
        .parse::<i64>()
        .ok()
}

/// Port of `handleUsageEdit` (`usage-edit.ts`).
pub fn run_usage_edit(parsed: &ParsedArgs) -> i32 {
    use crate::db::SqlValue;
    use crate::validation::{js_number, validate_date, validate_model_name, validate_tokens};

    let id = match parsed.str("id").and_then(|s| s.trim().parse::<i64>().ok()) {
        Some(id) => id,
        None if crate::prompt::interactive() => match pick_usage("select usage entry to edit") {
            Some(id) => id,
            None => return 0,
        },
        None => {
            logger::fail("id must be a positive integer");
            return 1;
        }
    };
    if id < 1 {
        logger::fail("id must be a positive integer");
        return 1;
    }

    let mut sets: Vec<(&str, SqlValue)> = Vec::new();

    // Flags first (validated as in the reference).
    if let Some(provider) = parsed.str("provider") {
        if provider == "__other__" {
            logger::fail(
                "Invalid provider \"__other__\" — provide the custom provider name directly (e.g. --provider my-custom-llm)",
            );
            return 1;
        }
        if !LLM_PROVIDERS.contains(&provider.as_str()) {
            logger::fail(&format!(
                "Invalid provider \"{}\". Use one of: openai, anthropic, google-ai, mistral, groq, together, deepseek, cohere, or a custom name.",
                provider
            ));
            return 1;
        }
        sets.push(("provider", SqlValue::Text(provider)));
    }
    if let Some(model) = parsed.str("model") {
        if let Err(e) = validate_model_name(&model) {
            logger::fail(&format!("Invalid model: {}", e));
            return 1;
        }
        sets.push(("model", SqlValue::Text(model)));
    }
    if let Some(v) = parsed.str("inputTokens") {
        if let Err(e) = validate_tokens(&v) {
            logger::fail(&format!("Invalid input tokens: {}", e));
            return 1;
        }
        sets.push(("input_tokens", SqlValue::Int(v.trim().parse().unwrap_or(0))));
    }
    if let Some(v) = parsed.str("outputTokens") {
        if let Err(e) = validate_tokens(&v) {
            logger::fail(&format!("Invalid output tokens: {}", e));
            return 1;
        }
        sets.push((
            "output_tokens",
            SqlValue::Int(v.trim().parse().unwrap_or(0)),
        ));
    }
    if let Some(v) = parsed.str("date") {
        if let Err(e) = validate_date(&v) {
            logger::fail(&format!("Invalid date: {}", e));
            return 1;
        }
        if !v.trim().is_empty() {
            sets.push(("date", SqlValue::Text(v)));
        }
    }
    if let Some(cost) = parsed.str("cost") {
        match js_number(&cost) {
            Some(n) if n.is_finite() && n >= 0.0 => {
                sets.push(("cost", SqlValue::Int((n * 100.0).round() as i64)));
            }
            _ => {
                logger::fail("Invalid cost. Enter a non-negative number (e.g. 0.50 for 50 cents)");
                return 1;
            }
        }
    }
    if let Some(description) = parsed.str("description") {
        let trimmed = description.trim();
        sets.push((
            "description",
            if trimmed.is_empty() {
                SqlValue::Null
            } else {
                SqlValue::Text(trimmed.to_string())
            },
        ));
    }

    if sets.is_empty() && crate::prompt::interactive() {
        let choices = vec![
            crate::prompt::Choice::new("provider", "provider"),
            crate::prompt::Choice::new("model", "model"),
            crate::prompt::Choice::new("input tokens", "inputTokens"),
            crate::prompt::Choice::new("output tokens", "outputTokens"),
            crate::prompt::Choice::new("cost", "cost"),
            crate::prompt::Choice::new("date", "date"),
            crate::prompt::Choice::new("description", "description"),
        ];
        let Some(fields) = crate::prompt::checkbox("select fields to edit", &choices) else {
            return 0;
        };
        if fields.is_empty() {
            info("Cancelled");
            return 0;
        }
        for field in fields {
            match field.as_str() {
                "provider" => {
                    let entries: Vec<crate::prompt::Choice> = LLM_PROVIDERS
                        .iter()
                        .map(|p| crate::prompt::Choice::plain(p))
                        .collect();
                    let Some(v) = crate::prompt::select("Provider", &entries, Some(0)) else {
                        return 0;
                    };
                    sets.push(("provider", SqlValue::Text(v)));
                }
                "model" => {
                    let Some(v) =
                        crate::prompt::input("Model", Some(""), Some(validate_model_name))
                    else {
                        return 0;
                    };
                    sets.push(("model", SqlValue::Text(v)));
                }
                "inputTokens" => {
                    let Some(v) =
                        crate::prompt::input("Input tokens", Some("0"), Some(validate_tokens))
                    else {
                        return 0;
                    };
                    sets.push(("input_tokens", SqlValue::Int(v.trim().parse().unwrap_or(0))));
                }
                "outputTokens" => {
                    let Some(v) =
                        crate::prompt::input("Output tokens", Some("0"), Some(validate_tokens))
                    else {
                        return 0;
                    };
                    sets.push((
                        "output_tokens",
                        SqlValue::Int(v.trim().parse().unwrap_or(0)),
                    ));
                }
                "cost" => {
                    let Some(v) = crate::prompt::input(
                        "Cost (USD, e.g. 0.50)",
                        Some("0.00"),
                        Some(validate_cost_input),
                    ) else {
                        return 0;
                    };
                    let cents = js_number(v.trim())
                        .map(|n| (n * 100.0).round() as i64)
                        .unwrap_or(0);
                    sets.push(("cost", SqlValue::Int(cents)));
                }
                "date" => {
                    let today = crate::date::today_string();
                    let Some(v) = crate::prompt::input(
                        "Date (YYYY-MM-DD)",
                        Some(today.as_str()),
                        Some(validate_date),
                    ) else {
                        return 0;
                    };
                    sets.push(("date", SqlValue::Text(v)));
                }
                "description" => {
                    let Some(v) =
                        crate::prompt::input("Description (empty to clear)", Some(""), None)
                    else {
                        return 0;
                    };
                    let trimmed = v.trim();
                    sets.push((
                        "description",
                        if trimmed.is_empty() {
                            SqlValue::Null
                        } else {
                            SqlValue::Text(trimmed.to_string())
                        },
                    ));
                }
                _ => {}
            }
        }
    }

    if sets.is_empty() {
        logger::fail(
            "No fields to update. Provide at least one flag (e.g. usage edit 5 --cost 0.50 --description 'refined prompt')",
        );
        return 1;
    }

    let conn = match crate::db::open_db() {
        Ok(c) => c,
        Err(e) => {
            logger::fail(&format!("Failed to open database: {}", e));
            return 1;
        }
    };
    match crate::db::update_llm_usage(&conn, id, &sets, true) {
        Ok(true) => {
            logger::success(&format!("Updated usage entry: {}", id));
            0
        }
        Ok(false) => {
            logger::fail(&format!("Usage entry with id {} not found", id));
            1
        }
        Err(e) => {
            logger::fail(&format!("Failed to update usage entry: {}", e));
            1
        }
    }
}
