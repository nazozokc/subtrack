use rusqlite::{Connection, params};

use super::{open_db, save_db};

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
