use rusqlite::Connection;

use super::run_migrations;

// ── Backup / restore (port of db/connection.ts backup helpers) ──

/// Serialize the open DB to bytes via `VACUUM INTO` (port of `exportDbBytes`).
pub fn export_db_bytes(conn: &Connection) -> std::io::Result<Vec<u8>> {
    let dir = crate::crypto::config_dir();
    std::fs::create_dir_all(&dir)?;
    let tmp = dir.join(".subtrack.save.db");
    let _ = std::fs::remove_file(&tmp);
    let quoted = tmp.to_string_lossy().replace('\'', "''");
    conn.execute_batch(&format!("VACUUM INTO '{}'", quoted))
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let data = std::fs::read(&tmp)?;
    let _ = std::fs::remove_file(&tmp);
    Ok(data)
}

/// Default backup directory: `<config>/backups`.
pub fn default_backup_dir() -> std::path::PathBuf {
    crate::crypto::config_dir().join("backups")
}

fn file_name_of(path: &std::path::Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Write `<path>.sha256` with the file's SHA256 (port of `writeBackupHash`).
pub fn write_backup_hash(path: &std::path::Path) -> std::io::Result<()> {
    let content = std::fs::read(path)?;
    let hash = crate::crypto::sha256_hex(&content);
    crate::crypto::write_secret_file(
        &std::path::PathBuf::from(format!("{}.sha256", path.display())),
        format!("{}\n", hash).as_bytes(),
    )
}

/// Verify `<path>.sha256`. Missing sidecar warns and returns true (port of
/// `verifyBackupHash`).
pub fn verify_backup_hash(path: &std::path::Path) -> bool {
    let hash_path = std::path::PathBuf::from(format!("{}.sha256", path.display()));
    if !hash_path.exists() {
        crate::logger::warn(&format!(
            "No integrity hash found for \"{}\" — integrity cannot be verified.",
            file_name_of(path)
        ));
        return true;
    }
    let expected = std::fs::read_to_string(&hash_path)
        .unwrap_or_default()
        .trim()
        .to_string();
    let content = std::fs::read(path).unwrap_or_default();
    crate::crypto::sha256_hex(&content) == expected
}

fn gunzip(data: &[u8]) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut dec = flate2::read::GzDecoder::new(data);
    let mut out = Vec::new();
    dec.read_to_end(&mut out)?;
    Ok(out)
}

/// Replace the live database with the contents of `backup_path`.
/// Port of `restoreDb` (validated + migrated before it becomes active).
pub fn restore_db(backup_path: &std::path::Path) -> std::io::Result<()> {
    let raw = std::fs::read(backup_path)?;
    let lower = backup_path.to_string_lossy().to_lowercase();
    let has_gzip_magic = raw.len() >= 2 && raw[0] == 0x1f && raw[1] == 0x8b;
    let key = crate::crypto::get_or_create_key()?;

    let decrypt_err = || {
        std::io::Error::other(
            "Failed to decrypt backup. The encryption key may have changed or the backup is corrupted.\n  If you changed your database passphrase or .key file, restore using the old key.",
        )
    };

    let data = if lower.ends_with(".db.enc") {
        crate::crypto::decrypt_buffer(&key, &raw).ok_or_else(decrypt_err)?
    } else if lower.ends_with(".gz") || has_gzip_magic {
        let decompressed = gunzip(&raw)?;
        if crate::crypto::is_encrypted(&decompressed) {
            crate::crypto::decrypt_buffer(&key, &decompressed).ok_or_else(decrypt_err)?
        } else {
            decompressed
        }
    } else if crate::crypto::is_encrypted(&raw) {
        crate::crypto::decrypt_buffer(&key, &raw).ok_or_else(decrypt_err)?
    } else {
        raw
    };

    let buf = if data.len() >= 2 && data[0] == 0x1f && data[1] == 0x8b {
        gunzip(&data)?
    } else {
        data
    };

    let dir = crate::crypto::config_dir();
    std::fs::create_dir_all(&dir)?;

    // Validate + migrate against the throwaway save path before replacing live.
    let save_path = dir.join(".subtrack.save.db");
    std::fs::write(&save_path, &buf)?;
    let validate = (|| -> std::io::Result<()> {
        let c = Connection::open(&save_path).map_err(|e| std::io::Error::other(e.to_string()))?;
        let ok: String = c
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        if ok != "ok" {
            return Err(std::io::Error::other(format!(
                "Invalid backup file: integrity check failed ({})",
                ok
            )));
        }
        let has_subs: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='subscriptions'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if has_subs == 0 {
            return Err(std::io::Error::other(
                "Invalid backup file: subscriptions table is missing",
            ));
        }
        run_migrations(&c).map_err(|e| std::io::Error::other(e.to_string()))?;
        Ok(())
    })();
    if let Err(e) = validate {
        let _ = std::fs::remove_file(&save_path);
        return Err(e);
    }

    let restored = std::fs::read(&save_path)?;
    let _ = std::fs::remove_file(&save_path);

    let encrypted = crate::crypto::encrypt_buffer(&key, &restored);
    let db_path = dir.join("subtrack.db");
    crate::crypto::write_secret_file(&db_path, &encrypted)?;
    crate::crypto::write_secret_file(
        &dir.join("subtrack.db.sha256"),
        format!("{}\n", crate::crypto::sha256_hex(&encrypted)).as_bytes(),
    )?;
    let _ = std::fs::remove_file(dir.join(".subtrack.open.db"));
    Ok(())
}
