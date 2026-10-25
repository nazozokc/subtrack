use rusqlite::Connection;

pub fn db_path() -> std::path::PathBuf {
    subtrack_core::crypto::config_dir().join("subtrack.db")
}

/// Size in bytes of the encrypted database file (`getFileSize`).
pub fn db_file_size() -> i64 {
    std::fs::metadata(db_path())
        .map(|m| m.len() as i64)
        .unwrap_or(0)
}

/// `PRAGMA integrity_check` → the reported string (defaults to "ok").
pub fn integrity_check(conn: &Connection) -> String {
    conn.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
        .unwrap_or_else(|_| "ok".to_string())
}

/// Run `VACUUM` and persist. Port of the maintenance/cleanup VACUUM step.
pub fn vacuum_db(conn: &Connection) -> std::io::Result<()> {
    conn.execute_batch("VACUUM")
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    save_db(conn)
}

// ── Write layer (port of db/connection.ts saveDb + writeSubscription + audit) ──

/// Serialize the open connection to `subtrack.db`, encrypting it.
/// Port of `saveDb()`: `VACUUM INTO` a temp file, then encrypt + replace.
pub fn save_db(conn: &Connection) -> std::io::Result<()> {
    let dir = subtrack_core::crypto::config_dir_checked()?;
    subtrack_core::crypto::ensure_private_dir(&dir)?;
    // A random name prevents clobbering another process or following a
    // pre-created symlink, and the 0700 directory hides the plaintext dump.
    let tmp = dir.join(format!(
        ".subtrack.save.{}.db",
        subtrack_core::crypto::random_hex(8)
    ));
    subtrack_core::crypto::track_temp_file(&tmp);
    let quoted = tmp.to_string_lossy().replace('\'', "''");
    conn.execute_batch(&format!("VACUUM INTO '{}'", quoted))
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    subtrack_core::crypto::restrict_file_permissions(&tmp)?;
    let data = std::fs::read(&tmp)?;
    let _ = std::fs::remove_file(&tmp);

    let key = subtrack_core::crypto::get_or_create_key()?;
    let encrypted = subtrack_core::crypto::encrypt_buffer(&key, &data);
    write_db_atomic(&dir, &encrypted)
}

/// Write the encrypted database and its hash sidecar, then atomically rename
/// both into place. Prevents a crash mid-write from truncating the live DB.
pub(crate) fn write_db_atomic(dir: &std::path::Path, encrypted: &[u8]) -> std::io::Result<()> {
    let token = subtrack_core::crypto::random_hex(8);
    let db_tmp = dir.join(format!(".subtrack.db.{token}.tmp"));
    let hash_tmp = dir.join(format!(".subtrack.db.{token}.sha256.tmp"));
    let db_path = dir.join("subtrack.db");
    let hash_path = dir.join("subtrack.db.sha256");
    subtrack_core::crypto::track_temp_file(&db_tmp);
    subtrack_core::crypto::track_temp_file(&hash_tmp);

    if let Err(e) = subtrack_core::crypto::write_secret_file(&db_tmp, encrypted) {
        let _ = std::fs::remove_file(&db_tmp);
        return Err(e);
    }
    let hash = format!("{}\n", subtrack_core::crypto::sha256_hex(encrypted));
    if let Err(e) = subtrack_core::crypto::write_secret_file(&hash_tmp, hash.as_bytes()) {
        let _ = std::fs::remove_file(&db_tmp);
        let _ = std::fs::remove_file(&hash_tmp);
        return Err(e);
    }
    if let Err(e) = std::fs::rename(&db_tmp, &db_path) {
        let _ = std::fs::remove_file(&db_tmp);
        let _ = std::fs::remove_file(&hash_tmp);
        return Err(e);
    }
    if let Err(e) = std::fs::rename(&hash_tmp, &hash_path) {
        // The database is safely in place; drop the stale sidecar so the next
        // open regenerates a matching hash instead of warning.
        let _ = std::fs::remove_file(&hash_path);
        let _ = std::fs::remove_file(&hash_tmp);
        return Err(e);
    }
    Ok(())
}
