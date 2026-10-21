use rusqlite::{Connection, params};

use super::{open_db, save_db};

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
        .query_map(
            rusqlite::params_from_iter(params.iter()),
            row_to_subscription,
        )
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| std::io::Error::other(e.to_string()))?)
    }
    Ok(out)
}

fn map_tags(conn: &Connection, subs: &mut [Subscription]) -> std::io::Result<()> {
    if subs.is_empty() {
        return Ok(());
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
    let pairs: Vec<(i64, String)> = rows.filter_map(|r| r.ok()).collect();
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
    let add = |conditions: &mut Vec<&'static str>,
               params: &mut Vec<rusqlite::types::Value>,
               cond: &'static str| {
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
            row_to_subscription(row)
        })
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| std::io::Error::other(e.to_string()))?)
    }
    map_tags(&conn, &mut out)?;
    Ok(out)
}

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
                if s.status.is_empty() {
                    "active"
                } else {
                    &s.status
                },
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
                .query_row("SELECT id FROM tags WHERE name = ?", params![tag], |r| {
                    r.get(0)
                })
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
/// Update fields on a subscription. Port of `updateSubscription` (subset).
pub fn update_subscription(
    conn: &Connection,
    id: i64,
    sets: &[(&str, SqlValue)],
    persist: bool,
) -> std::io::Result<bool> {
    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM subscriptions WHERE id = ?",
            params![id],
            |_| Ok(true),
        )
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
        let mut params_vec: Vec<rusqlite::types::Value> =
            sets.iter().map(|(_, v)| v.to_sql()).collect();
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
            .query_row("SELECT id FROM tags WHERE name = ?", params![tag], |r| {
                r.get(0)
            })
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
pub fn remove_subscription(conn: &Connection, id: i64, persist: bool) -> std::io::Result<bool> {
    let n = conn
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
    pub(super) fn to_sql(&self) -> rusqlite::types::Value {
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
            .query_row("SELECT id FROM tags WHERE name = ?", params![tag], |r| {
                r.get(0)
            })
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
