//! Database access — port of `db/subscriptions.ts` + `db/usage.ts` queries.
//! The on-disk `subtrack.db` is decrypted to a temp backing file and opened
//! with rusqlite; the temp file is removed after opening.

use rusqlite::{Connection, params};

#[derive(Clone, Debug)]
pub struct Subscription {
    pub id: i64,
    pub name: String,
    pub price: f64,
    pub currency: String,
    pub cycle: String,
    pub status: String,
    pub billing_day: Option<i64>,
    pub created_at: String,
    pub notes: Option<String>,
    pub payment_method: Option<String>,
    pub contract_start: Option<String>,
    pub contract_end: Option<String>,
    pub auto_renewal: bool,
    pub vendor_name: Option<String>,
    pub vendor_url: Option<String>,
    pub plan_tier: Option<String>,
    pub discount_amount: Option<f64>,
    pub discount_type: Option<String>,
    pub tags: Vec<String>,
}

pub const SORT_FIELDS: [&str; 18] = [
    "id",
    "name",
    "price",
    "currency",
    "cycle",
    "status",
    "billingDay",
    "createdAt",
    "paymentMethod",
    "contractStart",
    "contractEnd",
    "autoRenewal",
    "vendorName",
    "vendorUrl",
    "planTier",
    "discountAmount",
    "discountType",
    "tags",
];

const SUB_COLUMNS: [&str; 18] = [
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
];

#[derive(Default, Clone)]
pub struct QueryOptions {
    pub sort: Option<String>,
    pub desc: bool,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub include_archived: bool,
    pub status: Option<String>,
    pub min_price: Option<f64>,
    pub max_price: Option<f64>,
    pub tags: Vec<String>,
}

/// Open the (possibly encrypted) database file.
///
/// Mirrors `getDb()` in `db/connection.ts`: the live `Connection` always backs
/// onto a decrypted temp file (`.subtrack.open.db`); `subtrack.db` holds the
/// (encrypted) export written by `save_db`.
pub fn open_db() -> std::io::Result<Connection> {
    let dir = crate::crypto::config_dir();
    let db_path = dir.join("subtrack.db");
    let open_path = dir.join(".subtrack.open.db");

    if let Ok(raw) = std::fs::read(&db_path) {
        let plain = if raw.len() >= 16 && !raw[..16].eq(crate::crypto::SQLITE_MAGIC)
            && raw.len() > 32
        {
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

fn row_to_subscription(row: &rusqlite::Row) -> rusqlite::Result<Subscription> {
    Ok(Subscription {
        id: row.get(0)?,
        name: row.get(1)?,
        price: row.get::<_, f64>(2)?,
        currency: row.get(3)?,
        cycle: row.get(4)?,
        status: row.get(5)?,
        billing_day: row.get(6)?,
        created_at: row.get(7)?,
        notes: row.get(8)?,
        payment_method: row.get(9)?,
        contract_start: row.get(10)?,
        contract_end: row.get(11)?,
        auto_renewal: row.get::<_, i64>(12)? != 0,
        vendor_name: row.get(13)?,
        vendor_url: row.get(14)?,
        plan_tier: row.get(15)?,
        discount_amount: row.get::<_, Option<f64>>(16)?,
        discount_type: row.get(17)?,
        tags: Vec::new(),
    })
}

/// Port of `mapTags` + `getSubscriptions`.
pub fn get_subscriptions(options: &QueryOptions) -> std::io::Result<Vec<Subscription>> {
    let conn = open_db()?;
    let mut subs = query_subscriptions(&conn, options)?;
    map_tags(&conn, &mut subs)?;
    Ok(subs)
}

fn query_subscriptions(
    conn: &Connection,
    options: &QueryOptions,
) -> std::io::Result<Vec<Subscription>> {
    let field = match &options.sort {
        Some(s) if SORT_FIELDS.contains(&s.as_str()) => s.clone(),
        _ => "id".to_string(),
    };
    let order = if options.desc { "DESC" } else { "ASC" };

    let mut conditions: Vec<String> = Vec::new();
    if !options.include_archived {
        conditions.push("status != 'archived'".to_string())
    }
    if options.status.is_some() {
        conditions.push("status = ?".to_string())
    }
    if options.min_price.is_some() {
        conditions.push("price >= ?".to_string())
    }
    if options.max_price.is_some() {
        conditions.push("price <= ?".to_string())
    }
    let mut tags: Vec<String> = Vec::new();
    if !options.tags.is_empty() {
        let mut seen = std::collections::HashSet::new();
        for t in &options.tags {
            if seen.insert(t.clone()) {
                tags.push(t.clone())
            }
        }
    }
    if !tags.is_empty() {
        let placeholders = vec!["?"; tags.len()].join(",");
        conditions.push(format!(
            "id IN (SELECT st.subscription_id FROM subscription_tags st JOIN tags t ON t.id = st.tag_id WHERE t.name IN ({}) GROUP BY st.subscription_id HAVING COUNT(DISTINCT t.name) = ?)",
            placeholders
        ))
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };
    let mut limit_clause = String::new();
    let mut offset_clause = String::new();
    if options.limit.is_some() {
        limit_clause = "LIMIT ?".to_string()
    }
    if options.offset.is_some() {
        offset_clause = "OFFSET ?".to_string()
    }

    let sql = format!(
        "SELECT {} FROM subscriptions {} ORDER BY {} {} {} {}",
        SUB_COLUMNS.join(", "),
        where_clause,
        field,
        order,
        limit_clause,
        offset_clause
    );

    let mut params: Vec<rusqlite::types::Value> = Vec::new();
    if let Some(s) = &options.status {
        params.push(rusqlite::types::Value::Text(s.clone()))
    }
    if let Some(p) = options.min_price {
        params.push(rusqlite::types::Value::Real(p))
    }
    if let Some(p) = options.max_price {
        params.push(rusqlite::types::Value::Real(p))
    }
    for t in &tags {
        params.push(rusqlite::types::Value::Text(t.clone()))
    }
    if !tags.is_empty() {
        params.push(rusqlite::types::Value::Integer(tags.len() as i64))
    }
    if let Some(l) = options.limit {
        params.push(rusqlite::types::Value::Integer(l))
    }
    if let Some(o) = options.offset {
        params.push(rusqlite::types::Value::Integer(o))
    }

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(params.iter()), row_to_subscription)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| std::io::Error::other(e.to_string()))?)
    }
    Ok(out)
}

