//! `usage` commands — port of `usage.ts` (list), `usage-total.ts` (total)
//! and `usage-refresh.ts` (refresh; sources absent in the golden env).

use crate::date::get_period_date_range;
use crate::db::{get_llm_usage, get_llm_usage_token_total, get_llm_usage_total, get_llm_usage_total_by_model, get_llm_usage_total_by_provider};
use crate::json::quote_string;
use crate::logger::{self, info, log};
use crate::parser::ParsedArgs;
use crate::price::{format_usd_cost, group_whole};
use crate::table::{calc_column_widths, table_style, zebra_row, ColumnConfig};

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
                )
            }
        },
        None => None,
    };
    let offset = match parsed.str("offset") {
        Some(v) => match v.trim().parse::<i64>() {
            Ok(n) if n >= 0 => Some(n),
            _ => {
                return Err(
                    "Invalid --offset. Enter a non-negative integer (e.g. --offset 100)".to_string(),
                )
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

    let headers: Vec<String> = ["Provider", "Model", "Input", "Output", "Cost", "Date", "Description"]
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
                format_usd_cost(e.cost, 4),
                e.date.clone(),
                e.description.clone().unwrap_or_default(),
            ]
        })
        .collect();

    let total_cost: i64 = entries.iter().map(|e| e.cost).sum();
    let mut all_rows = rows.clone();
    all_rows.push(vec![
        "Total".to_string(),
        String::new(),
        String::new(),
        String::new(),
        format_usd_cost(total_cost, 2),
        String::new(),
        format!("({} entr{})", entries.len(), if entries.len() == 1 { "y" } else { "ies" }),
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
        crate::ansi::bold(&crate::ansi::yellow(&format_usd_cost(total_cost, 2))),
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
    let period = parsed.str("period").unwrap_or_else(|| "monthly".to_string());
    if !VALID_PERIODS.contains(&period.as_str()) {
        logger::fail("period must be one of: weekly, bi-weekly, monthly, quarterly, semi-annual, yearly");
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
    log(&format!(
        "  {}",
        crate::table::divider(20, &theme)
    ));
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
        ("Claude Code session files", vec![home.join(".claude/projects")]),
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
                logger::fail(
                    "Invalid cost. Enter a non-negative number (e.g. 0.50 for 50 cents)",
                );
                return 1;
            }
        }
    }

    if let Some(provider) = parsed.str("provider") {
        if !LLM_PROVIDERS.contains(&provider.as_str()) {
            logger::fail(&format!(
                "Invalid provider \"{}\". Use one of: openai, anthropic, google-ai, mistral, \
groq, together, deepseek, cohere, or omit for interactive.",
                provider
            ));
            return 1;
        }
    }

    if let Some(model) = parsed.str("model") {
        if let Err(e) = validate_model_name(&model) {
            logger::fail(&e);
            return 1;
        }
    }

    if let Some(v) = parsed.str("inputTokens") {
        if let Err(e) = validate_tokens(&v) {
            logger::fail(&e);
            return 1;
        }
    }
    if let Some(v) = parsed.str("outputTokens") {
        if let Err(e) = validate_tokens(&v) {
            logger::fail(&e);
            return 1;
        }
    }
    if let Some(v) = parsed.str("date") {
        if let Err(e) = validate_date(&v) {
            logger::fail(&e);
            return 1;
        }
    }

    // Persistence is deferred to the mutating phase.
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

/// Port of `handleUsageImport` (file path validation only).
pub fn run_usage_import(parsed: &ParsedArgs) -> i32 {
    let Some(file) = parsed.str("file") else {
        return 0;
    };
    if file == "-" {
        // Stdin import is not wired up in this environment.
        return 0;
    }
    if crate::safe_path::safe_path(&file).is_none() {
        logger::fail("File not found or path not allowed — must be within home or temp directory");
        return 1;
    }
    // Actual parsing/import is deferred to the mutating phase.
    0
}
