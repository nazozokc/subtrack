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
        let tmp = dir.join(format!(
            ".subtrack.open-{}.db",
            std::process::id()
        ));
        std::fs::write(&tmp, &plain)?;
        let c = Connection::open(&tmp).map_err(|e| std::io::Error::other(e.to_string()))?;
        // unlink immediately; fd stays valid on unix
        let _ = std::fs::remove_file(&tmp);
        c
    } else {
        Connection::open(&db_path).map_err(|e| std::io::Error::other(e.to_string()))?
    };
    conn.pragma_update(None, "foreign_keys", true)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(conn)
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