fn map_tags(conn: &Connection, subs: &mut [Subscription]) -> std::io::Result<()> {
    if subs.is_empty() {
        return Ok(())
    }
    let ids: Vec<i64> = subs.iter().map(|s| s.id).collect();
    let placeholders = vec!["?"; ids.len()].join(",");
    let sql = format!(
        "SELECT subscription_tags.subscription_id, tags.name FROM tags \
         JOIN subscription_tags ON subscription_tags.tag_id = tags.id \
         WHERE subscription_tags.subscription_id IN ({})",
        placeholders
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let pairs: Vec<(i64, String)> = rows
        .filter_map(|r| r.ok())
        .collect();
    let mut tag_map: std::collections::HashMap<i64, Vec<String>> = std::collections::HashMap::new();
    for (id, name) in pairs {
        tag_map.entry(id).or_default().push(name)
    }
    for sub in subs {
        sub.tags = tag_map.get(&sub.id).cloned().unwrap_or_default()
    }
    Ok(())
}

/// Sum `cost` for all entries whose `date` falls within [from, to] (USD cents).
/// `cost` is stored as REAL; round the sum to whole cents.
pub fn get_llm_usage_total(from: &str, to: &str) -> std::io::Result<i64> {
    let conn = open_db()?;
    let total: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(cost), 0) AS total FROM llm_usage WHERE date >= ? AND date <= ?",
            params![from, to],
            |row| row.get(0),
        )
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(total.round() as i64)
}

/// Sum of `cost` grouped by provider for a date range, ordered by total DESC.
pub fn get_llm_usage_total_by_provider(
    from: &str,
    to: &str,
) -> std::io::Result<Vec<(String, i64)>> {
    let conn = open_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT provider, SUM(cost) AS total FROM llm_usage \
             WHERE date >= ? AND date <= ? GROUP BY provider ORDER BY total DESC",
        )
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let rows = stmt
        .query_map(params![from, to], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?.round() as i64))
        })
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| std::io::Error::other(e.to_string()))?)
    }
    Ok(out)
}

/// An LLM usage row (`cost` in USD cents).
#[derive(Clone, Debug)]
pub struct LlmUsage {
    pub id: i64,
    pub provider: String,
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost: f64,
    pub date: String,
    pub description: Option<String>,
}

/// Port of `getLlmUsage` (ORDER BY date DESC, id DESC).
pub fn get_llm_usage(
    provider: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
    min_cost: Option<f64>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> std::io::Result<Vec<LlmUsage>> {
    let conn = open_db()?;
    let mut conditions: Vec<&str> = Vec::new();
    let mut params: Vec<rusqlite::types::Value> = Vec::new();
    if let Some(p) = provider {
        conditions.push("provider = ?");
        params.push(p.to_string().into());
    }
    if let Some(f) = from {
        conditions.push("date >= ?");
        params.push(f.to_string().into());
    }
    if let Some(t) = to {
        conditions.push("date <= ?");
        params.push(t.to_string().into());
    }
    if let Some(mc) = min_cost {
        conditions.push("cost >= ?");
        params.push(mc.into());
    }
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };
    let pagination = match (limit, offset) {
        (Some(_), Some(_)) => " LIMIT ? OFFSET ?",
        (Some(_), None) => " LIMIT ?",
        (None, Some(_)) => " LIMIT -1 OFFSET ?",
        (None, None) => "",
    };
    if let Some(l) = limit {
        params.push(l.into());
    }
    if let Some(o) = offset {
        params.push(o.into());
    }
    let sql = format!(
        "SELECT id, provider, model, input_tokens, output_tokens, cost, date, description \
         FROM llm_usage {where_clause} ORDER BY date DESC, id DESC{pagination}"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(params.iter()), |row| {
            Ok(LlmUsage {
                id: row.get(0)?,
                provider: row.get(1)?,
                model: row.get(2)?,
                input_tokens: row.get(3)?,
                output_tokens: row.get(4)?,
                cost: row.get::<_, f64>(5)?,
                date: row.get(6)?,
                description: row.get(7)?,
            })
        })
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| std::io::Error::other(e.to_string()))?)
    }
    Ok(out)
}

/// A free-trial entry (`price` is in major units, nullable).
#[derive(Clone, Debug)]
pub struct Trial {
    pub id: i64,
    pub name: String,
    pub expires_at: String,
    pub price: Option<f64>,
    pub currency: Option<String>,
    pub cycle: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
}

/// All trials, oldest expiry first (port of `getTrials`).
pub fn get_trials() -> std::io::Result<Vec<Trial>> {
    let conn = open_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, expires_at, price, currency, cycle, notes, created_at FROM trials \
             ORDER BY expires_at ASC",
        )
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Trial {
                id: row.get(0)?,
                name: row.get(1)?,
                expires_at: row.get(2)?,
                price: row.get(3)?,
                currency: row.get(4)?,
                cycle: row.get(5)?,
                notes: row.get(6)?,
                created_at: row.get(7)?,
            })
        })
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| std::io::Error::other(e.to_string()))?)
    }
    Ok(out)
}

