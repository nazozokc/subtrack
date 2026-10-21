use rusqlite::{Connection, params};

use super::{open_db, save_db};

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
                .prepare(&format!(
                    "{} WHERE status = ? ORDER BY created_at DESC",
                    SUGGESTION_SELECT
                ))
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
