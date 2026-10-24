use std::collections::HashSet;

use rusqlite::Connection;

use super::run_migrations;

// ── Backup / restore (port of db/connection.ts backup helpers) ──

/// Upper bound for a decompressed backup (port of `MAX_DECOMPRESSED_BYTES`).
const MAX_DECOMPRESSED_BYTES: u64 = 256 * 1024 * 1024;
/// Upper bound for the backup file itself; a larger file cannot represent a
/// database that fits the decompression cap.
const MAX_BACKUP_FILE_BYTES: u64 = 256 * 1024 * 1024;
const SCHEMA_VERSION: i64 = 1;

const LEGACY_TABLES: [&str; 3] = ["subscriptions", "tags", "subscription_tags"];
const REQUIRED_TABLES: [&str; 8] = [
    "subscriptions",
    "tags",
    "subscription_tags",
    "llm_usage",
    "trials",
    "price_history",
    "audit_log",
    "suggestions",
];
const REQUIRED_COLUMNS: [(&str, &[&str]); 8] = [
    (
        "subscriptions",
        &[
            "id",
            "name",
            "price",
            "currency",
            "cycle",
            "status",
            "billing_day",
            "created_at",
            "notes",
            "payment_method",
            "contract_start",
            "contract_end",
            "auto_renewal",
            "vendor_name",
            "vendor_url",
            "plan_tier",
            "discount_amount",
            "discount_type",
        ],
    ),
    ("tags", &["id", "name"]),
    ("subscription_tags", &["subscription_id", "tag_id"]),
    (
        "llm_usage",
        &[
            "id",
            "provider",
            "model",
            "input_tokens",
            "output_tokens",
            "cost",
            "date",
            "description",
            "generation_id",
        ],
    ),
    (
        "trials",
        &[
            "id",
            "name",
            "expires_at",
            "price",
            "currency",
            "cycle",
            "notes",
            "created_at",
        ],
    ),
    (
        "price_history",
        &[
            "id",
            "subscription_id",
            "old_price",
            "new_price",
            "old_currency",
            "new_currency",
            "changed_at",
        ],
    ),
    (
        "audit_log",
        &[
            "id",
            "action",
            "target_type",
            "target_id",
            "details",
            "created_at",
        ],
    ),
    (
        "suggestions",
        &[
            "id",
            "name",
            "price",
            "currency",
            "cycle",
            "vendor_name",
            "vendor_url",
            "plan_tier",
            "payment_method",
            "source",
            "source_detail",
            "email_subject",
            "email_from",
            "email_date",
            "confidence",
            "status",
            "matched_sub_id",
            "created_at",
        ],
    ),
];
const LEGACY_COLUMNS: [(&str, &[&str]); 3] = [
    (
        "subscriptions",
        &[
            "id",
            "name",
            "price",
            "currency",
            "cycle",
            "status",
            "billing_day",
            "created_at",
        ],
    ),
    ("tags", &["id", "name"]),
    ("subscription_tags", &["subscription_id", "tag_id"]),
];

/// Serialize the open DB to bytes via `VACUUM INTO` (port of `exportDbBytes`).
pub fn export_db_bytes(conn: &Connection) -> std::io::Result<Vec<u8>> {
    let dir = crate::crypto::config_dir_checked()?;
    crate::crypto::ensure_private_dir(&dir)?;
    let tmp = dir.join(format!(
        ".subtrack.save.{}.db",
        crate::crypto::random_hex(8)
    ));
    crate::crypto::track_temp_file(&tmp);
    let quoted = tmp.to_string_lossy().replace('\'', "''");
    conn.execute_batch(&format!("VACUUM INTO '{}'", quoted))
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    crate::crypto::restrict_file_permissions(&tmp)?;
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
    // Read at most one byte past the cap so a decompression bomb cannot
    // exhaust memory before the size check.
    let mut dec = flate2::read::GzDecoder::new(data).take(MAX_DECOMPRESSED_BYTES + 1);
    let mut out = Vec::new();
    dec.read_to_end(&mut out)?;
    if out.len() as u64 > MAX_DECOMPRESSED_BYTES {
        return Err(std::io::Error::other(format!(
            "Backup decompresses to more than {} MB — refusing to restore (possible decompression bomb)",
            MAX_DECOMPRESSED_BYTES / 1024 / 1024
        )));
    }
    Ok(out)
}

fn table_columns(conn: &Connection, table: &str) -> std::io::Result<HashSet<String>> {
    // `table` is always one of the hard-coded constants above.
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({})", table))
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let names = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut out = HashSet::new();
    for name in names {
        out.insert(name.map_err(|e| std::io::Error::other(e.to_string()))?);
    }
    Ok(out)
}