/// Trials expiring within `days` (inclusive of today; expired excluded).
pub fn get_trials_expiring_soon(days: i64) -> std::io::Result<Vec<Trial>> {
    let conn = open_db()?;
    let from = crate::date::today_string();
    let to = (crate::date::today() + chrono::Duration::days(days))
        .format("%Y-%m-%d")
        .to_string();
    let mut stmt = conn
        .prepare(
            "SELECT id, name, expires_at, price, currency, cycle, notes, created_at FROM trials \
             WHERE expires_at >= ? AND expires_at <= ? ORDER BY expires_at ASC",
        )
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let rows = stmt
        .query_map(params![from, to], |row| {
            Ok(Trial {
                id: row.get(0)?,
                name: row.get(1)?,
                expires_at: row.get(2)?,
                price: row.get(3)?,
                currency: row.get(4)?,
                cycle: row.get(5)?,
                notes: row.get(6)?,
                created_at: row.get(7)?,
            })
        })
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| std::io::Error::other(e.to_string()))?)
    }
    Ok(out)
}

/// Tag name + subscription count (port of `getTagsWithCount`).
pub fn get_tags_with_count() -> std::io::Result<Vec<(String, i64)>> {
    let conn = open_db()?;
    let sql = "SELECT tags.name, (SELECT COUNT(*) FROM subscription_tags \
               WHERE subscription_tags.tag_id = tags.id) AS count FROM tags ORDER BY name";
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| std::io::Error::other(e.to_string()))?)
    }
    Ok(out)
}

/// A price-history row joined with its subscription name.
#[derive(Clone, Debug)]
pub struct PriceChange {
    pub id: i64,
    pub subscription_id: i64,
    pub old_price: Option<f64>,
    pub new_price: f64,
    pub old_currency: Option<String>,
    pub new_currency: Option<String>,
    pub changed_at: String,
    pub name: String,
}

/// All price-history changes, newest first (port of `getAllPriceChanges`).
pub fn get_all_price_changes() -> std::io::Result<Vec<PriceChange>> {
    let conn = open_db()?;
    let sql = "SELECT ph.id, ph.subscription_id, ph.old_price, ph.new_price, \
               ph.old_currency, ph.new_currency, ph.changed_at, s.name \
               FROM price_history ph JOIN subscriptions s ON s.id = ph.subscription_id \
               ORDER BY ph.changed_at DESC, ph.id DESC";
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(PriceChange {
                id: row.get(0)?,
                subscription_id: row.get(1)?,
                old_price: row.get(2)?,
                new_price: row.get(3)?,
                old_currency: row.get(4)?,
                new_currency: row.get(5)?,
                changed_at: row.get(6)?,
                name: row.get(7)?,
            })
        })
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| std::io::Error::other(e.to_string()))?)
    }
    Ok(out)
}

/// Free-text search across name/notes/tags (port of `searchSubscriptions`).
pub fn search_subscriptions(
    query: &str,
    names: bool,
    notes: bool,
    tags: bool,
) -> std::io::Result<Vec<Subscription>> {
    let conn = open_db()?;
    let search_all = !names && !notes && !tags;
    let pattern = format!("%{}%", query);
    let mut conditions: Vec<&str> = Vec::new();
    let mut params: Vec<rusqlite::types::Value> = Vec::new();
    let add = |conditions: &mut Vec<&'static str>, params: &mut Vec<rusqlite::types::Value>, cond: &'static str| {
        conditions.push(cond);
        params.push(pattern.clone().into());
    };
    if names || search_all {
        add(&mut conditions, &mut params, "s.name LIKE ?");
    }
    if notes || search_all {
        add(&mut conditions, &mut params, "s.notes LIKE ?");
    }
    if tags || search_all {
        add(
            &mut conditions,
            &mut params,
            "s.id IN (SELECT st.subscription_id FROM subscription_tags st \
             JOIN tags t ON t.id = st.tag_id WHERE t.name LIKE ?)",
        );
    }
    let where_clause = format!("WHERE {}", conditions.join(" OR "));
    let cols = SUB_COLUMNS
        .iter()
        .map(|c| format!("s.{}", c))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT DISTINCT {} FROM subscriptions s {} ORDER BY s.name",
        cols, where_clause
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(params.iter()), |row| {
            row_to_subscription(&row)
        })
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| std::io::Error::other(e.to_string()))?)
    }
    map_tags(&conn, &mut out)?;
    Ok(out)
}
pub fn get_llm_usage_token_total(from: &str, to: &str) -> std::io::Result<(i64, i64)> {
    let conn = open_db()?;
    conn.query_row(
        "SELECT COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0) \
         FROM llm_usage WHERE date >= ? AND date <= ?",
        params![from, to],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .map_err(|e| std::io::Error::other(e.to_string()))
}

/// Cost + token totals grouped by model (ORDER BY total DESC).
#[allow(clippy::type_complexity)]
pub fn get_llm_usage_total_by_model(
    from: &str,
    to: &str,
) -> std::io::Result<Vec<(String, String, i64, i64, i64)>> {
    let conn = open_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT model, MAX(provider) AS provider, SUM(cost) AS total, \
                    SUM(input_tokens) AS inputTokens, SUM(output_tokens) AS outputTokens \
             FROM llm_usage WHERE date >= ? AND date <= ? GROUP BY model ORDER BY total DESC",
        )
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let rows = stmt
        .query_map(params![from, to], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?.round() as i64,
                row.get::<_, f64>(3)?.round() as i64,
                row.get::<_, f64>(4)?.round() as i64,
            ))
        })
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| std::io::Error::other(e.to_string()))?)
    }
    Ok(out)
}
/// A single audit log entry (mirror of types.ts `AuditEntry`).
pub struct AuditEntry {
    pub id: i64,
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<i64>,
    pub details: Option<String>,
    pub created_at: String,
}

/// Query audit log entries with optional filters (port of `getAuditLogs`).
pub fn get_audit_logs(
    action: Option<&str>,
    limit: i64,
    offset: i64,
    from: Option<&str>,
    to: Option<&str>,
) -> Vec<AuditEntry> {
    get_audit_logs_filtered(action, None, limit, offset, from, to)
}

