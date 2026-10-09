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
pub fn open_db() -> std::io::Result<Connection> {
    let dir = crate::crypto::config_dir();
    let db_path = dir.join("subtrack.db");
    let raw = std::fs::read(&db_path)?;
    let plain;
    let conn = if raw.len() >= 16 && !raw[..16].eq(crate::crypto::SQLITE_MAGIC)
        && raw.len() > 32
    {
        let key = crate::crypto::get_or_create_key()?;
        plain = crate::crypto::decrypt_buffer(&key, &raw)
            .ok_or_else(|| std::io::Error::other("database decryption failed"))?;
        // Stable backing file (mirrors `getOpenDbPath()`); it must exist as a
        // real path so SQLite can journal migration/write transactions.
        let tmp = dir.join(".subtrack.open.db");
        std::fs::write(&tmp, &plain)?;
        Connection::open(&tmp).map_err(|e| std::io::Error::other(e.to_string()))?
    } else {
        Connection::open(&db_path).map_err(|e| std::io::Error::other(e.to_string()))?
    };
    conn.pragma_update(None, "foreign_keys", true)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    conn.pragma_update(None, "secure_delete", true)
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
    pub cost: i64,
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
                cost: row.get::<_, f64>(5)?.round() as i64,
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
}

/// All trials, oldest expiry first (port of `getTrials`).
pub fn get_trials() -> std::io::Result<Vec<Trial>> {
    let conn = open_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, expires_at, price, currency, cycle, notes FROM trials \
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
            "SELECT id, name, expires_at, price, currency, cycle, notes FROM trials \
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