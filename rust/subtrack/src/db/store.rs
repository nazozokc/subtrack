use rusqlite::Connection;

pub fn db_path() -> std::path::PathBuf {
    crate::crypto::config_dir().join("subtrack.db")
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
    let dir = crate::crypto::config_dir();
    std::fs::create_dir_all(&dir)?;
    let tmp = dir.join(".subtrack.save.db");
    let _ = std::fs::remove_file(&tmp);
    let quoted = tmp.to_string_lossy().replace('\'', "''");
    conn.execute_batch(&format!("VACUUM INTO '{}'", quoted))
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let data = std::fs::read(&tmp)?;
    let _ = std::fs::remove_file(&tmp);

    let key = crate::crypto::get_or_create_key()?;
    let encrypted = crate::crypto::encrypt_buffer(&key, &data);
    let db_path = dir.join("subtrack.db");
    crate::crypto::write_secret_file(&db_path, &encrypted)?;
    let sidecar = dir.join("subtrack.db.sha256");
    crate::crypto::write_secret_file(
        &sidecar,
        format!("{}\n", crate::crypto::sha256_hex(&encrypted)).as_bytes(),
    )?;
    Ok(())
}