/// Query audit log entries with optional target_id filter (port of `getAuditLogs`).
pub fn get_audit_logs_filtered(
    action: Option<&str>,
    target_id: Option<i64>,
    limit: i64,
    offset: i64,
    from: Option<&str>,
    to: Option<&str>,
) -> Vec<AuditEntry> {
    let mut conditions: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    if let Some(a) = action {
        conditions.push("action = ?".to_string());
        params.push(Box::new(a.to_string()));
    }
    if let Some(t) = target_id {
        conditions.push("target_id = ?".to_string());
        params.push(Box::new(t));
    }
    if let Some(f) = from {
        conditions.push("created_at >= ?".to_string());
        params.push(Box::new(f.to_string()));
    }
    if let Some(t) = to {
        conditions.push("created_at <= ?".to_string());
        params.push(Box::new(format!("{}T23:59:59", t)));
    }
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let conn = match open_db() {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let sql = format!(
        "SELECT id, action, target_type AS targetType, target_id AS targetId, \
                details, created_at AS createdAt \
         FROM audit_log {} ORDER BY id DESC LIMIT ? OFFSET ?",
        where_clause
    );
    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let mut bound: Vec<&dyn rusqlite::types::ToSql> = Vec::new();
    for p in &params {
        bound.push(p.as_ref());
    }
    let limit_box: i64 = limit;
    let offset_box: i64 = offset;
    bound.push(&limit_box);
    bound.push(&offset_box);
    let rows = match stmt.query_map(bound.as_slice(), |row| {
        Ok(AuditEntry {
            id: row.get(0)?,
            action: row.get(1)?,
            target_type: row.get(2)?,
            target_id: row.get(3)?,
            details: row.get(4)?,
            created_at: row.get(5)?,
        })
    }) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for r in rows.flatten() {
        out.push(r)
    }
    out
}

/// Count of audit log entries with optional filters (port of `getAuditLogCount`).
pub fn get_audit_log_count(action: Option<&str>, from: Option<&str>, to: Option<&str>) -> i64 {
    let mut conditions: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    if let Some(a) = action {
        conditions.push("action = ?".to_string());
        params.push(Box::new(a.to_string()));
    }
    if let Some(f) = from {
        conditions.push("created_at >= ?".to_string());
        params.push(Box::new(f.to_string()));
    }
    if let Some(t) = to {
        conditions.push("created_at <= ?".to_string());
        params.push(Box::new(format!("{}T23:59:59", t)));
    }
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };
    let conn = match open_db() {
        Ok(c) => c,
        Err(_) => return 0,
    };
    let sql = format!("SELECT COUNT(*) AS count FROM audit_log {}", where_clause);
    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let mut bound: Vec<&dyn rusqlite::types::ToSql> = Vec::new();
    for p in &params {
        bound.push(p.as_ref());
    }
    match stmt.query_row(bound.as_slice(), |row| row.get::<_, i64>(0)) {
        Ok(n) => n,
        Err(_) => 0,
    }
}

/// Price history for one subscription, newest first (port of `getPriceHistory`).
pub fn get_price_history(subscription_id: i64) -> std::io::Result<Vec<PriceChange>> {
    let conn = open_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT ph.id, ph.subscription_id, ph.old_price, ph.new_price, \
                    ph.old_currency, ph.new_currency, ph.changed_at, s.name \
             FROM price_history ph JOIN subscriptions s ON s.id = ph.subscription_id \
             WHERE ph.subscription_id = ? \
             ORDER BY ph.changed_at DESC",
        )
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let rows = stmt
        .query_map(params![subscription_id], |row| {
            Ok(PriceChange {
                id: row.get(0)?,
                subscription_id: row.get(1)?,
                old_price: row.get(2)?,
                new_price: row.get(3)?,
                old_currency: row.get(4)?,
                new_currency: row.get(5)?,
                changed_at: row.get(6)?,
                name: row.get(7)?,
            })
        })
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| std::io::Error::other(e.to_string()))?)
    }
    Ok(out)
}

/// All price changes within the last `days` days (port of `getAllPriceChanges(days)`).
pub fn get_price_changes_recent(days: i64) -> std::io::Result<Vec<PriceChange>> {
    let cutoff = crate::date::today_string_days_ago(days);
    let conn = open_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT ph.id, ph.subscription_id, ph.old_price, ph.new_price, \
                    ph.old_currency, ph.new_currency, ph.changed_at, s.name \
             FROM price_history ph JOIN subscriptions s ON s.id = ph.subscription_id \
             WHERE ph.changed_at >= ? \
             ORDER BY ph.changed_at DESC",
        )
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let rows = stmt
        .query_map(params![cutoff], |row| {
            Ok(PriceChange {
                id: row.get(0)?,
                subscription_id: row.get(1)?,
                old_price: row.get(2)?,
                new_price: row.get(3)?,
                old_currency: row.get(4)?,
                new_currency: row.get(5)?,
                changed_at: row.get(6)?,
                name: row.get(7)?,
            })
        })
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| std::io::Error::other(e.to_string()))?)
    }
    Ok(out)
}

/// A row from the `suggestions` table (port of types.ts `Suggestion`).
#[derive(Clone, Debug)]
pub struct Suggestion {
    pub id: i64,
    pub name: String,
    pub price: Option<f64>,
    pub currency: Option<String>,
    pub cycle: Option<String>,
    pub vendor_name: Option<String>,
    pub vendor_url: Option<String>,
    pub plan_tier: Option<String>,
    pub payment_method: Option<String>,
    pub source: String,
    pub source_detail: Option<String>,
    pub email_subject: Option<String>,
    pub email_from: Option<String>,
    pub email_date: Option<String>,
    pub confidence: f64,
    pub status: String,
    pub matched_sub_id: Option<i64>,
    pub created_at: Option<String>,
}

fn suggestion_from_row(row: &rusqlite::Row) -> rusqlite::Result<Suggestion> {
    Ok(Suggestion {
        id: row.get(0)?,
        name: row.get(1)?,
        price: row.get(2)?,
        currency: row.get(3)?,
        cycle: row.get(4)?,
        vendor_name: row.get(5)?,
        vendor_url: row.get(6)?,
        plan_tier: row.get(7)?,
        payment_method: row.get(8)?,
        source: row.get(9)?,
        source_detail: row.get(10)?,
        email_subject: row.get(11)?,
        email_from: row.get(12)?,
        email_date: row.get(13)?,
        confidence: row.get(14)?,
        status: row.get(15)?,
        matched_sub_id: row.get(16)?,
        created_at: row.get(17)?,
    })
}

