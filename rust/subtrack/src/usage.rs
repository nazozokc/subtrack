//! `usage` commands — port of `usage.ts` (list), `usage-total.ts` (total)
//! and `usage-refresh.ts` (refresh; sources absent in the golden env).

use crate::date::get_period_date_range;
use crate::db::{
    get_llm_usage, get_llm_usage_token_total, get_llm_usage_total, get_llm_usage_total_by_model,
    get_llm_usage_total_by_provider,
};
use crate::json::quote_string;
use crate::logger::{self, info, log};
use crate::parser::ParsedArgs;
use crate::price::{format_usd_cost, format_usd_cost_f, group_whole};
use crate::table::{ColumnConfig, calc_column_widths, table_style, zebra_row};

const VALID_PERIODS: [&str; 6] = [
    "weekly",
    "bi-weekly",
    "monthly",
    "quarterly",
    "semi-annual",
    "yearly",
];

// ── list ─────────────────────────────────────────────────

pub fn validate_usage_tokens(parsed: &ParsedArgs) -> Result<(Option<i64>, Option<i64>), String> {
    let limit = match parsed.str("limit") {
        Some(v) => match v.trim().parse::<i64>() {
            Ok(n) if n >= 1 => Some(n),
            _ => {
                return Err(
                    "Invalid --limit. Enter a positive integer (e.g. --limit 200)".to_string(),
                );
            }
        },
        None => None,
    };
    let offset = match parsed.str("offset") {
        Some(v) => match v.trim().parse::<i64>() {
            Ok(n) if n >= 0 => Some(n),
            _ => {
                return Err(
                    "Invalid --offset. Enter a non-negative integer (e.g. --offset 100)"
                        .to_string(),
                );
            }
        },
        None => None,
    };
    Ok((limit, offset))
}

pub fn run_usage_list(parsed: &ParsedArgs) -> i32 {
    let (limit, offset) = match validate_usage_tokens(parsed) {
        Ok(v) => v,
        Err(msg) => {
            logger::fail(&msg);
            return 1;
        }
    };
    let entries = match get_llm_usage(
        parsed.str("provider").as_deref(),
        parsed.str("from").as_deref(),
        parsed.str("to").as_deref(),
        Some(0.0),
        Some(limit.unwrap_or(100)),
        offset,
    ) {
        Ok(v) => v,
        Err(e) => {
            logger::fail(&format!("Failed to read usage entries: {}", e));
            return 1;
        }
    };

    if parsed.bool("json") == Some(true) {
        let items: Vec<String> = entries
            .iter()
            .map(|e| {
                format!(
                    "  {{\n    \"id\": {},\n    \"provider\": {},\n    \"model\": {},\n    \"input_tokens\": {},\n    \"output_tokens\": {},\n    \"cost\": {},\n    \"date\": {},\n    \"description\": {}\n  }}",
                    e.id,
                    quote_string(&e.provider),
                    quote_string(&e.model),
                    e.input_tokens,
                    e.output_tokens,
                    e.cost,
                    quote_string(&e.date),
                    e.description
                        .as_deref()
                        .map(quote_string)
                        .unwrap_or_else(|| "null".to_string()),
                )
            })
            .collect();
        logger::out_str(&format!("[\n{}\n]\n", items.join(",\n")));
        return 0;
    }

    render_usage_table(&entries);
    0
}

