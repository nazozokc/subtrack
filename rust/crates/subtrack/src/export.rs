//! `export` command — port of `export.ts` (handleExport) for the stdout
//! formats (json/csv/md). excel/ics/output-file are later phases.

use crate::db::{get_subscriptions, QueryOptions, Subscription};
use crate::json::{list_json, number};
use crate::logger::{self, log, success};
use std::fs;
use crate::parser::ParsedArgs;

const SUPPORTED: [&str; 5] = ["csv", "json", "md", "excel", "ics"];

pub fn run_export(parsed: &ParsedArgs, depth: usize) -> i32 {
    let format = parsed
        .positionals
        .get(depth)
        .cloned()
        .unwrap_or_else(|| "undefined".to_string());

    if !SUPPORTED.contains(&format.as_str()) {
        logger::fail(&format!(
            "Unsupported export format: \"{}\". Supported: {}",
            format,
            SUPPORTED.join(", ")
        ));
        return 1;
    }

    let tags: Vec<String> = parsed
        .str("tags")
        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    let mut list = match get_subscriptions(&QueryOptions {
        include_archived: true,
        tags,
        ..Default::default()
    }) {
        Ok(l) => l,
        Err(e) => {
            logger::fail(&format!("Failed to read subscriptions: {}", e));
            return 1;
        }
    };

    if let Some(status) = parsed.str("status") {
        let statuses: Vec<String> = status
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .collect();
        list.retain(|s| statuses.contains(&s.status));
    }

    if list.is_empty() {
        logger::info("No subscriptions found — try `subtrack add`");
        return 0;
    }

    // Currency conversion is handled by the caller phase; original currencies
    // are shown when no target is requested.
    let content = match format.as_str() {
        "json" => export_json(&list),
        "csv" => export_csv(&list),
        "md" => export_md(&list),
        _ => {
            // excel/ics not yet ported.
            return 0;
        }
    };

    // `consola.log(content)` — content already ends in "\n" for json.
    if let Some(outpath) = parsed.str("output").or_else(|| parsed.str("o")) {
        let cwd = std::env::current_dir().unwrap_or_default();
        let path = cwd.join(outpath);
        if let Err(e) = fs::write(&path, content.as_bytes()) {
            logger::fail(&format!("Failed to write file: {}", e));
            return 1;
        }
        success(&format!("Exported to: {}", path.display()));
        return 0;
    }
    log(&content);
    0
}

fn export_json(subs: &[Subscription]) -> String {
    format!("{}\n", list_json(subs))
}

fn escape_md_cell(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace(['\r', '\n'], " ")
}

fn export_md(subs: &[Subscription]) -> String {
    let header = "| name | status | cycle | tags | price | currency | notes | payment_method | contract | vendor | plan | discount |";
    let separator = "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |";
    let rows: Vec<String> = subs
        .iter()
        .map(|s| {
            let tags = if s.tags.is_empty() {
                "-".to_string()
            } else {
                s.tags
                    .iter()
                    .map(|t| escape_md_cell(t))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let price = crate::price::format_price(s.price, &s.currency);
            let notes = or_dash(&escape_md_cell(s.notes.as_deref().unwrap_or("")));
            let pm = or_dash(&escape_md_cell(s.payment_method.as_deref().unwrap_or("")));
            let contract = match &s.contract_start {
                Some(start) => match &s.contract_end {
                    Some(end) => format!("{}~{}", start, end),
                    None => format!("{}~ongoing", start),
                },
                None => "-".to_string(),
            };
            let vendor = or_dash(&escape_md_cell(s.vendor_name.as_deref().unwrap_or("")));
            let tier = or_dash(&escape_md_cell(s.plan_tier.as_deref().unwrap_or("")));
            let discount = match s.discount_amount {
                Some(a) => {
                    let suffix = if s.discount_type.as_deref() == Some("percentage") {
                        "%"
                    } else {
                        ""
                    };
                    format!("{}{}", number(a), suffix)
                }
                None => "-".to_string(),
            };
            format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                escape_md_cell(&s.name),
                s.status,
                s.cycle,
                tags,
                price,
                s.currency,
                notes,
                pm,
                contract,
                vendor,
                tier,
                discount
            )
        })
        .collect();
    [header, separator]
        .into_iter()
        .chain(rows.iter().map(|s| s.as_str()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn or_dash(s: &str) -> String {
    if s.is_empty() {
        "-".to_string()
    } else {
        s.to_string()
    }
}

/// Port of `escapeCsv` (control-char strip + CSV-injection guard + quoting).
fn escape_csv(value: &str) -> String {
    let mut cleaned: String = value
        .chars()
        .filter(|c| {
            let u = *c as u32;
            !((0x00..=0x08).contains(&u)
                || u == 0x0B
                || u == 0x0C
                || (0x0E..=0x1F).contains(&u)
                || u == 0x7F)
        })
        .collect();

    let trimmed = cleaned.trim_start();
    if let Some(first) = trimmed.chars().next() {
        if matches!(first, '=' | '+' | '-' | '@' | '\t' | '\r') {
            cleaned = format!("\t{}", cleaned);
        }
    }
    if cleaned.contains('"') || cleaned.contains(',') || cleaned.contains('\n') || cleaned.contains('\r')
    {
        return format!("\"{}\"", cleaned.replace('"', "\"\""));
    }
    cleaned
}

fn csv_field(v: Option<&str>) -> String {
    match v {
        Some(s) => escape_csv(s),
        None => String::new(),
    }
}

fn export_csv(subs: &[Subscription]) -> String {
    let header = "name,status,cycle,tags,price,currency,notes,payment_method,contract_start,contract_end,auto_renewal,vendor_name,vendor_url,plan_tier,discount_amount,discount_type,billing_day";
    let rows: Vec<String> = subs
        .iter()
        .map(|s| {
            let tags = s
                .tags
                .iter()
                .map(|t| escape_csv(t))
                .collect::<Vec<_>>()
                .join(";");
            let fields = [
                csv_field(Some(&s.name)),
                csv_field(Some(&s.status)),
                csv_field(Some(&s.cycle)),
                tags,
                csv_field(Some(&number(s.price))),
                csv_field(Some(&s.currency)),
                csv_field(s.notes.as_deref()),
                csv_field(s.payment_method.as_deref()),
                csv_field(s.contract_start.as_deref()),
                csv_field(s.contract_end.as_deref()),
                csv_field(Some(if s.auto_renewal { "true" } else { "false" })),
                csv_field(s.vendor_name.as_deref()),
                csv_field(s.vendor_url.as_deref()),
                csv_field(s.plan_tier.as_deref()),
                csv_field(s.discount_amount.map(number).as_deref()),
                csv_field(s.discount_type.as_deref()),
                csv_field(s.billing_day.map(|d| d.to_string()).as_deref()),
            ];
            fields.join(",")
        })
        .collect();
    let body = std::iter::once(header.to_string())
        .chain(rows)
        .collect::<Vec<_>>()
        .join("\n");
    format!("\u{FEFF}{}", body)
}