const SUGGESTION_SELECT: &str = "SELECT id, name, price, currency, cycle, \
     vendor_name, vendor_url, plan_tier, payment_method, source, source_detail, \
     email_subject, email_from, email_date, confidence, status, matched_sub_id, created_at \
     FROM suggestions";

/// List suggestions, optionally filtered by status (newest first).
pub fn get_suggestions(status: Option<&str>) -> std::io::Result<Vec<Suggestion>> {
    let conn = open_db()?;
    let (sql, out) = match status {
        Some(s) => {
            let mut stmt = conn
                .prepare(&format!("{} WHERE status = ? ORDER BY created_at DESC", SUGGESTION_SELECT))
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            let rows = stmt
                .query_map(params![s], suggestion_from_row)
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r.map_err(|e| std::io::Error::other(e.to_string()))?)
            }
            (String::new(), out)
        }
        None => {
            let mut stmt = conn
                .prepare(&format!("{} ORDER BY created_at DESC", SUGGESTION_SELECT))
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            let rows = stmt
                .query_map([], suggestion_from_row)
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r.map_err(|e| std::io::Error::other(e.to_string()))?)
            }
            (String::new(), out)
        }
    };
    let _ = sql;
    Ok(out)
}

/// Fetch a single suggestion by id.
pub fn get_suggestion(id: i64) -> std::io::Result<Option<Suggestion>> {
    let conn = open_db()?;
    let mut stmt = conn
        .prepare(&format!("{} WHERE id = ?", SUGGESTION_SELECT))
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut rows = stmt
        .query_map(params![id], suggestion_from_row)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    match rows.next() {
        Some(r) => Ok(Some(r.map_err(|e| std::io::Error::other(e.to_string()))?)),
        None => Ok(None),
    }
}

/// Fetch a single subscription by id.
pub fn get_subscription_by_id(id: i64) -> std::io::Result<Option<Subscription>> {
    let conn = open_db()?;
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {} FROM subscriptions WHERE id = ? LIMIT 1",
            SUB_COLUMNS.join(", ")
        ))
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut rows = stmt
        .query_map(params![id], row_to_subscription)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    match rows.next() {
        Some(r) => {
            let mut sub = r.map_err(|e| std::io::Error::other(e.to_string()))?;
            map_tags(&conn, std::slice::from_mut(&mut sub))
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            Ok(Some(sub))
        }
        None => Ok(None),
    }
}

// ── Single-row lookup / mutation helpers (error-path phases) ─────────────

/// Fetch a single trial by id (port of `trialRepository.get`).
pub fn get_trial(id: i64) -> std::io::Result<Option<Trial>> {
    let conn = open_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, expires_at, price, currency, cycle, notes, created_at FROM trials \
             WHERE id = ? LIMIT 1",
        )
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut rows = stmt
        .query_map(params![id], |row| {
            Ok(Trial {
                id: row.get(0)?,
                name: row.get(1)?,
                expires_at: row.get(2)?,
                price: row.get(3)?,
                currency: row.get(4)?,
                cycle: row.get(5)?,
                notes: row.get(6)?,
                created_at: row.get(7)?,
            })
        })
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    match rows.next() {
        Some(r) => Ok(Some(r.map_err(|e| std::io::Error::other(e.to_string()))?)),
        None => Ok(None),
    }
}

/// Insert a trial (port of the persistence half of `handleTrialAdd`).
#[allow(clippy::too_many_arguments)]
pub fn insert_trial(
    conn: &Connection,
    name: &str,
    expires_at: &str,
    price: Option<f64>,
    currency: Option<&str>,
    cycle: Option<&str>,
    notes: Option<&str>,
    created_at: &str,
    persist: bool,
) -> std::io::Result<bool> {
    let r = conn.execute(
        "INSERT INTO trials (name, expires_at, price, currency, cycle, notes, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
        params![name, expires_at, price, currency, cycle, notes, created_at],
    );
    match r {
        Ok(_) => {
            if persist {
                save_db(conn)?;
            }
            Ok(true)
        }
        Err(e) => Err(std::io::Error::other(e.to_string())),
    }
}

/// Delete a trial by id; returns false when no row matched.
pub fn remove_trial(id: i64) -> std::io::Result<bool> {
    let conn = open_db()?;
    let n = conn
        .execute("DELETE FROM trials WHERE id = ?", params![id])
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(n > 0)
}

/// Dismiss a pending suggestion; returns false when no pending row matched.
/// Dismiss all pending suggestions (dismissAllSuggestions).
pub fn dismiss_all_suggestions() -> std::io::Result<usize> {
    let conn = open_db()?;
    let n = conn
        .execute(
            "UPDATE suggestions SET status = 'dismissed' WHERE status = 'pending'",
            [],
        )
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(n)
}

pub fn dismiss_suggestion(id: i64) -> std::io::Result<bool> {
    let conn = open_db()?;
    let n = conn
        .execute(
            "UPDATE suggestions SET status = 'dismissed' WHERE id = ? AND status = 'pending'",
            params![id],
        )
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(n > 0)
}

/// Mark a suggestion as added and link it to a subscription.
/// Port of `markSuggestionAsAdded`.
pub fn mark_suggestion_added(
    conn: &Connection,
    suggestion_id: i64,
    subscription_id: i64,
) -> std::io::Result<bool> {
    let n = conn
        .execute(
            "UPDATE suggestions SET status = 'added', matched_sub_id = ? \
             WHERE id = ? AND status = 'pending'",
            params![subscription_id, suggestion_id],
        )
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(n > 0)
}

/// A suggestion row to insert (port of the fields read by `writeSuggestion`).
pub struct NewSuggestion {
    pub name: String,
    pub price: Option<f64>,
    pub currency: Option<String>,
    pub cycle: Option<String>,
    pub source: String,
    pub source_detail: Option<String>,
    pub confidence: f64,
}

