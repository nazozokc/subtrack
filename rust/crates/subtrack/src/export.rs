//! `export` command — port of `export.ts` (handleExport) for the stdout
//! formats: json, csv, md, excel and ics. The document builders themselves
//! live in `subtrack_core::formats`.

use crate::db::{QueryOptions, get_subscriptions};
use crate::logger::{self, log, success};
use crate::parser::ParsedArgs;
use subtrack_core::formats::{export_csv, export_excel, export_ics, export_json, export_md};

const SUPPORTED: [&str; 5] = ["csv", "json", "md", "excel", "ics"];

fn safe_output_path_or_fail(outpath: &str) -> Option<std::path::PathBuf> {
    match crate::safe_path::safe_output_path(outpath) {
        Some(path) => Some(path),
        None => {
            logger::fail("Invalid output path — must be within home or temp directory");
            None
        }
    }
}

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
        let statuses: Vec<String> = status.split(',').map(|s| s.trim().to_lowercase()).collect();
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
                let Some(path) = safe_output_path_or_fail(&outpath) else {
                    return 1;
                };
                if let Err(e) = crate::crypto::write_secret_file(&path, &buf) {
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
        }
        _ => return 0,
    };

    // `consola.log(content)` — content already ends in "\n" for json.
    if let Some(outpath) = parsed.str("output").or_else(|| parsed.str("o")) {
        let Some(path) = safe_output_path_or_fail(&outpath) else {
            return 1;
        };
        if let Err(e) = crate::crypto::write_secret_file(&path, content.as_bytes()) {
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
