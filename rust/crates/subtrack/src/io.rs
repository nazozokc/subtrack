//! `import` (CSV) and `restore` commands — port of the non-interactive error
//! paths of `import-csv.ts` (`handleImport`) and `backup.ts` (`handleRestore`).

use crate::logger;
use crate::parser::ParsedArgs;
use crate::safe_path::safe_path;

/// Port of `handleImport` (path validation only).
pub fn run_import(parsed: &ParsedArgs) -> i32 {
    let file = parsed.str("file").unwrap_or_default();
    if file.is_empty() {
        logger::fail("Usage: subtrack import <file> [--dry-run]");
        return 1;
    }
    if safe_path(&file).is_none() {
        logger::fail("File not found or path not allowed — must be within home or temp directory");
        return 1;
    }
    // CSV parsing/import is deferred to the mutating phase.
    0
}

/// Port of `handleRestore` (non-interactive path).
pub fn run_restore(parsed: &ParsedArgs) -> i32 {
    let Some(file) = parsed.str("file") else {
        // Interactive selection is not available in this environment.
        return 0;
    };
    if safe_path(&file).is_none() {
        let msg = if std::path::Path::new(&file).exists() {
            "Invalid backup file — must be within home or temp directory".to_string()
        } else {
            format!("Backup file not found: {}", file)
        };
        logger::fail(&msg);
        return 1;
    }
    // Actual restore is deferred to the mutating phase.
    0
}