/// Batch-insert suggestions, deduping by name+price+source (case-insensitive).
/// Port of `writeSuggestionBatch`; returns the number of rows inserted.
pub fn write_suggestion_batch(
    conn: &Connection,
    items: &[NewSuggestion],
    persist: bool,
) -> std::io::Result<usize> {
    conn.execute_batch("BEGIN TRANSACTION")
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let result = (|| -> rusqlite::Result<usize> {
        let mut inserted = 0usize;
        for it in items {
            let existing: Option<i64> = conn
                .query_row(
                    "SELECT id FROM suggestions \
                     WHERE LOWER(name) = LOWER(?) AND price IS ? AND source = ? \
                     AND status = 'pending' LIMIT 1",
                    params![it.name, it.price, it.source],
                    |r| r.get(0),
                )
                .ok();
            if existing.is_some() {
                continue;
            }
            conn.execute(
                "INSERT INTO suggestions (\
                    name, price, currency, cycle, vendor_name, vendor_url, plan_tier, \
                    payment_method, source, source_detail, email_subject, email_from, \
                    email_date, confidence\
                 ) VALUES (?, ?, ?, ?, NULL, NULL, NULL, NULL, ?, ?, NULL, NULL, NULL, ?)",
                params![
                    it.name,
                    it.price,
                    it.currency,
                    it.cycle,
                    it.source,
                    it.source_detail,
                    it.confidence
                ],
            )?;
            inserted += 1;
        }
        Ok(inserted)
    })();
    match result {
        Ok(n) => {
            conn.execute_batch("COMMIT")
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            if persist && n > 0 {
                save_db(conn)?;
            }
            Ok(n)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(std::io::Error::other(e.to_string()))
        }
    }
}

/// Rename a tag (port of `renameTag`); returns false when the old tag is absent.
pub fn rename_tag(old_name: &str, new_name: &str) -> std::io::Result<bool> {
    if old_name == new_name {
        return Ok(true);
    }
    let conn = open_db()?;
    let old_id: Option<i64> = conn
        .query_row("SELECT id FROM tags WHERE name = ?", params![old_name], |r| {
            r.get(0)
        })
        .ok();
    let Some(old_id) = old_id else {
        return Ok(false);
    };
    let existing: Option<i64> = conn
        .query_row("SELECT id FROM tags WHERE name = ?", params![new_name], |r| {
            r.get(0)
        })
        .ok();
    conn.execute_batch("BEGIN TRANSACTION")
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let result = (|| -> rusqlite::Result<()> {
        if let Some(dest_id) = existing {
            conn.execute(
                "UPDATE OR IGNORE subscription_tags SET tag_id = ? WHERE tag_id = ?",
                params![dest_id, old_id],
            )?;
            conn.execute(
                "DELETE FROM subscription_tags WHERE tag_id = ?",
                params![old_id],
            )?;
            conn.execute("DELETE FROM tags WHERE id = ?", params![old_id])?;
        } else {
            conn.execute(
                "UPDATE tags SET name = ? WHERE id = ?",
                params![new_name, old_id],
            )?;
        }
        Ok(())
    })();
    match result {
        Ok(()) => {
            conn.execute_batch("COMMIT")
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            Ok(true)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(std::io::Error::other(e.to_string()))
        }
    }
}

/// Delete a tag and its links. Returns whether the tag existed.
pub fn remove_tag(conn: &Connection, name: &str, persist: bool) -> std::io::Result<bool> {
    let tag_id: Option<i64> = conn
        .query_row("SELECT id FROM tags WHERE name = ?", params![name], |r| r.get(0))
        .ok();
    let Some(tag_id) = tag_id else { return Ok(false) };
    conn.execute("DELETE FROM subscription_tags WHERE tag_id = ?", params![tag_id])
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    conn.execute("DELETE FROM tags WHERE id = ?", params![tag_id])
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    if persist {
        save_db(conn)?;
    }
    Ok(true)
}

/// Remove tags that have no subscription links. Returns the number removed.
pub fn prune_tags(conn: &Connection, persist: bool) -> std::io::Result<usize> {
    let n = conn
        .execute(
            "DELETE FROM tags WHERE id NOT IN (SELECT DISTINCT tag_id FROM subscription_tags)",
            [],
        )
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    if n > 0 && persist {
        save_db(conn)?;
    }
    Ok(n)
}

/// Path of the encrypted database file (`getDbPath`).
pub fn db_path() -> std::path::PathBuf {
    crate::crypto::config_dir().join("subtrack.db")
}

/// Size in bytes of the encrypted database file (`getFileSize`).
pub fn db_file_size() -> i64 {
    std::fs::metadata(db_path()).map(|m| m.len() as i64).unwrap_or(0)
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

/// Count of audit log entries with `created_at <= before + "T23:59:59"`
/// (port of `auditRepository.count({ to: before })`).
pub fn count_audit_before(before: &str) -> i64 {
    let conn = match open_db() {
        Ok(c) => c,
        Err(_) => return 0,
    };
    conn.query_row(
        "SELECT COUNT(*) FROM audit_log WHERE created_at <= ?",
        params![format!("{}T23:59:59", before)],
        |row| row.get::<_, i64>(0),
    )
    .unwrap_or(0)
}

/// Delete audit log entries older than `before`; returns count deleted.
pub fn prune_audit_logs(conn: &Connection, before: &str, persist: bool) -> std::io::Result<usize> {
    let n = conn
        .execute("DELETE FROM audit_log WHERE created_at < ?", params![before])
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    if n > 0 && persist {
        save_db(conn)?;
    }
    Ok(n)
}

/// Fetch a single LLM usage row by id.
pub fn get_llm_usage_by_id(id: i64) -> std::io::Result<Option<LlmUsage>> {
    let conn = open_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, provider, model, input_tokens, output_tokens, cost, date, description \
             FROM llm_usage WHERE id = ? LIMIT 1",
        )
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut rows = stmt
        .query_map(params![id], |row| {
            Ok(LlmUsage {
                id: row.get(0)?,
                provider: row.get(1)?,
                model: row.get(2)?,
                input_tokens: row.get(3)?,
                output_tokens: row.get(4)?,
                cost: row.get::<_, f64>(5)?,
                date: row.get(6)?,
                description: row.get(7)?,
            })
        })
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    match rows.next() {
        Some(r) => Ok(Some(r.map_err(|e| std::io::Error::other(e.to_string()))?)),
        None => Ok(None),
    }
}

