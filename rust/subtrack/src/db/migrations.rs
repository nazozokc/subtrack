use rusqlite::Connection;

pub fn open_db() -> std::io::Result<Connection> {
    let dir = crate::crypto::config_dir();
    let db_path = dir.join("subtrack.db");
    let open_path = dir.join(".subtrack.open.db");

    if let Ok(raw) = std::fs::read(&db_path) {
        let plain =
            if raw.len() >= 16 && !raw[..16].eq(crate::crypto::SQLITE_MAGIC) && raw.len() > 32 {
                let key = crate::crypto::get_or_create_key()?;
                crate::crypto::decrypt_buffer(&key, &raw)
                    .ok_or_else(|| std::io::Error::other("database decryption failed"))?
            } else {
                raw
            };
        std::fs::write(&open_path, &plain)?;
    } else {
        std::fs::create_dir_all(&dir)?;
        let _ = std::fs::remove_file(&open_path);
    }

    let conn = Connection::open(&open_path).map_err(|e| std::io::Error::other(e.to_string()))?;
    conn.pragma_update(None, "foreign_keys", true)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    conn.pragma_update(None, "secure_delete", true)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    run_migrations(&conn).map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(conn)
}

/// Port of `runMigrations` + `migrateToV1` (`db/schema.ts`). Idempotent.
pub fn run_migrations(conn: &Connection) -> rusqlite::Result<()> {
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);
    if version < 1 {
        conn.execute_batch("BEGIN TRANSACTION")?;
        let result = (|| -> rusqlite::Result<()> {
            migrate_to_v1(conn)?;
            conn.execute_batch("PRAGMA user_version = 1")?;
            Ok(())
        })();
        match result {
            Ok(()) => conn.execute_batch("COMMIT")?,
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                return Err(e);
            }
        }
    }
    ensure_usage_generation_index(conn)?;
    Ok(())
}

fn table_has_column(conn: &Connection, table: &str, column: &str) -> rusqlite::Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let names: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(names.iter().any(|n| n == column))
}

/// Detect a UNIQUE-constraint failure on `CREATE UNIQUE INDEX`.
fn is_unique_violation(e: &rusqlite::Error) -> bool {
    let msg = e.to_string();
    msg.contains("UNIQUE constraint failed") || msg.contains("already exists")
}

fn ensure_usage_generation_index(conn: &Connection) -> rusqlite::Result<()> {
    if let Err(e) = conn.execute_batch(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_llm_usage_generation_id ON llm_usage(generation_id)",
    ) {
        if !is_unique_violation(&e) {
            return Err(e);
        }
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_llm_usage_generation_id_lookup ON llm_usage(generation_id)",
        )?;
    }
    Ok(())
}

fn migrate_to_v1(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS subscriptions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            price INTEGER NOT NULL,
            currency TEXT NOT NULL,
            cycle TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'active',
            billing_day INTEGER,
            created_at TEXT NOT NULL DEFAULT (date('now')),
            notes TEXT
        );
        CREATE TABLE IF NOT EXISTS tags (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE
        );
        CREATE TABLE IF NOT EXISTS subscription_tags (
            subscription_id INTEGER NOT NULL,
            tag_id INTEGER NOT NULL,
            PRIMARY KEY (subscription_id, tag_id),
            FOREIGN KEY (subscription_id) REFERENCES subscriptions(id) ON DELETE CASCADE,
            FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS llm_usage (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            provider TEXT NOT NULL,
            model TEXT NOT NULL,
            input_tokens INTEGER NOT NULL DEFAULT 0,
            output_tokens INTEGER NOT NULL DEFAULT 0,
            cost REAL NOT NULL,
            date TEXT NOT NULL,
            description TEXT,
            generation_id TEXT
        );
        CREATE TABLE IF NOT EXISTS trials (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            expires_at TEXT NOT NULL,
            price INTEGER,
            currency TEXT,
            cycle TEXT,
            notes TEXT,
            created_at TEXT NOT NULL DEFAULT (date('now'))
        );
        CREATE TABLE IF NOT EXISTS price_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            subscription_id INTEGER NOT NULL,
            old_price INTEGER,
            new_price INTEGER NOT NULL,
            old_currency TEXT,
            new_currency TEXT NOT NULL,
            changed_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (subscription_id) REFERENCES subscriptions(id) ON DELETE CASCADE
        );",
    )?;

    if !table_has_column(conn, "llm_usage", "generation_id")? {
        conn.execute_batch("ALTER TABLE llm_usage ADD COLUMN generation_id TEXT")?;
    }
    if !table_has_column(conn, "subscriptions", "notes")? {
        conn.execute_batch("ALTER TABLE subscriptions ADD COLUMN notes TEXT")?;
    }
    if !table_has_column(conn, "subscriptions", "payment_method")? {
        conn.execute_batch("ALTER TABLE subscriptions ADD COLUMN payment_method TEXT")?;
    }
    let extended: [(&str, &str); 8] = [
        ("contract_start", "TEXT"),
        ("contract_end", "TEXT"),
        ("auto_renewal", "INTEGER NOT NULL DEFAULT 1"),
        ("vendor_name", "TEXT"),
        ("vendor_url", "TEXT"),
        ("plan_tier", "TEXT"),
        ("discount_amount", "INTEGER"),
        ("discount_type", "TEXT"),
    ];
    for (name, ddl) in extended {
        if !table_has_column(conn, "subscriptions", name)? {
            conn.execute_batch(&format!(
                "ALTER TABLE subscriptions ADD COLUMN {} {}",
                name, ddl
            ))?;
        }
    }

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS audit_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            action TEXT NOT NULL,
            target_type TEXT,
            target_id INTEGER,
            details TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_audit_action ON audit_log(action);
        CREATE INDEX IF NOT EXISTS idx_audit_created ON audit_log(created_at);
        CREATE INDEX IF NOT EXISTS idx_audit_target ON audit_log(target_type, target_id);
        CREATE TABLE IF NOT EXISTS suggestions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            price INTEGER,
            currency TEXT,
            cycle TEXT,
            vendor_name TEXT,
            vendor_url TEXT,
            plan_tier TEXT,
            payment_method TEXT,
            source TEXT NOT NULL DEFAULT 'email',
            source_detail TEXT,
            email_subject TEXT,
            email_from TEXT,
            email_date TEXT,
            confidence REAL DEFAULT 0.0,
            status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'dismissed', 'added')),
            matched_sub_id INTEGER,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (matched_sub_id) REFERENCES subscriptions(id) ON DELETE SET NULL
        );",
    )?;
    Ok(())
}
