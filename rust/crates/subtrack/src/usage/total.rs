use crate::date::get_period_date_range;
use crate::db::{
    get_llm_usage_token_total, get_llm_usage_total, get_llm_usage_total_by_model,
    get_llm_usage_total_by_provider,
};
use crate::json::quote_string;
use crate::logger::{self, info, log};
use crate::parser::ParsedArgs;
use crate::price::{format_usd_cost, group_whole};

// ── total ────────────────────────────────────────────────

pub fn run_usage_total(parsed: &ParsedArgs) -> i32 {
    let period = parsed
        .str("period")
        .unwrap_or_else(|| "monthly".to_string());
    if crate::domain::Cycle::parse_named(&period).is_none() {
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

    if parsed.flag("json") {
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