/// Delete an LLM usage row by id; returns false when no row matched.
pub fn remove_llm_usage(id: i64) -> std::io::Result<bool> {
    let conn = open_db()?;
    let n = conn
        .execute("DELETE FROM llm_usage WHERE id = ?", params![id])
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(n > 0)
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

/// Fields for a new subscription row.
#[derive(Default, Clone)]
pub struct NewSubscription {
    pub name: String,
    pub price: f64,
    pub currency: String,
    pub cycle: String,
    pub status: String,
    pub billing_day: Option<i64>,
    pub notes: Option<String>,
    pub payment_method: Option<String>,
    pub contract_start: Option<String>,
    pub contract_end: Option<String>,
    pub auto_renewal: bool,
    pub vendor_name: Option<String>,
    pub vendor_url: Option<String>,
    pub plan_tier: Option<String>,
    pub discount_amount: Option<f64>,
    pub discount_type: Option<String>,
    pub tags: Vec<String>,
}

/// Port of `subscriptionRepository.findByName`; returns the matching id.
pub fn find_subscription_by_name(name: &str) -> std::io::Result<Option<i64>> {
    let conn = open_db()?;
    let mut stmt = conn
        .prepare("SELECT id FROM subscriptions WHERE name = ? LIMIT 1")
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut rows = stmt
        .query_map(params![name], |row| row.get::<_, i64>(0))
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    match rows.next() {
        Some(r) => Ok(Some(r.map_err(|e| std::io::Error::other(e.to_string()))?)),
        None => Ok(None),
    }
}

/// Insert a subscription (+ tags) and optionally persist. Port of `writeSubscription`.
pub fn write_subscription(
    conn: &Connection,
    s: &NewSubscription,
    persist: bool,
) -> std::io::Result<i64> {
    conn.execute_batch("BEGIN TRANSACTION")
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let result = (|| -> rusqlite::Result<i64> {
        conn.execute(
            "INSERT INTO subscriptions (
                name, price, currency, cycle, status, billing_day, created_at, notes,
                payment_method, contract_start, contract_end, auto_renewal,
                vendor_name, vendor_url, plan_tier, discount_amount, discount_type
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                s.name,
                s.price,
                s.currency,
                s.cycle,
                if s.status.is_empty() { "active" } else { &s.status },
                s.billing_day,
                crate::date::today_string(),
                s.notes,
                s.payment_method,
                s.contract_start,
                s.contract_end,
                if s.auto_renewal { 1 } else { 0 },
                s.vendor_name,
                s.vendor_url,
                s.plan_tier,
                s.discount_amount,
                s.discount_type,
            ],
        )?;
        let id: i64 = conn.query_row("SELECT last_insert_rowid()", [], |r| r.get(0))?;

        let mut seen: Vec<&String> = Vec::new();
        for tag in &s.tags {
            if seen.contains(&tag) {
                continue;
            }
            seen.push(tag);
            conn.execute("INSERT OR IGNORE INTO tags (name) VALUES (?)", params![tag])?;
            let tag_id: Option<i64> = conn
                .query_row("SELECT id FROM tags WHERE name = ?", params![tag], |r| r.get(0))
                .ok();
            if let Some(tag_id) = tag_id {
                conn.execute(
                    "INSERT INTO subscription_tags (subscription_id, tag_id) VALUES (?, ?)",
                    params![id, tag_id],
                )?;
            }
        }
        Ok(id)
    })();
    match result {
        Ok(id) => {
            conn.execute_batch("COMMIT")
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            if persist {
                save_db(conn)?;
            }
            Ok(id)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(std::io::Error::other(e.to_string()))
        }
    }
}

/// Append an audit entry. Port of `addAuditLog`.
pub fn add_audit_log(
    conn: &Connection,
    action: &str,
    target_type: Option<&str>,
    target_id: Option<i64>,
    details: Option<&str>,
    persist: bool,
) -> std::io::Result<bool> {
    let r = conn.execute(
        "INSERT INTO audit_log (action, target_type, target_id, details) VALUES (?, ?, ?, ?)",
        params![action, target_type, target_id, details],
    );
    match r {
        Ok(_) => {
            if persist {
                save_db(conn)?;
            }
            Ok(true)
        }
        Err(_) => Ok(false),
    }
}

/// Update fields on a subscription. Port of `updateSubscription` (subset).
pub fn update_subscription(
    conn: &Connection,
    id: i64,
    sets: &[(&str, SqlValue)],
    persist: bool,
) -> std::io::Result<bool> {
    let exists: bool = conn
        .query_row("SELECT 1 FROM subscriptions WHERE id = ?", params![id], |_| Ok(true))
        .unwrap_or(false);
    if !exists {
        return Ok(false);
    }
    if !sets.is_empty() {
        let clause = sets
            .iter()
            .map(|(c, _)| format!("{} = ?", c))
            .collect::<Vec<_>>()
            .join(", ");
        let mut params_vec: Vec<rusqlite::types::Value> = sets.iter().map(|(_, v)| v.to_sql()).collect();
        params_vec.push(rusqlite::types::Value::Integer(id));
        conn.execute(
            &format!("UPDATE subscriptions SET {} WHERE id = ?", clause),
            rusqlite::params_from_iter(params_vec),
        )
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    }
    if persist {
        save_db(conn)?;
    }
    Ok(true)
}

