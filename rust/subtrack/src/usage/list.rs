use crate::db::get_llm_usage;
use crate::json::quote_string;
use crate::logger::{self, info, log};
use crate::parser::ParsedArgs;
use crate::price::{format_usd_cost_f, group_whole};
use crate::table::{ColumnConfig, calc_column_widths, table_style, zebra_row};

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
    let widths = calc_column_widths(&all_rows, &config, crate::ui::term_width(), 40);

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
