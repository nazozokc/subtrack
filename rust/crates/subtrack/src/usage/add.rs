use crate::logger;
use crate::parser::ParsedArgs;

use super::{LLM_PROVIDERS, validate_cost_input};

// ── add / edit / delete / import ─────────────────────────

/// Port of `handleUsageAdd` (`usage-add.ts`) flag validation.
pub fn run_usage_add(parsed: &ParsedArgs) -> i32 {
    use crate::validation::{js_number, validate_date, validate_model_name, validate_tokens};

    if let Some(cost) = parsed.str("cost") {
        match js_number(&cost) {
            Some(n) if n.is_finite() && n >= 0.0 => {}
            _ => {
                logger::fail("Invalid cost. Enter a non-negative number (e.g. 0.50 for 50 cents)");
                return 1;
            }
        }
    }

    if let Some(provider) = parsed.str("provider")
        && !LLM_PROVIDERS.contains(&provider.as_str())
    {
        logger::fail(&format!(
            "Invalid provider \"{}\". Use one of: openai, anthropic, google-ai, mistral, \
groq, together, deepseek, cohere, or omit for interactive.",
            provider
        ));
        return 1;
    }

    if let Some(model) = parsed.str("model")
        && let Err(e) = validate_model_name(&model)
    {
        logger::fail(&e);
        return 1;
    }

    if let Some(v) = parsed.str("inputTokens")
        && let Err(e) = validate_tokens(&v)
    {
        logger::fail(&e);
        return 1;
    }
    if let Some(v) = parsed.str("outputTokens")
        && let Err(e) = validate_tokens(&v)
    {
        logger::fail(&e);
        return 1;
    }
    if let Some(v) = parsed.str("date")
        && let Err(e) = validate_date(&v)
    {
        logger::fail(&e);
        return 1;
    }

    // Interactive fallback: prompt for anything not supplied as a flag.
    let provider = match parsed.str("provider") {
        Some(p) => p,
        None if crate::prompt::interactive() => {
            let entries: Vec<crate::prompt::Choice> = LLM_PROVIDERS
                .iter()
                .map(|p| crate::prompt::Choice::plain(p))
                .collect();
            let Some(p) = crate::prompt::select("Provider", &entries, Some(0)) else {
                return 0;
            };
            p
        }
        None => return 0,
    };
    let model = match parsed.str("model") {
        Some(m) => m,
        None if crate::prompt::interactive() => {
            match crate::prompt::input("Model (e.g. gpt-4o)", Some(""), Some(validate_model_name)) {
                Some(m) => m,
                None => return 0,
            }
        }
        None => return 0,
    };
    // Without an explicit --cost we would look the model up in the LiteLLM
    // pricing cache; it is unavailable in this environment, so prompt for it
    // when interactive and otherwise fail as the reference does.
    let cost_cents = match parsed.str("cost") {
        Some(raw) => js_number(&raw)
            .map(|n| (n * 100.0).round() as i64)
            .unwrap_or(0),
        None if crate::prompt::interactive() => {
            let Some(raw) = crate::prompt::input(
                "Cost (USD, e.g. 0.50)",
                Some("0.00"),
                Some(validate_cost_input),
            ) else {
                return 0;
            };
            js_number(raw.trim())
                .map(|n| (n * 100.0).round() as i64)
                .unwrap_or(0)
        }
        None => {
            logger::fail(&format!(
                "Could not find pricing for \"{}\". Provide --cost to set cost manually (e.g. --cost 0.50 for 50 cents).",
                model
            ));
            return 1;
        }
    };
    let input_tokens = match parsed.str("inputTokens") {
        Some(v) => v.trim().parse::<i64>().unwrap_or(0),
        None if crate::prompt::interactive() => {
            match crate::prompt::input("Input tokens", Some("0"), Some(validate_tokens)) {
                Some(v) => v.trim().parse::<i64>().unwrap_or(0),
                None => return 0,
            }
        }
        None => 0,
    };
    let output_tokens = match parsed.str("outputTokens") {
        Some(v) => v.trim().parse::<i64>().unwrap_or(0),
        None if crate::prompt::interactive() => {
            match crate::prompt::input("Output tokens", Some("0"), Some(validate_tokens)) {
                Some(v) => v.trim().parse::<i64>().unwrap_or(0),
                None => return 0,
            }
        }
        None => 0,
    };
    let date = match parsed.str("date") {
        Some(v) => v,
        None if crate::prompt::interactive() => {
            let today = crate::date::today_string();
            match crate::prompt::input(
                "Date (YYYY-MM-DD)",
                Some(today.as_str()),
                Some(validate_date),
            ) {
                Some(v) if !v.trim().is_empty() => v,
                Some(_) => today,
                None => return 0,
            }
        }
        None => crate::date::today_string(),
    };
    let description = match parsed.str("description") {
        Some(v) => Some(v),
        None if crate::prompt::interactive() => {
            let Some(v) = crate::prompt::input("Description (optional)", Some(""), None) else {
                return 0;
            };
            let trimmed = v.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        None => None,
    };

    let conn = match crate::db::open_db() {
        Ok(c) => c,
        Err(e) => {
            logger::fail(&format!("Failed to add usage entry: {}", e));
            return 1;
        }
    };
    if let Err(e) = crate::db::insert_llm_usage(
        &conn,
        &provider,
        &model,
        input_tokens,
        output_tokens,
        cost_cents,
        &date,
        description.as_deref(),
        true,
    ) {
        logger::fail(&format!("Failed to add usage entry: {}", e));
        return 1;
    }
    let _ = crate::db::add_audit_log(
        &conn,
        "usage.add",
        Some("usage"),
        None,
        Some(&format!(
            "{}/{}: {} on {}",
            provider,
            model,
            crate::price::format_usd_cost(cost_cents, 4),
            date
        )),
        true,
    );
    logger::success(&format!(
        "Added usage: {}/{} — {} on {}",
        provider,
        model,
        crate::price::format_usd_cost(cost_cents, 4),
        date
    ));
    0
}
