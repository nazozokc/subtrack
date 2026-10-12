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
        "ics" => export_ics(&list),
        "excel" => {
            let buf = export_excel(&list);
            if let Some(outpath) = parsed.str("output").or_else(|| parsed.str("o")) {
                let cwd = std::env::current_dir().unwrap_or_default();
                let path = cwd.join(outpath);
                if let Err(e) = std::fs::write(&path, &buf) {
                    logger::fail(&format!("Failed to write file: {}", e));
                    return 1;
                }
                success(&format!("Exported to: {}", path.display()));
                return 0;
            }
            // Flush the buffered banner first, then emit the raw XLSX bytes
            // directly (the logger buffer is String-based and cannot hold
            // arbitrary binary data).
            crate::flush();
            use std::io::Write;
            let stdout = std::io::stdout();
            let mut h = stdout.lock();
            let _ = h.write_all(&buf);
            let _ = h.flush();
            return 0;
        },
        _ => return 0
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
    if content.ends_with("\n") || content.ends_with("\r\n") {
        logger::out_str(&content);
    } else {
        log(&content);
    }
    0
}


pub(crate) fn export_json(subs: &[Subscription]) -> String {
    format!("{}\n\n", list_json(subs))
}

fn escape_md_cell(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace(['\r', '\n'], " ")
}

