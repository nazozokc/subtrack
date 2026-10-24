//! `cleanup` command — port of `cleanup.ts` (handleCleanup).

use crate::db;
use crate::logger::{self, fail, info};
use crate::parser::ParsedArgs;

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

pub fn run_cleanup(parsed: &ParsedArgs) -> i32 {
    let json = parsed.flag("json");
    let do_vacuum = parsed.bool("vacuum").unwrap_or(true);
    let audit_days = parsed
        .str("audit-days")
        .and_then(|d| d.parse::<i64>().ok())
        .unwrap_or(90);

    let conn = match db::open_db() {
        Ok(c) => c,
        Err(e) => {
            fail(&e.to_string());
            return 1;
        }
    };

    let mut results: Vec<String> = Vec::new();

    // ── Integrity check ──
    let check_result = db::integrity_check(&conn);
    let integrity_ok = check_result == "ok";
    if json {
        results.push(format!(
            "  \"integrityCheck\": {}",
            crate::json::quote_string(if integrity_ok {
                "passed"
            } else {
                &check_result
            })
        ));
    } else if integrity_ok {
        logger::success("Integrity check: passed");
    } else {
        fail(&format!("Integrity check: FAILED — {}", check_result));
    }

    // ── VACUUM ──
    if do_vacuum && integrity_ok {
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
            return 0;
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
                "VACUUM: {} → {} (freed {})",
                format_bytes(before_size),
                format_bytes(after_size),
                format_bytes(saved)
            ));
        } else {
            info("VACUUM: no space reclaimed (already optimized)");
        }
    }

    // ── Prune audit logs ────────────────────────────────
    let before = (chrono::Utc::now() - chrono::Duration::days(audit_days))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    let pruned_audit = db::prune_audit_logs(&conn, &before, false).unwrap_or(0);
    if json {
        results.push(format!("  \"auditPruned\": {}", pruned_audit));
    } else if pruned_audit > 0 {
        logger::success(&format!(
            "Audit log: pruned {} entr{}",
            pruned_audit,
            if pruned_audit != 1 { "ies" } else { "y" }
        ));
    } else {
        info("Audit log: nothing to prune");
    }

    // ── Prune orphaned tags ─────────────────────────────
    let pruned_tags = db::prune_tags(&conn, false).unwrap_or(0);
    if json {
        results.push(format!("  \"tagsPruned\": {}", pruned_tags));
    } else if pruned_tags > 0 {
        logger::success(&format!("Tags: pruned {}", pruned_tags));
    } else {
        info("Tags: no orphans found");
    }

    if pruned_audit > 0 || pruned_tags > 0 {
        let _ = db::save_db(&conn);
    }

    if json {
        logger::log(&format!("{{\n{}\n}}", results.join(",\n")));
    }
    0
}
