//! `maintenance` command — port of `maintenance.ts` (handleMaintenance).

use crate::db;
use crate::logger::{self, fail, info};
use crate::parser::ParsedArgs;

/// Format bytes to a human-readable string (B, kB, MB, GB).
fn format_bytes(bytes: i64) -> String {
    let units = ["B", "kB", "MB", "GB"];
    let mut i = 0usize;
    let mut size = bytes as f64;
    while size >= 1024.0 && i < units.len() - 1 {
        size /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{} {}", size.round() as i64, units[i])
    } else {
        format!("{:.1} {}", size, units[i])
    }
}

pub fn run_maintenance(parsed: &ParsedArgs) -> i32 {
    let json = parsed.bool("json") == Some(true);
    let do_check = parsed.bool("check").unwrap_or(true);
    let do_vacuum = parsed.bool("vacuum") == Some(true);

    let conn = match db::open_db() {
        Ok(c) => c,
        Err(e) => {
            fail(&e.to_string());
            return 1
        }
    };

    let mut results: Vec<String> = Vec::new();

    if do_check {
        let check_result = db::integrity_check(&conn);
        if json {
            results.push(format!(
                "  \"integrityCheck\": {}",
                crate::json::quote_string(if check_result == "ok" { "passed" } else { &check_result })
            ));
        } else if check_result == "ok" {
            logger::success("Integrity check: passed");
        } else {
            fail(&format!("Integrity check: FAILED — {}", check_result));
            logger::warn(
                "Database integrity issues detected.\n  Run 'subtrack backup' immediately, then restore from a known-good backup.",
            );
        }
    }

    if do_vacuum {
        let before_size = db::db_file_size();
        if let Err(e) = db::vacuum_db(&conn) {
            if json {
                results.push(format!(
                    "  \"vacuum\": {{\n    \"error\": {}\n  }}",
                    crate::json::quote_string(&e.to_string())
                ));
            } else {
                fail(&format!("VACUUM failed: {}", e));
            }
            if json {
                logger::log(&format!("{{\n{}\n}}", results.join(",\n")));
            }
            return 0
        }
        let after_size = db::db_file_size();
        let saved = before_size - after_size;

        if json {
            results.push(format!(
                "  \"vacuum\": {{\n    \"beforeBytes\": {},\n    \"afterBytes\": {},\n    \"savedBytes\": {}\n  }}",
                before_size, after_size, saved
            ));
        } else if saved > 0 {
            logger::success(&format!(
                "VACUUM complete: {} → {} (freed {})",
                format_bytes(before_size),
                format_bytes(after_size),
                format_bytes(saved)
            ));
        } else {
            info("VACUUM complete: no space reclaimed (database already optimized)");
        }
    }

    if json {
        logger::log(&format!("{{\n{}\n}}", results.join(",\n")));
    }
    0
}