/// Validate the schema before a restore may replace the live database.
/// Port of `validateRestoredDatabase`.
fn validate_restored_database(conn: &Connection) -> std::io::Result<()> {
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    if !(0..=SCHEMA_VERSION).contains(&version) {
        return Err(std::io::Error::other(format!(
            "Invalid backup file: schema version {version} is not supported (max {SCHEMA_VERSION})"
        )));
    }

    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let tables: HashSet<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| std::io::Error::other(e.to_string()))?
        .filter_map(|row| row.ok())
        .collect();

    let (required_tables, required_columns): (&[&str], &[(&str, &[&str])]) = if version < 1 {
        (&LEGACY_TABLES, &LEGACY_COLUMNS)
    } else {
        (&REQUIRED_TABLES, &REQUIRED_COLUMNS)
    };

    let missing: Vec<&str> = required_tables
        .iter()
        .copied()
        .filter(|table| !tables.contains(*table))
        .collect();
    if !missing.is_empty() {
        return Err(std::io::Error::other(format!(
            "Invalid backup file: missing table(s): {}",
            missing.join(", ")
        )));
    }

    for (table, columns) in required_columns {
        let present = table_columns(conn, table)?;
        let missing: Vec<&str> = columns
            .iter()
            .copied()
            .filter(|column| !present.contains(*column))
            .collect();
        if !missing.is_empty() {
            return Err(std::io::Error::other(format!(
                "Invalid backup file: {table} is missing column(s): {}",
                missing.join(", ")
            )));
        }
    }

    let mut stmt = conn
        .prepare("PRAGMA foreign_key_check")
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut rows = stmt
        .query([])
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    if rows
        .next()
        .map_err(|e| std::io::Error::other(e.to_string()))?
        .is_some()
    {
        return Err(std::io::Error::other(
            "Invalid backup file: foreign key integrity check failed",
        ));
    }
    Ok(())
}

/// Replace the live database with the contents of `backup_path`.
/// Port of `restoreDb` (validated + migrated before it becomes active).
pub fn restore_db(backup_path: &std::path::Path) -> std::io::Result<()> {
    let size = std::fs::metadata(backup_path)?.len();
    if size > MAX_BACKUP_FILE_BYTES {
        return Err(std::io::Error::other(format!(
            "Backup file is larger than {} MB — refusing to restore",
            MAX_BACKUP_FILE_BYTES / 1024 / 1024
        )));
    }
    let raw = std::fs::read(backup_path)?;
    let lower = backup_path.to_string_lossy().to_lowercase();
    let has_gzip_magic = raw.len() >= 2 && raw[0] == 0x1f && raw[1] == 0x8b;

    let decrypt_err = || {
        std::io::Error::other(
            "Failed to decrypt backup. The encryption key may have changed or the backup is corrupted.\n  If you changed your database passphrase or .key file, restore using the old key.",
        )
    };

    let data = if lower.ends_with(".db.enc") {
        crate::crypto::decrypt_with_fallback(&raw)?.ok_or_else(decrypt_err)?
    } else if lower.ends_with(".gz") || has_gzip_magic {
        let decompressed = gunzip(&raw)?;
        if crate::crypto::is_encrypted(&decompressed) {
            crate::crypto::decrypt_with_fallback(&decompressed)?.ok_or_else(decrypt_err)?
        } else {
            decompressed
        }
    } else if crate::crypto::is_encrypted(&raw) {
        crate::crypto::decrypt_with_fallback(&raw)?.ok_or_else(decrypt_err)?
    } else {
        raw
    };

    let buf = if data.len() >= 2 && data[0] == 0x1f && data[1] == 0x8b {
        gunzip(&data)?
    } else {
        data
    };

    // The restored database is always encrypted; this may create a fresh key.
    let key = crate::crypto::get_or_create_key()?;

    let dir = crate::crypto::config_dir_checked()?;
    crate::crypto::ensure_private_dir(&dir)?;

    // Validate + migrate against a throwaway save path before replacing live.
    let save_path = dir.join(format!(
        ".subtrack.save.{}.db",
        crate::crypto::random_hex(8)
    ));
    crate::crypto::track_temp_file(&save_path);
    crate::crypto::write_secret_file(&save_path, &buf)?;
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
        validate_restored_database(&c)?;
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
    super::store::write_db_atomic(&dir, &encrypted)?;
    let _ = std::fs::remove_file(dir.join(".subtrack.open.db"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn current_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn accepts_current_schema() {
        validate_restored_database(&current_db()).unwrap();
    }

    #[test]
    fn accepts_legacy_v0_schema() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE subscriptions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                price INTEGER NOT NULL,
                currency TEXT NOT NULL,
                cycle TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'active',
                billing_day INTEGER,
                created_at TEXT NOT NULL DEFAULT (date('now'))
            );
            CREATE TABLE tags (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE);
            CREATE TABLE subscription_tags (
                subscription_id INTEGER NOT NULL,
                tag_id INTEGER NOT NULL,
                PRIMARY KEY (subscription_id, tag_id)
            );",
        )
        .unwrap();
        validate_restored_database(&conn).unwrap();
    }

    #[test]
    fn rejects_newer_schema_version() {
        let conn = current_db();
        conn.execute_batch("PRAGMA user_version = 2").unwrap();
        let err = validate_restored_database(&conn).unwrap_err();
        assert!(err.to_string().contains("schema version"), "{err}");
    }

    #[test]
    fn rejects_missing_table() {
        let conn = current_db();
        conn.execute_batch("DROP TABLE suggestions").unwrap();
        let err = validate_restored_database(&conn).unwrap_err();
        assert!(err.to_string().contains("missing table"), "{err}");
    }

    #[test]
    fn rejects_missing_column() {
        let conn = current_db();
        conn.execute_batch("ALTER TABLE suggestions DROP COLUMN source_detail")
            .unwrap();
        let err = validate_restored_database(&conn).unwrap_err();
        assert!(err.to_string().contains("missing column"), "{err}");
    }

    #[test]
    fn rejects_foreign_key_violations() {
        let conn = current_db();
        // Disable enforcement so the violating row can be inserted; the
        // validation check is what must catch it.
        conn.execute_batch(
            "PRAGMA foreign_keys = OFF; INSERT INTO subscription_tags VALUES (999, 999)",
        )
        .unwrap();
        let err = validate_restored_database(&conn).unwrap_err();
        assert!(err.to_string().contains("foreign key"), "{err}");
    }
}
