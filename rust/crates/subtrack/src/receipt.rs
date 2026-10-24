//! `receipt` command — port of `receipt.ts` (handleReceipt).
//!
//! The suggestion-parser pipeline (bank/credit card/receipt/wallet/generic)
//! lives in `suggest_parser`; this module handles file reading, JSON preview
//! output, and the persistence path.

use crate::logger::{self, fail, info};
use crate::parser::ParsedArgs;
use crate::suggest_parser::{Candidate, RawEmail, parse_email};

const MAX_RECEIPT_SIZE: u64 = 5 * 1024 * 1024;

fn read_receipts(file: &str) -> Result<Vec<RawEmail>, String> {
    let meta = std::fs::metadata(file).map_err(|_| format!("Cannot read file: {}", file))?;
    if meta.len() > MAX_RECEIPT_SIZE {
        return Err(format!(
            "File too large ({:.1} MB). Maximum: 5 MB",
            meta.len() as f64 / 1024.0 / 1024.0
        ));
    }
    let text = std::fs::read_to_string(file).map_err(|_| format!("Cannot read file: {}", file))?;
    let lower = file.to_lowercase();
    if lower.ends_with(".json") {
        let parsed: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("Invalid JSON: {}", e))?;
        let rows: Vec<&serde_json::Value> = match &parsed {
            serde_json::Value::Array(a) => a.iter().collect(),
            _ => vec![&parsed],
        };
        if rows.len() > 10_000 {
            return Err(format!("JSON file has {} entries (max 10000)", rows.len()));
        }
        let mut out = Vec::new();
        for row in rows {
            let body = row
                .get("body")
                .or_else(|| row.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let subject = row
                .get("subject")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let from = row
                .get("from")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            out.push(RawEmail {
                subject,
                from,
                body,
            });
        }
        return Ok(out);
    }
    if lower.ends_with(".csv") {
        let lines: Vec<&str> = text
            .split(['\r', '\n'])
            .filter(|l| !l.is_empty())
            .skip(1)
            .collect();
        if lines.len() > 10_000 {
            return Err(format!(
                "CSV file has {} data rows (max 10000)",
                lines.len()
            ));
        }
        let mut out = Vec::new();
        for line in lines {
            let cols: Vec<String> = line.split(',').map(|s| s.trim().to_string()).collect();
            let body = cols.first().cloned().unwrap_or_default();
            let subject = cols.get(1).cloned().filter(|s| !s.is_empty());
            let from = cols.get(2).cloned().filter(|s| !s.is_empty());
            out.push(RawEmail {
                subject,
                from,
                body,
            });
        }
        return Ok(out);
    }
    Ok(vec![RawEmail {
        subject: None,
        from: None,
        body: text,
    }])
}

/// Port of the `map(parseEmail).filter(Boolean)` step.
fn parse_email_candidates(rows: Vec<RawEmail>) -> Vec<Candidate> {
    rows.iter().filter_map(parse_email).collect()
}

/// `JSON.stringify(candidates, null, 2)`.
fn candidates_json(items: &[Candidate]) -> String {
    if items.is_empty() {
        return "[]".to_string();
    }
    let objs: Vec<String> = items.iter().map(|c| c.to_json("  ")).collect();
    format!("[\n{}\n]", objs.join(",\n"))
}

pub fn run_receipt(parsed: &ParsedArgs) -> i32 {
    let file = match parsed.str("file") {
        Some(f) => f,
        None => {
            fail("Receipt file path is required");
            return 1;
        }
    };
    let json = parsed.flag("json");
    let dry_run = parsed.flag("dryRun");

    let candidates = match read_receipts(&file) {
        Ok(rows) => parse_email_candidates(rows),
        Err(msg) => {
            fail(&format!("Failed to read receipt: {}", msg));
            return 1;
        }
    };

    // json and dryRun are read-only previews: no writes, no console noise.
    if json || dry_run {
        logger::out_str(&format!("{}\n", candidates_json(&candidates)));
        return 0;
    }
    if candidates.is_empty() {
        info("No receipt candidates found");
        return 0;
    }

    // Persist the candidates (port of recordBatch + logAudit + success).
    let items: Vec<crate::db::NewSuggestion> = candidates
        .iter()
        .map(|c| crate::db::NewSuggestion {
            name: c.name.clone(),
            price: Some(c.price),
            currency: Some(c.currency.clone()),
            cycle: c.cycle.clone(),
            source: c.source.clone(),
            source_detail: Some(c.source_detail.clone()),
            confidence: c.confidence,
        })
        .collect();

    let conn = match crate::db::open_db() {
        Ok(c) => c,
        Err(e) => {
            fail(&format!("Failed to open database: {}", e));
            return 1;
        }
    };
    if let Err(e) = crate::db::write_suggestion_batch(&conn, &items, false) {
        fail(&format!("Failed to import receipt candidates: {}", e));
        return 1;
    }
    let details = format!("{} receipt candidate(s) imported", candidates.len());
    let _ = crate::db::add_audit_log(
        &conn,
        "suggestion.receipt",
        None,
        None,
        Some(&details),
        false,
    );
    if let Err(e) = crate::db::save_db(&conn) {
        fail(&format!("Failed to save receipt candidates: {}", e));
        return 1;
    }
    logger::success(&format!(
        "Imported {} receipt candidate(s)",
        candidates.len()
    ));
    0
}
