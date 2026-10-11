//! `receipt` command (reads scope) — port of `receipt.ts` (handleReceipt).
//! The suggestion-parser pipeline is deferred; for this phase the fixture
//! only exercises the empty-candidates / error paths.

use crate::logger::{self, fail, info};
use crate::parser::ParsedArgs;

const MAX_RECEIPT_SIZE: u64 = 5 * 1024 * 1024;

fn read_receipts(file: &str) -> Result<Vec<serde_json::Value>, String> {
    let meta = std::fs::metadata(file).map_err(|_| format!("Cannot read file: {}", file))?;
    if meta.len() > MAX_RECEIPT_SIZE {
        return Err(format!(
            "File too large ({:.1} MB). Maximum: 5 MB",
            meta.len() as f64 / 1024.0 / 1024.0
        ));
    }
    let text = std::fs::read_to_string(file)
        .map_err(|_| format!("Cannot read file: {}", file))?;
    let lower = file.to_lowercase();
    if lower.ends_with(".json") {
        let parsed: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("Invalid JSON: {}", e))?;
        let rows: Vec<&serde_json::Value> = match &parsed {
            serde_json::Value::Array(a) => a.iter().collect(),
            _ => vec![&parsed],
        };
        if rows.len() > 10_000 {
            return Err(format!("JSON file has {} entries (max 10000)", rows.len()));
        }
        let mut out = Vec::new();
        for (index, row) in rows.iter().enumerate() {
            let body = row
                .get("body")
                .or_else(|| row.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let subject = row.get("subject").and_then(|v| v.as_str());
            let from = row.get("from").and_then(|v| v.as_str());
            let date = row.get("date").and_then(|v| v.as_str());
            out.push(serde_json::json!({
                "id": index.to_string(),
                "textBody": body,
                "subject": subject,
                "from": from,
                "date": date,
            }));
        }
        return Ok(out);
    }
    if lower.ends_with(".csv") {
        let lines: Vec<&str> = text
            .split(|c| c == '\r' || c == '\n')
            .filter(|l| !l.is_empty())
            .skip(1)
            .collect();
        if lines.len() > 10_000 {
            return Err(format!("CSV file has {} data rows (max 10000)", lines.len()));
        }
        let mut out = Vec::new();
        for line in lines {
            let cols: Vec<String> = line.split(',').map(|s| s.trim().to_string()).collect();
            let body = cols.first().cloned().unwrap_or_default();
            let subject = cols.get(1).cloned().filter(|s| !s.is_empty());
            let from = cols.get(2).cloned().filter(|s| !s.is_empty());
            let date = cols.get(3).cloned().filter(|s| !s.is_empty());
            out.push(serde_json::json!({
                "id": line,
                "textBody": body,
                "subject": subject,
                "from": from,
                "date": date,
            }));
        }
        return Ok(out);
    }
    Ok(vec![serde_json::json!({
        "id": file,
        "textBody": text,
        "subject": serde_json::Value::Null,
        "from": serde_json::Value::Null,
        "date": serde_json::Value::Null,
    })])
}

/// Candidates extracted via `parseEmail` — deferred; always empty for now.
fn parse_email_candidates(rows: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
    let _ = rows;
    Vec::new()
}

pub fn run_receipt(parsed: &ParsedArgs) -> i32 {
    let file = match parsed.str("file") {
        Some(f) => f,
        None => {
            fail("Receipt file path is required");
            return 1;
        }
    };
    let json = parsed.bool("json") == Some(true);
    let dry_run = parsed.bool("dryRun") == Some(true);

    let candidates = match read_receipts(&file) {
        Ok(rows) => parse_email_candidates(rows),
        Err(msg) => {
            fail(&format!("Failed to read receipt: {}", msg));
            return 1;
        }
    };

    if json || dry_run {
        let out = serde_json::to_string_pretty(&candidates).unwrap_or_else(|_| "[]".to_string());
        logger::out_str(&format!("{}\n", out));
        return 0;
    }
    if candidates.is_empty() {
        info("No receipt candidates found");
        return 0;
    }
    // Writes + review are a mutate-phase concern.
    0
}