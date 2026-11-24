//! `maintenance` command — port of `maintenance.ts` (handleMaintenance).

use crate::db;
use crate::logger::{self, fail, info};
use crate::parser::ParsedArgs;

struct RekeyOutcome {
    from: String,
    to: String,
    database: bool,
    config: bool,
}

fn rekey_json(from: &str, to: &str, database: bool, config: bool) -> String {
    format!(
        "{{\n  \"from\": {},\n  \"to\": {},\n  \"databaseRekeyed\": {},\n  \"configRekeyed\": {}\n}}",
        crate::json::quote_string(from),
        crate::json::quote_string(to),
        database,
        config
    )
}

fn perform_rekey(quiet: bool) -> Result<Option<RekeyOutcome>, String> {
    if !crate::crypto::passphrase_encryption() {
        return Err(
            "KDF rekey is only needed for passphrase encryption (SUBSC_CLI_DB_PASSPHRASE)."
                .to_string(),
        );
    }
    let from = crate::crypto::kdf_version().map_err(|e| e.to_string())?;
    if from == crate::crypto::KdfVersion::LATEST {
        return Ok(None);
    }
    if !crate::backup::create_safety_backup("rekey-kdf", !quiet) {
        return Err("Rekey aborted because the safety backup failed.".to_string());
    }
    // Marker first: a crash here leaves data that the decrypt fallback can
    // still read with the old work factor.
    crate::crypto::set_kdf_version_latest().map_err(|e| e.to_string())?;
    let database = db::rekey_db_to_latest().map_err(|e| e.to_string())?;
    let config = crate::config::rekey_config_to_latest().map_err(|e| e.to_string())?;
    crate::config::reload_config();
    Ok(Some(RekeyOutcome {
        from: crate::crypto::kdf_version_name(from).to_string(),
        to: crate::crypto::kdf_version_name(crate::crypto::KdfVersion::LATEST).to_string(),
        database,
        config,
    }))
}

/// Prompt once on the first interactive `add` for passphrase installations that
/// still use an older work factor. Non-interactive runs are untouched.
pub fn maybe_prompt_rekey() {
    if !matches!(crate::crypto::passphrase_needs_rekey(), Ok(true)) {
        return;
    }
    if !crate::prompt::interactive() {
        return;
    }
    logger::warn(
        "This installation still uses an older KDF work factor; commands spend more time deriving the key than necessary.",
    );
    if crate::prompt::confirm("Switch to the faster KDF now (recommended)?", true) != Some(true) {
        return;
    }
    match perform_rekey(false) {
        Ok(Some(_)) => logger::success("KDF rekey complete. Future commands will be faster."),
        Ok(None) => {}
        Err(e) => logger::fail(&e),
    }
}

/// `maintenance rekey-kdf` — re-encrypt the database and config with the
/// latest KDF after writing a safety backup.
pub fn run_rekey_kdf(parsed: &ParsedArgs) -> i32 {
    let json = parsed.flag("json");
    match perform_rekey(json) {
        Ok(Some(outcome)) => {
            if json {
                logger::log(&rekey_json(
                    &outcome.from,
                    &outcome.to,
                    outcome.database,
                    outcome.config,
                ));
            } else {
                logger::success("KDF rekey complete (N=2^14). Future commands will be faster.");
            }
            0
        }
        Ok(None) => {
            if json {
                logger::log(&rekey_json("balanced", "balanced", false, false));
            } else {
                info("KDF already uses the latest work factor — nothing to do.");
            }
            0
        }
        Err(e) => {
            fail(&e);
            1
        }
    }
}

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
    let json = parsed.flag("json");
    let do_check = parsed.bool("check").unwrap_or(true);
    let do_vacuum = parsed.flag("vacuum");

    let conn = match db::open_db() {
        Ok(c) => c,
        Err(e) => {
            fail(&e.to_string());
            return 1;
        }
    };

    let mut results: Vec<String> = Vec::new();

    if do_check {
        let check_result = db::integrity_check(&conn);
        if json {
            results.push(format!(
                "  \"integrityCheck\": {}",
                crate::json::quote_string(if check_result == "ok" {
                    "passed"
                } else {
                    &check_result
                })
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
