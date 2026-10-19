//! `backup` / `restore` commands — port of `backup.ts`.

use std::io::Write;
use std::path::PathBuf;

use chrono::Datelike;
use chrono::Timelike;

use crate::logger;
use crate::parser::ParsedArgs;
use crate::safe_path::{safe_output_path, safe_path};

fn timestamp() -> String {
    let now = chrono::Utc::now();
    format!(
        "{}{:02}{:02}_{:02}{:02}{:02}",
        now.year(),
        now.month(),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    )
}

fn basename(path: &std::path::Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Compress the current DB and write it (exclusive create). Returns success.
fn write_compressed_backup(dest: &std::path::Path, encrypt: bool) -> bool {
    let conn = match crate::db::open_db() {
        Ok(c) => c,
        Err(e) => {
            logger::fail(&format!("Backup failed: {}", e));
            return false;
        }
    };
    let sqlite_buf = match crate::db::export_db_bytes(&conn) {
        Ok(b) => b,
        Err(e) => {
            logger::fail(&format!("Backup failed: {}", e));
            return false;
        }
    };

    let compressed = {
        use flate2::Compression;
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), Compression::default());
        if enc.write_all(&sqlite_buf).is_err() {
            logger::fail("Backup failed");
            return false;
        }
        match enc.finish() {
            Ok(v) => v,
            Err(e) => {
                logger::fail(&format!("Backup failed: {}", e));
                return false;
            }
        }
    };

    let write_buf = if encrypt {
        match crate::crypto::get_or_create_key() {
            Ok(key) => crate::crypto::encrypt_buffer(&key, &compressed),
            Err(e) => {
                logger::fail(&format!("Backup failed: {}", e));
                return false;
            }
        }
    } else {
        compressed
    };

    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    match opts.open(dest) {
        Ok(mut f) => {
            if let Err(e) = f.write_all(&write_buf) {
                logger::fail(&format!("Backup failed: {}", e));
                return false;
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            logger::fail(&format!("Backup file already exists: {}", dest.display()));
            return false;
        }
        Err(e) => {
            logger::fail(&format!("Backup failed: {}", e));
            return false;
        }
    }
    let _ = crate::db::write_backup_hash(dest);
    true
}

/// Create an automatic safety backup before a restore. Port of `safeAutoBackup`.
fn safe_auto_backup() -> bool {
    if let Ok(conn) = crate::db::open_db() {
        let _ = crate::db::save_db(&conn);
    }
    let backup_dir = crate::db::default_backup_dir();
    if let Err(e) = std::fs::create_dir_all(&backup_dir) {
        logger::fail(&format!("Auto-backup failed: {}", e));
        return false;
    }
    let ts = timestamp();
    let encrypt = crate::crypto::has_encryption_key();
    let ext = if encrypt { ".db.enc" } else { ".db.gz" };
    let dest = backup_dir.join(format!("subtrack_{}_before_restore{}", ts, ext));

    if !write_compressed_backup(&dest, encrypt) {
        return false;
    }
    logger::info(&format!(
        "Auto-backup created: {}{}",
        dest.display(),
        if encrypt { " (encrypted)" } else { "" }
    ));
    true
}

fn restore_from_file(file_path: &std::path::Path, force: bool) {
    if !force {
        // Interactive confirmation is unavailable on a non-TTY stdin; the
        // prompt aborts, matching the reference `confirm` cancel path.
        return;
    }

    if !crate::db::verify_backup_hash(file_path) {
        logger::warn("Backup integrity check failed (SHA256 mismatch)");
        if !force {
            return;
        }
    }

    if !safe_auto_backup() {
        logger::warn("Restore aborted because the current database could not be backed up");
        return;
    }

    if let Err(e) = crate::db::restore_db(file_path) {
        logger::fail(&e.to_string());
        return;
    }

    let subs =
        crate::db::get_subscriptions(&crate::db::QueryOptions::default()).unwrap_or_default();
    let n = subs.len();
    let details = format!("Restored {} subscriptions from {}", n, basename(file_path));
    if let Ok(conn) = crate::db::open_db() {
        let _ = crate::db::add_audit_log(&conn, "backup.restore", None, None, Some(&details), true);
    }
    logger::success(&format!(
        "Restored {} subscriptions from: {}",
        n,
        file_path.display()
    ));
}

/// Port of `handleBackup`.
pub fn run_backup(parsed: &ParsedArgs) -> i32 {
    if let Ok(conn) = crate::db::open_db() {
        let _ = crate::db::save_db(&conn);
    }

    let encrypt = parsed.bool("encrypt") == Some(true);

    let dest: PathBuf = match parsed.str("destination") {
        Some(d) => match safe_output_path(&d) {
            Some(p) => p,
            None => {
                logger::fail("Invalid backup destination — must be within home directory");
                return 0;
            }
        },
        None => crate::db::default_backup_dir(),
    };

    if !dest.exists() {
        if let Err(e) = std::fs::create_dir_all(&dest) {
            logger::fail(&format!("Backup destination is not accessible: {}", e));
            return 0;
        }
    } else if !dest.is_dir() {
        logger::fail(&format!(
            "Backup destination must be a directory: {}",
            dest.display()
        ));
        return 0;
    }

    if !encrypt && crate::crypto::has_encryption_key() {
        logger::warn(
            "Database is encrypted but backup will NOT be encrypted.\n  Use --encrypt (-e) to encrypt the backup.",
        );
    }

    let ts = timestamp();
    let dest_path = if encrypt {
        dest.join(format!("subtrack_{}.db.enc", ts))
    } else {
        dest.join(format!("subtrack_{}.db.gz", ts))
    };

    if write_compressed_backup(&dest_path, encrypt) {
        logger::success(&format!(
            "Backup created: {}{}",
            dest_path.display(),
            if encrypt { " (encrypted)" } else { "" }
        ));
    }
    0
}

/// Port of `handleRestore`.
pub fn run_restore(parsed: &ParsedArgs) -> i32 {
    if let Some(file) = parsed.str("file") {
        let Some(safe_file) = safe_path(&file) else {
            let msg = if std::path::Path::new(&file).exists() {
                "Invalid backup file — must be within home or temp directory".to_string()
            } else {
                format!("Backup file not found: {}", file)
            };
            logger::fail(&msg);
            return 1;
        };
        let force = parsed.bool("force") == Some(true);
        restore_from_file(&safe_file, force);
        return 0;
    }

    let search_dir: PathBuf = match parsed.str("dir") {
        Some(d) => match safe_path(&d) {
            Some(p) => p,
            None => {
                logger::fail("Invalid search directory — must be within home directory");
                return 1;
            }
        },
        None => crate::db::default_backup_dir(),
    };

    let backups = get_backup_files(&search_dir);
    if backups.is_empty() {
        logger::info(&format!(
            "No backup files found in: {}",
            search_dir.display()
        ));
        return 0;
    }

    // Interactive selection is unavailable on a non-TTY stdin.
    0
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct BackupFileInfo {
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
}

/// Port of `getBackupFiles` (only the fields used by the CLI).
pub fn get_backup_files(dir: &std::path::Path) -> Vec<BackupFileInfo> {
    let mut out: Vec<BackupFileInfo> = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let lower = name.to_lowercase();
        if !(lower.ends_with(".gz") || lower.ends_with(".db") || lower.ends_with(".db.enc")) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        out.push(BackupFileInfo {
            name,
            path: entry.path(),
            size: meta.len(),
        });
    }
    out.sort_by(|a, b| b.name.cmp(&a.name));
    out
}