/// Replace a subscription's tags (port of the `fields.tags` branch of
/// `updateSubscription`).
pub fn set_subscription_tags(
    conn: &Connection,
    id: i64,
    tags: &[String],
    persist: bool,
) -> std::io::Result<()> {
    conn.execute(
        "DELETE FROM subscription_tags WHERE subscription_id = ?",
        params![id],
    )
    .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut seen: Vec<&String> = Vec::new();
    for tag in tags {
        if seen.contains(&tag) {
            continue;
        }
        seen.push(tag);
        conn.execute("INSERT OR IGNORE INTO tags (name) VALUES (?)", params![tag])
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let tag_id: Option<i64> = conn
            .query_row("SELECT id FROM tags WHERE name = ?", params![tag], |r| r.get(0))
            .ok();
        if let Some(tag_id) = tag_id {
            conn.execute(
                "INSERT INTO subscription_tags (subscription_id, tag_id) VALUES (?, ?)",
                params![id, tag_id],
            )
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        }
    }
    if persist {
        save_db(conn)?;
    }
    Ok(())
}

/// Delete a subscription by id. Port of `subscriptionRepository.remove`.
pub fn remove_subscription(conn: &Connection, id: i64, persist: bool) -> std::io::Result<bool> {    let n = conn
        .execute("DELETE FROM subscriptions WHERE id = ?", params![id])
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    if n > 0 && persist {
        save_db(conn)?;
    }
    Ok(n > 0)
}

/// Unarchive a subscription by id. Port of `unarchiveSubscription`.
pub fn unarchive_subscription(conn: &Connection, id: i64, persist: bool) -> std::io::Result<bool> {
    let n = conn
        .execute(
            "UPDATE subscriptions SET status = 'active' WHERE id = ? AND status = 'archived'",
            params![id],
        )
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    if n > 0 && persist {
        save_db(conn)?;
    }
    Ok(n > 0)
}

/// A minimal SQL value for dynamic updates.
#[derive(Clone, Debug)]
pub enum SqlValue {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
}

impl SqlValue {
    fn to_sql(&self) -> rusqlite::types::Value {
        match self {
            SqlValue::Null => rusqlite::types::Value::Null,
            SqlValue::Int(i) => rusqlite::types::Value::Integer(*i),
            SqlValue::Real(f) => rusqlite::types::Value::Real(*f),
            SqlValue::Text(s) => rusqlite::types::Value::Text(s.clone()),
        }
    }
}

/// Archive a subscription by id. Port of `archiveSubscription`.
pub fn archive_subscription(conn: &Connection, id: i64, persist: bool) -> std::io::Result<bool> {
    let n = conn
        .execute(
            "UPDATE subscriptions SET status = 'archived' WHERE id = ? AND status != 'archived'",
            params![id],
        )
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    if n > 0 && persist {
        save_db(conn)?;
    }
    Ok(n > 0)
}

/// Record a price-history row. Port of `writePriceHistory`.
pub fn write_price_history(
    conn: &Connection,
    subscription_id: i64,
    old_price: f64,
    new_price: f64,
    old_currency: &str,
    new_currency: &str,
) -> std::io::Result<()> {
    conn.execute(
        "INSERT INTO price_history (subscription_id, old_price, new_price, old_currency, new_currency)
         VALUES (?, ?, ?, ?, ?)",
        params![subscription_id, old_price, new_price, old_currency, new_currency],
    )
    .map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(())
}

/// Replace a subscription's tags. Port of the `fields.tags` branch of
/// `updateSubscription`.
pub fn replace_tags(conn: &Connection, id: i64, tags: &[String]) -> std::io::Result<()> {
    conn.execute(
        "DELETE FROM subscription_tags WHERE subscription_id = ?",
        params![id],
    )
    .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut seen: Vec<&String> = Vec::new();
    for tag in tags {
        if seen.contains(&tag) {
            continue;
        }
        seen.push(tag);
        conn.execute("INSERT OR IGNORE INTO tags (name) VALUES (?)", params![tag])
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let tag_id: Option<i64> = conn
            .query_row("SELECT id FROM tags WHERE name = ?", params![tag], |r| r.get(0))
            .ok();
        if let Some(tag_id) = tag_id {
            conn.execute(
                "INSERT INTO subscription_tags (subscription_id, tag_id) VALUES (?, ?)",
                params![id, tag_id],
            )
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        }
    }
    Ok(())
}

/// Insert an LLM usage row. Port of `addLlmUsage`.
pub fn insert_llm_usage(
    conn: &Connection,
    provider: &str,
    model: &str,
    input_tokens: i64,
    output_tokens: i64,
    cost: i64,
    date: &str,
    description: Option<&str>,
    persist: bool,
) -> std::io::Result<()> {
    conn.execute(
        "INSERT INTO llm_usage (provider, model, input_tokens, output_tokens, cost, date, description)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
        params![provider, model, input_tokens, output_tokens, cost, date, description],
    )
    .map_err(|e| std::io::Error::other(e.to_string()))?;
    if persist {
        save_db(conn)?;
    }
    Ok(())
}

/// Port of `addLlmUsageFromLog` — insert with fractional-cent cost and dedup by
/// `generation_id`. Returns `true` when a row was inserted, `false` for a
/// duplicate.
pub fn add_llm_usage_from_log(
    conn: &Connection,
    provider: &str,
    model: &str,
    input_tokens: i64,
    output_tokens: i64,
    cost: f64,
    date: &str,
    description: Option<&str>,
    generation_id: &str,
    persist: bool,
) -> std::io::Result<bool> {
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM llm_usage WHERE generation_id = ? LIMIT 1",
            params![generation_id],
            |row| row.get(0),
        )
        .ok();
    if existing.is_some() {
        return Ok(false);
    }
    conn.execute(
        "INSERT INTO llm_usage (provider, model, input_tokens, output_tokens, cost, date, description, generation_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        params![provider, model, input_tokens, output_tokens, cost, date, description, generation_id],
    )
    .map_err(|e| std::io::Error::other(e.to_string()))?;
    if persist {
        save_db(conn)?;
    }
    Ok(true)
}