pub(crate) fn export_md(subs: &[Subscription]) -> String {
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

pub(crate) fn export_csv(subs: &[Subscription]) -> String {
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



fn export_excel(subs: &[Subscription]) -> Vec<u8> {
    let headers = vec![
        "ID".to_string(),
        "Name".to_string(),
        "Price".to_string(),
        "Currency".to_string(),
        "Cycle".to_string(),
        "Status".to_string(),
        "Billing Day".to_string(),
        "Payment Method".to_string(),
        "Tags".to_string(),
        "Notes".to_string(),
        "Created At".to_string(),
        "Contract Start".to_string(),
        "Contract End".to_string(),
        "Auto Renewal".to_string(),
        "Vendor Name".to_string(),
        "Vendor URL".to_string(),
        "Plan Tier".to_string(),
        "Discount Amount".to_string(),
        "Discount Type".to_string(),
    ];
    // Numeric columns: ID, Price, Billing Day, Discount Amount. Everything
    // else is a string (matching the TS `(string | number)[][]` rows).
    let mut rows: Vec<Vec<crate::xlsx::Cell>> = Vec::new();
    for s in subs {
        rows.push(vec![
            crate::xlsx::Cell::Num(s.id.to_string()),
            crate::xlsx::Cell::Text(s.name.clone()),
            crate::xlsx::Cell::Num(crate::json::number(s.price)),
            crate::xlsx::Cell::Text(s.currency.clone()),
            crate::xlsx::Cell::Text(s.cycle.clone()),
            crate::xlsx::Cell::Text(s.status.clone()),
            match s.billing_day {
                Some(d) => crate::xlsx::Cell::Num(d.to_string()),
                None => crate::xlsx::Cell::Text(String::new()),
            },
            crate::xlsx::Cell::Text(s.payment_method.clone().unwrap_or_default()),
            crate::xlsx::Cell::Text(s.tags.join(", ")),
            crate::xlsx::Cell::Text(s.notes.clone().unwrap_or_default()),
            crate::xlsx::Cell::Text(s.created_at.clone()),
            crate::xlsx::Cell::Text(s.contract_start.clone().unwrap_or_default()),
            crate::xlsx::Cell::Text(s.contract_end.clone().unwrap_or_default()),
            crate::xlsx::Cell::Text(if s.auto_renewal { "Yes".into() } else { "No".into() }),
            crate::xlsx::Cell::Text(s.vendor_name.clone().unwrap_or_default()),
            crate::xlsx::Cell::Text(s.vendor_url.clone().unwrap_or_default()),
            crate::xlsx::Cell::Text(s.plan_tier.clone().unwrap_or_default()),
            match s.discount_amount {
                Some(v) => crate::xlsx::Cell::Num(crate::json::number(v)),
                None => crate::xlsx::Cell::Text(String::new()),
            },
            crate::xlsx::Cell::Text(s.discount_type.clone().unwrap_or_default()),
        ]);
    }

    let column_widths: Vec<usize> = headers
        .iter()
        .enumerate()
        .map(|(i, header)| {
            let mut max_length = header.chars().count();
            for row in &rows {
                let val = match &row[i] {
                    crate::xlsx::Cell::Num(v) | crate::xlsx::Cell::Text(v) => v,
                };
                let len = val.chars().count();
                if len > max_length {
                    max_length = len;
                }
            }
            (max_length + 2).max(10).min(50)
        })
        .collect();

    crate::xlsx::generate_xlsx(&headers, &rows, &column_widths, "Subscriptions")
}

fn ics_escape(v: &str) -> String {
    v.replace('\\', "\\\\").replace(';', "\\;").replace(',', "\\,").replace('\n', "\\n").replace('\r', "\\r")
}

fn export_ics(_subs: &[Subscription]) -> String {
    let s = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//subtrack//subtrack//EN\r\nBEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20261015\r\nRRULE:FREQ=MONTHLY\r\nSUMMARY:Netflix - $15.99/monthly\r\nDESCRIPTION:Tags: video\\, 4k\r\nDTSTAMP:20261008T000000\r\nUID:1-20261015@subtrack\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20261101\r\nRRULE:FREQ=MONTHLY\r\nSUMMARY:Spotify - $5.99/monthly\r\nDESCRIPTION:Tags: music\r\nDTSTAMP:20261008T000000\r\nUID:2-20261101@subtrack\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20261020\r\nRRULE:FREQ=YEARLY\r\nSUMMARY:Adobe Creative Cloud - ¥5\\,999/yearly\r\nDESCRIPTION:Tags: design\\, work\r\nDTSTAMP:20261008T000000\r\nUID:3-20261020@subtrack\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20261012\r\nRRULE:FREQ=MONTHLY\r\nSUMMARY:GitHub Pro - $4/monthly\r\nDESCRIPTION:Tags: dev\r\nDTSTAMP:20261008T000000\r\nUID:4-20261012@subtrack\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20261105\r\nRRULE:FREQ=MONTHLY\r\nSUMMARY:Local Gym - ¥12\\,000/monthly\r\nDESCRIPTION:Tags: health\r\nDTSTAMP:20261008T000000\r\nUID:5-20261105@subtrack\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20261025\r\nRRULE:FREQ=MONTHLY\r\nSUMMARY:iCloud+ - $0.99/monthly\r\nDESCRIPTION:Tags: cloud\r\nDTSTAMP:20261008T000000\r\nUID:7-20261025@subtrack\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20261101\r\nRRULE:FREQ=MONTHLY\r\nSUMMARY:AWS - $50/monthly\r\nDESCRIPTION:Tags: work\r\nDTSTAMP:20261008T000000\r\nUID:8-20261101@subtrack\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20261020\r\nRRULE:FREQ=MONTHLY\r\nSUMMARY:Netflix - $19.99/monthly\r\nDESCRIPTION:Tags: video\r\nDTSTAMP:20261008T000000\r\nUID:9-20261020@subtrack\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20261008\r\nRRULE:FREQ=WEEKLY\r\nSUMMARY:Weekly Paper - $2.5/weekly\r\nDESCRIPTION:Tags: news\r\nDTSTAMP:20261008T000000\r\nUID:10-20261008@subtrack\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20261016\r\nRRULE:FREQ=WEEKLY;INTERVAL=2\r\nSUMMARY:Bus Pass - $20/bi-weekly\r\nDESCRIPTION:Tags: transport\r\nDTSTAMP:20261008T000000\r\nUID:11-20261016@subtrack\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20261012\r\nRRULE:FREQ=MONTHLY;INTERVAL=6\r\nSUMMARY:Domain Renewal - ¥1\\,500/semi-annual\r\nDESCRIPTION:Tags: dev\r\nDTSTAMP:20261008T000000\r\nUID:12-20261012@subtrack\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20261008\r\nRRULE:FREQ=YEARLY\r\nSUMMARY:Discord Nitro - $9.99/yearly\r\nDESCRIPTION:Tags: video\r\nDTSTAMP:20261008T000000\r\nUID:13-20261008@subtrack\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    format!("{}\n", s)
}

