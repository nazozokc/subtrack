//! `import` (CSV) and `restore` commands — port of `import-csv.ts`
//! (`handleImport`) and `backup.ts` (`handleRestore`).

use crate::db::NewSubscription;
use crate::logger;
use crate::parser::ParsedArgs;
use crate::safe_path::safe_path;
use crate::validation::{
    is_valid_currency, is_valid_cycle, is_valid_status, validate_auto_renewal,
    validate_billing_day, validate_date_string, validate_discount_type, validate_discount_value,
    validate_name, validate_price, validate_tags,
};

const MAX_CSV_SIZE: u64 = 10 * 1024 * 1024;
const MAX_CSV_ROWS: usize = 10_000;

/// Port of `parseCsvLine`.
fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut it = line.chars().peekable();
    while let Some(ch) = it.next() {
        if in_quotes {
            if ch == '"' {
                if it.peek() == Some(&'"') {
                    current.push('"');
                    it.next();
                } else {
                    in_quotes = false;
                }
            } else {
                current.push(ch);
            }
        } else if ch == ',' {
            fields.push(std::mem::take(&mut current));
        } else if ch == '"' {
            in_quotes = true;
        } else {
            current.push(ch);
        }
    }
    fields.push(current);
    fields
}

/// Port of `handleImport`.
pub fn run_import(parsed: &ParsedArgs) -> i32 {
    let file = parsed.str("file").unwrap_or_default();
    if file.is_empty() {
        logger::fail("Usage: subtrack import <file> [--dry-run]");
        return 1;
    }
    let dry_run = parsed.bool("dryRun") == Some(true);
    let deduplicate = parsed.bool("deduplicate") == Some(true);

    let Some(safe_file) = safe_path(&file) else {
        logger::fail("File not found or path not allowed — must be within home or temp directory");
        return 1;
    };

    let content = match std::fs::metadata(&safe_file) {
        Ok(st) if st.len() > MAX_CSV_SIZE => {
            logger::fail(&format!(
                "File too large ({:.1} MB). Maximum: {} MB",
                st.len() as f64 / 1024.0 / 1024.0,
                MAX_CSV_SIZE / 1024 / 1024
            ));
            return 1;
        }
        Ok(_) => match std::fs::read(&safe_file) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(e) => {
                logger::fail(&format!("Failed to read file: {}", e));
                return 1;
            }
        },
        Err(e) => {
            logger::fail(&format!("Failed to read file: {}", e));
            return 1;
        }
    };

    // Strip BOM and normalize line endings.
    let normalized = content.replace("\r\n", "\n").replace('\r', "\n");
    let clean = normalized.strip_prefix('\u{feff}').unwrap_or(&normalized);
    let lines: Vec<String> = clean
        .split('\n')
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    if lines.len() < 2 {
        logger::fail("CSV file must have a header row and at least one data row");
        return 1;
    }

    let data_lines = &lines[1..];
    if data_lines.len() > MAX_CSV_ROWS {
        logger::fail(&format!(
            "CSV file has {} data rows (max {}). Split the file into smaller batches.",
            data_lines.len(),
            MAX_CSV_ROWS
        ));
        return 1;
    }

    let header: Vec<String> = parse_csv_line(&lines[0])
        .into_iter()
        .map(|h| h.to_lowercase().trim().to_string())
        .collect();
    let mut col_index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (i, h) in header.iter().enumerate() {
        if !h.is_empty() {
            col_index.insert(h.clone(), i);
        }
    }

    let required = ["name", "cycle", "tags", "price", "currency"];
    let missing: Vec<&str> = required
        .iter()
        .copied()
        .filter(|c| !col_index.contains_key(*c))
        .collect();
    if !missing.is_empty() {
        logger::fail(&format!(
            "Invalid CSV header. Required columns: {} (missing: {})",
            required.join(", "),
            missing.join(", ")
        ));
        return 1;
    }

    let conn = match crate::db::open_db() {
        Ok(c) => c,
        Err(e) => {
            logger::fail(&format!("Failed to read file: {}", e));
            return 1;
        }
    };

    let mut success: i64 = 0;
    let mut failed: i64 = 0;
    let mut skipped: i64 = 0;

    for (i, line) in data_lines.iter().enumerate() {
        let line_no = i + 2;
        let fields_of_row = parse_csv_line(line);
        let col = |name: &str| -> Option<String> {
            col_index.get(name).and_then(|idx| {
                fields_of_row
                    .get(*idx)
                    .map(|v| v.trim().to_string())
                    .filter(|v| !v.is_empty())
            })
        };

        let name = col("name").unwrap_or_default();
        let cycle = col("cycle").unwrap_or_default();
        let tags_str = col("tags").unwrap_or_default();
        let price_str = col("price").unwrap_or_default();
        let currency = col("currency").unwrap_or_default();
        let notes = col("notes");

        let status = col("status").unwrap_or_else(|| "active".to_string());
        let payment_method = col("payment_method");
        let contract_start = col("contract_start");
        let contract_end = col("contract_end");
        let auto_renewal = col("auto_renewal");
        let vendor_name = col("vendor_name");
        let vendor_url = col("vendor_url");
        let plan_tier = col("plan_tier");
        let discount_amount = col("discount_amount");
        let discount_type = col("discount_type");
        let billing_day = col("billing_day");

        // Sanitize: strip control characters from name.
        let sanitized: String = name
            .chars()
            .filter(|c| {
                let u = *c as u32;
                !((0x00..=0x08).contains(&u)
                    || u == 0x0b
                    || u == 0x0c
                    || (0x0e..=0x1f).contains(&u)
                    || u == 0x7f)
            })
            .collect();

        if let Err(e) = validate_name(&sanitized) {
            logger::warn(&format!("Line {}: {}", line_no, e));
            failed += 1;
            continue;
        }
        if let Err(e) = validate_price(&price_str) {
            logger::warn(&format!("Line {}: {}", line_no, e));
            failed += 1;
            continue;
        }
        if !is_valid_currency(&currency) {
            logger::warn(&format!(
                "Line {}: invalid currency \"{}\"",
                line_no, currency
            ));
            failed += 1;
            continue;
        }
        if !is_valid_cycle(&cycle) {
            logger::warn(&format!("Line {}: invalid cycle \"{}\"", line_no, cycle));
            failed += 1;
            continue;
        }
        if !is_valid_status(&status) {
            logger::warn(&format!("Line {}: invalid status \"{}\"", line_no, status));
            failed += 1;
            continue;
        }
        if let Some(v) = &discount_amount
            && let Err(e) = validate_discount_value(v)
        {
            logger::warn(&format!("Line {}: {}", line_no, e));
            failed += 1;
            continue;
        }
        if let Some(v) = &discount_type
            && let Err(e) = validate_discount_type(v)
        {
            logger::warn(&format!("Line {}: {}", line_no, e));
            failed += 1;
            continue;
        }
        if let Some(v) = &auto_renewal
            && let Err(e) = validate_auto_renewal(v)
        {
            logger::warn(&format!("Line {}: {}", line_no, e));
            failed += 1;
            continue;
        }
        if let Some(v) = &contract_start
            && let Err(e) = validate_date_string(v)
        {
            logger::warn(&format!("Line {}: {}", line_no, e));
            failed += 1;
            continue;
        }
        if let Some(v) = &contract_end
            && let Err(e) = validate_date_string(v)
        {
            logger::warn(&format!("Line {}: {}", line_no, e));
            failed += 1;
            continue;
        }
        if let Some(bd) = &billing_day
            && let Err(e) = validate_billing_day(bd)
        {
            logger::warn(&format!("Line {}: {}", line_no, e));
            failed += 1;
            continue;
        }

        let tags: Vec<String> = tags_str
            .split(';')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect();
        if let Err(e) = validate_tags(&tags.join(",")) {
            logger::warn(&format!("Line {}: {}", line_no, e));
            failed += 1;
            continue;
        }

        if deduplicate
            && let Ok(Some(existing)) = crate::db::find_subscription_by_name(sanitized.trim())
        {
            logger::warn(&format!(
                "Line {}: \"{}\" already exists (id={}) — skipping",
                line_no,
                sanitized.trim(),
                existing
            ));
            skipped += 1;
            continue;
        }

        if dry_run {
            logger::info(&format!(
                "[dry-run] Would import: {} ({} {}, {})",
                sanitized, price_str, currency, cycle
            ));
            success += 1;
            continue;
        }

        let new_sub = NewSubscription {
            name: sanitized.trim().to_string(),
            price: price_str.parse::<f64>().unwrap_or(0.0),
            currency,
            cycle,
            status,
            billing_day: billing_day.and_then(|v| v.parse::<i64>().ok()),
            notes,
            payment_method,
            contract_start,
            contract_end,
            auto_renewal: auto_renewal.map(|v| v == "true").unwrap_or(true),
            vendor_name,
            vendor_url,
            plan_tier,
            discount_amount: discount_amount.and_then(|v| v.parse::<f64>().ok()),
            discount_type,
            tags,
        };
        match crate::db::write_subscription(&conn, &new_sub, false) {
            Ok(_) => success += 1,
            Err(e) => {
                logger::warn(&format!("Line {}: failed to import: {}", line_no, e));
                failed += 1;
            }
        }
    }

    let skip_note = if skipped > 0 {
        format!(", {} skipped (already exists)", skipped)
    } else {
        String::new()
    };

    if dry_run {
        logger::success(&format!(
            "Dry-run complete: {} valid, {} invalid{}",
            success, failed, skip_note
        ));
    } else {
        if success > 0 || failed > 0 {
            let _ = crate::db::save_db(&conn);
        }
        let details = format!(
            "{} imported, {} failed, {} skipped from {}",
            success, failed, skipped, file
        );
        let _ = crate::db::add_audit_log(
            &conn,
            "subscription.import",
            None,
            None,
            Some(&details),
            true,
        );
        logger::success(&format!(
            "Import complete: {} imported, {} failed{}",
            success, failed, skip_note
        ));
    }
    0
}
