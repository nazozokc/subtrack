//! Document builders — port of `export.ts` (the pure format writers):
//! JSON, Markdown, CSV, XLSX bytes and ICS. No DB access, no argument
//! parsing; callers fetch subscriptions and hand them over.

use crate::date::{calculate_next_billing_inclusive, today};
use crate::json::{list_json, number};
use crate::models::Subscription;
use crate::price::format_price;

pub fn export_json(subs: &[Subscription]) -> String {
    format!("{}\n\n", list_json(subs))
}

fn escape_md_cell(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace(['\r', '\n'], " ")
}

pub fn export_md(subs: &[Subscription]) -> String {
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
    if let Some(first) = trimmed.chars().next()
        && matches!(first, '=' | '+' | '-' | '@' | '\t' | '\r')
    {
        cleaned = format!("\t{}", cleaned);
    }
    if cleaned.contains('"')
        || cleaned.contains(',')
        || cleaned.contains('\n')
        || cleaned.contains('\r')
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

pub fn export_csv(subs: &[Subscription]) -> String {
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

pub fn export_excel(subs: &[Subscription]) -> Vec<u8> {
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
            crate::xlsx::Cell::Text(if s.auto_renewal {
                "Yes".into()
            } else {
                "No".into()
            }),
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
            (max_length + 2).clamp(10, 50)
        })
        .collect();

    crate::xlsx::generate_xlsx(&headers, &rows, &column_widths, "Subscriptions")
}

fn ics_escape(v: &str) -> String {
    v.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

/// Port of `buildIcs` (export.ts) — one VEVENT per non-cancelled
/// subscription, DTSTART set to its next billing date.
pub fn export_ics(subs: &[Subscription]) -> String {
    let now = today();
    let mut body =
        String::from("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//subtrack//subtrack//EN\r\n");
    for sub in subs
        .iter()
        .filter(|s| crate::domain::not_cancelled(&s.status))
    {
        let start =
            calculate_next_billing_inclusive(sub.billing_day, &sub.created_at, &sub.cycle, now);
        let start_str = start.format("%Y%m%d").to_string();
        let rrule = match sub.cycle.as_str() {
            "weekly" => "FREQ=WEEKLY",
            "bi-weekly" => "FREQ=WEEKLY;INTERVAL=2",
            "monthly" => "FREQ=MONTHLY",
            "quarterly" => "FREQ=MONTHLY;INTERVAL=3",
            "semi-annual" => "FREQ=MONTHLY;INTERVAL=6",
            _ => "FREQ=YEARLY",
        };
        let summary = format!(
            "{} - {}/{}",
            sub.name,
            format_price(sub.price, &sub.currency),
            sub.cycle
        );
        let description = if sub.tags.is_empty() {
            String::new()
        } else {
            format!("Tags: {}", sub.tags.join(", "))
        };
        body.push_str("BEGIN:VEVENT\r\n");
        body.push_str(&format!("DTSTART;VALUE=DATE:{}\r\n", start_str));
        body.push_str(&format!("RRULE:{}\r\n", rrule));
        body.push_str(&format!("SUMMARY:{}\r\n", ics_escape(&summary)));
        body.push_str(&format!("DESCRIPTION:{}\r\n", ics_escape(&description)));
        body.push_str(&format!("DTSTAMP:{}T000000\r\n", now.format("%Y%m%d")));
        body.push_str(&format!("UID:{}-{}@subtrack\r\n", sub.id, start_str));
        body.push_str("END:VEVENT\r\n");
    }
    body.push_str("END:VCALENDAR\r\n");
    format!("{}\n", body)
}
