use rusqlite::{Connection, params};

use super::{open_db, save_db};

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
    stmt.query_row(bound.as_slice(), |row| row.get::<_, i64>(0))
        .unwrap_or_default()
}

/// Price history for one subscription, newest first (port of `getPriceHistory`).
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
        .execute(
            "DELETE FROM audit_log WHERE created_at < ?",
            params![before],
        )
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    if n > 0 && persist {
        save_db(conn)?;
    }
    Ok(n)
}

/// Delete an LLM usage row by id; returns false when no row matched.
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