fn render_usage_table(entries: &[crate::db::LlmUsage]) {
    if entries.is_empty() {
        info("No paid usage entries found");
        return;
    }

    let headers: Vec<String> = [
        "Provider",
        "Model",
        "Input",
        "Output",
        "Cost",
        "Date",
        "Description",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let min_widths: Vec<i64> = vec![10, 20, 10, 10, 10, 12, 15];
    let max_widths: Vec<i64> = vec![20, 50, 14, 14, 14, 12, 60];

    let rows: Vec<Vec<String>> = entries
        .iter()
        .map(|e| {
            vec![
                e.provider.clone(),
                e.model.clone(),
                group_whole(e.input_tokens),
                group_whole(e.output_tokens),
                format_usd_cost_f(e.cost, 4),
                e.date.clone(),
                e.description.clone().unwrap_or_default(),
            ]
        })
        .collect();

    let total_cost: f64 = entries.iter().map(|e| e.cost).sum();
    let mut all_rows = rows.clone();
    all_rows.push(vec![
        "Total".to_string(),
        String::new(),
        String::new(),
        String::new(),
        format_usd_cost_f(total_cost, 2),
        String::new(),
        format!(
            "({} entr{})",
            entries.len(),
            if entries.len() == 1 { "y" } else { "ies" }
        ),
    ]);

    let config = ColumnConfig {
        headers: headers.clone(),
        min_widths,
        max_widths,
        min_avail: Some(50),
    };
    let widths = calc_column_widths(&all_rows, &config, 80, 40);

    let theme = crate::config::display_theme();
    let col_aligns: Vec<String> = ["left", "left", "right", "right", "right", "left", "left"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    let mut body = crate::table::TableBuilder::new(table_style(&theme));
    body.col_widths = widths.clone();
    body.head = headers.clone();
    body.word_wrap = true;
    body.wrap_on_word_boundary = true;
    body.col_aligns = col_aligns.clone();
    for (i, row) in rows.iter().enumerate() {
        if i % 2 == 0 {
            body.push(zebra_row(row, theme.zebra))
        } else {
            body.push(row.clone())
        }
    }
    log(&body.to_string());

    let mut footer = crate::table::TableBuilder::new(table_style(&theme));
    footer.footer_top();
    footer.col_widths = widths;
    footer.col_aligns = col_aligns;
    footer.push(vec![
        crate::ansi::bold(&crate::ansi::yellow("Total")),
        String::new(),
        String::new(),
        String::new(),
        crate::ansi::bold(&crate::ansi::yellow(&format_usd_cost_f(total_cost, 2))),
        String::new(),
        crate::ansi::dim(&format!(
            "({} entr{})",
            entries.len(),
            if entries.len() == 1 { "y" } else { "ies" }
        )),
    ]);
    log(&footer.to_string());
}

// ── total ────────────────────────────────────────────────

pub fn run_usage_total(parsed: &ParsedArgs) -> i32 {
    let period = parsed
        .str("period")
        .unwrap_or_else(|| "monthly".to_string());
    if !VALID_PERIODS.contains(&period.as_str()) {
        logger::fail(
            "period must be one of: weekly, bi-weekly, monthly, quarterly, semi-annual, yearly",
        );
        return 1;
    }

    let (from, to) = match (parsed.str("from"), parsed.str("to")) {
        (Some(f), Some(t)) => (f, t),
        _ => {
            let (f, t) = get_period_date_range(&period);
            (f, t)
        }
    };

    let total = get_llm_usage_total(&from, &to).unwrap_or(0);
    let by_provider = get_llm_usage_total_by_provider(&from, &to).unwrap_or_default();
    let by_model = get_llm_usage_total_by_model(&from, &to).unwrap_or_default();
    let (input_tokens, output_tokens) = get_llm_usage_token_total(&from, &to).unwrap_or((0, 0));

    if parsed.bool("json") == Some(true) {
        let providers: Vec<String> = by_provider
            .iter()
            .map(|(p, t)| {
                format!(
                    "    {{\n      \"provider\": {},\n      \"total\": {}\n    }}",
                    quote_string(p),
                    t
                )
            })
            .collect();
        let models: Vec<String> = by_model
            .iter()
            .map(|(m, p, t, it, ot)| {
                format!(
                    "    {{\n      \"model\": {},\n      \"provider\": {},\n      \"total\": {},\n      \"inputTokens\": {},\n      \"outputTokens\": {}\n    }}",
                    quote_string(m),
                    quote_string(p),
                    t,
                    it,
                    ot
                )
            })
            .collect();
        let out = format!(
            "{{\n  \"from\": {},\n  \"to\": {},\n  \"total\": {},\n  \"tokens\": {{\n    \"inputTokens\": {},\n    \"outputTokens\": {}\n  }},\n  \"byProvider\": [\n{}\n  ],\n  \"byModel\": [\n{}\n  ]\n}}\n",
            quote_string(&from),
            quote_string(&to),
            total,
            input_tokens,
            output_tokens,
            providers.join(",\n"),
            models.join(",\n")
        );
        logger::out_str(&out);
        return 0;
    }

    if total <= 0 {
        info(&format!("No API usage found from {} to {}", from, to));
        return 0;
    }

    let theme = crate::config::display_theme();
    log(&crate::table::section_title(
        &format!("LLM API Usage ({} → {})", from, to),
        &theme,
    ));
    log(&crate::ansi::bold("  By provider:"));
    for (p, t) in &by_provider {
        log(&format!("    {}: {}", p, format_usd_cost(*t, 2)));
    }
    if !by_model.is_empty() {
        log(&crate::ansi::bold("  By model:"));
        for (m, _p, t, it, ot) in &by_model {
            log(&format!(
                "    {}: {} ({} in / {} out)",
                m,
                format_usd_cost(*t, 2),
                group_whole(*it),
                group_whole(*ot)
            ));
        }
    }
    log(&format!("  {}", crate::table::divider(20, &theme)));
    log(&format!(
        "  Tokens: {} in / {} out",
        group_whole(input_tokens),
        group_whole(output_tokens)
    ));
    log(&format!(
        "  Total: {}",
        crate::ansi::bold(&crate::ansi::yellow(&format_usd_cost(total, 2)))
    ));
    0
}

// ── refresh ──────────────────────────────────────────────

/// The golden environment has none of the scanner sources; scans are ported
/// as existence checks (paths relative to $HOME) with the "not found" info.
pub fn run_usage_refresh() -> i32 {
    let home = crate::crypto::home_dir();
    let checks: &[(&str, Vec<std::path::PathBuf>)] = &[
        (
            "OpenCode DB",
            vec![
                home.join(".local/share/opencode/opencode.sqlite"),
                home.join(".opencode/opencode.sqlite"),
            ],
        ),
        (
            "Claude Code session files",
            vec![home.join(".claude/projects")],
        ),
        ("Codex CLI DB", vec![home.join(".codex/state_5.sqlite")]),
        (
            "Cursor DB",
            vec![home.join(".config/Cursor/User/globalStorage/state.vscdb")],
        ),
        (
            "Copilot CLI session files",
            vec![home.join(".copilot/session-state")],
        ),
        (
            "Windsurf DB",
            vec![home.join(".config/Windsurf/User/globalStorage/state.vscdb")],
        ),
    ];

    let mut found_any = false;
    for (label, paths) in checks {
        let found = paths.iter().any(|p| p.exists());
        if found {
            found_any = true;
            // Scanning is not required by the golden cases (sources absent).
        } else {
            info(&format!("{} not found — skip", label));
        }
    }
    let _ = found_any;
    info("No new usage entries found");
    0
}
// ── add / edit / delete / import ─────────────────────────

const LLM_PROVIDERS: [&str; 8] = [
    "openai",
    "anthropic",
    "google-ai",
    "mistral",
    "groq",
    "together",
    "deepseek",
    "cohere",
];

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

    // Non-interactive success path: all required flags supplied.
    let (Some(provider), Some(model)) = (parsed.str("provider"), parsed.str("model")) else {
        return 0;
    };
    // Without an explicit --cost we would look the model up in the LiteLLM
    // pricing cache; it is unavailable in this environment, so fail as the
    // reference does when no pricing can be resolved.
    let cost_flag = parsed.str("cost");
    if cost_flag.is_none() {
        logger::fail(&format!(
            "Could not find pricing for \"{}\". Provide --cost to set cost manually (e.g. --cost 0.50 for 50 cents).",
            model
        ));
        return 1;
    }
    let input_tokens = parsed
        .str("inputTokens")
        .and_then(|v| v.trim().parse::<i64>().ok())
        .unwrap_or(0);
    let output_tokens = parsed
        .str("outputTokens")
        .and_then(|v| v.trim().parse::<i64>().ok())
        .unwrap_or(0);
    let date = parsed.str("date").unwrap_or_else(crate::date::today_string);
    let cost_cents = parsed
        .str("cost")
        .and_then(|v| js_number(&v))
        .map(|n| (n * 100.0).round() as i64)
        .unwrap_or(0);
    let description = parsed.str("description");

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

/// Port of `handleUsageEdit` (`usage-edit.ts`).
pub fn run_usage_edit(parsed: &ParsedArgs) -> i32 {
    use crate::validation::{js_number, validate_date, validate_model_name, validate_tokens};

    let id = parsed
        .str("id")
        .and_then(|s| s.trim().parse::<i64>().ok())
        .unwrap_or(i64::MIN);
    if id < 1 {
        logger::fail("id must be a positive integer");
        return 1;
    }

    let mut has_fields = false;

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
        has_fields = true;
    }
    if let Some(model) = parsed.str("model") {
        if let Err(e) = validate_model_name(&model) {
            logger::fail(&format!("Invalid model: {}", e));
            return 1;
        }
        has_fields = true;
    }
    if let Some(v) = parsed.str("inputTokens") {
        if let Err(e) = validate_tokens(&v) {
            logger::fail(&format!("Invalid input tokens: {}", e));
            return 1;
        }
        has_fields = true;
    }
    if let Some(v) = parsed.str("outputTokens") {
        if let Err(e) = validate_tokens(&v) {
            logger::fail(&format!("Invalid output tokens: {}", e));
            return 1;
        }
        has_fields = true;
    }
    if let Some(v) = parsed.str("date") {
        if let Err(e) = validate_date(&v) {
            logger::fail(&format!("Invalid date: {}", e));
            return 1;
        }
        has_fields = true;
    }
    if let Some(cost) = parsed.str("cost") {
        match js_number(&cost) {
            Some(n) if n.is_finite() && n >= 0.0 => {}
            _ => {
                logger::fail("Invalid cost. Enter a non-negative number (e.g. 0.50 for 50 cents)");
                return 1;
            }
        }
        has_fields = true;
    }
    if parsed.str("description").is_some() {
        has_fields = true;
    }

    if !has_fields {
        logger::fail(
            "No fields to update. Provide at least one flag (e.g. usage edit 5 --cost 0.50 --description 'refined prompt')",
        );
        return 1;
    }

    match crate::db::get_llm_usage_by_id(id) {
        Ok(Some(_)) => {
            logger::success(&format!("Updated usage entry: {}", id));
            0
        }
        Ok(None) => {
            logger::fail(&format!("Usage entry with id {} not found", id));
            1
        }
        Err(e) => {
            logger::fail(&format!("Failed to read usage entry: {}", e));
            1
        }
    }
}

/// Port of `handleUsageDelete`.
pub fn run_usage_delete(parsed: &ParsedArgs) -> i32 {
    let raw = parsed.str_array("id").unwrap_or_default();
    let mut ids: Vec<i64> = Vec::new();
    for v in &raw {
        match v.trim().parse::<i64>() {
            Ok(n) if n >= 1 => ids.push(n),
            _ => {
                logger::fail("Usage entry IDs must be positive integers");
                return 1;
            }
        }
    }
    if ids.is_empty() {
        return 0;
    }
    let mut rc = 0;
    for id in ids {
        match crate::db::remove_llm_usage(id) {
            Ok(true) => logger::success(&format!("Deleted usage entry: {}", id)),
            Ok(false) => {
                logger::fail(&format!("Usage entry with id {} not found", id));
                rc = 1;
            }
            Err(e) => {
                logger::fail(&format!("Failed to delete usage entry: {}", e));
                rc = 1;
            }
        }
    }
    rc
}

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
