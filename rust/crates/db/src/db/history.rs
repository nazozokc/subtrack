use rusqlite::params;

use super::{PriceChange, open_db};

fn price_change_from_row(row: &rusqlite::Row) -> rusqlite::Result<PriceChange> {
    use subtrack_core::validation::sanitize_text;
    let mut change = PriceChange {
        id: row.get(0)?,
        subscription_id: row.get(1)?,
        old_price: row.get(2)?,
        new_price: row.get(3)?,
        old_currency: row.get(4)?,
        new_currency: row.get(5)?,
        changed_at: row.get(6)?,
        name: row.get(7)?,
    };
    change.old_currency = change.old_currency.map(|v| sanitize_text(&v));
    change.new_currency = change.new_currency.map(|v| sanitize_text(&v));
    change.changed_at = sanitize_text(&change.changed_at);
    change.name = sanitize_text(&change.name);
    Ok(change)
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
        .query_map([], price_change_from_row)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| std::io::Error::other(e.to_string()))?)
    }
    Ok(out)
}

/// Free-text search across name/notes/tags (port of `searchSubscriptions`).
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
        .query_map(params![subscription_id], price_change_from_row)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| std::io::Error::other(e.to_string()))?)
    }
    Ok(out)
}

/// All price changes within the last `days` days (port of `getAllPriceChanges(days)`).
pub fn get_price_changes_recent(days: i64) -> std::io::Result<Vec<PriceChange>> {
    let cutoff = subtrack_core::date::today_string_days_ago(days);
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
        .query_map(params![cutoff], price_change_from_row)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| std::io::Error::other(e.to_string()))?)
    }
    Ok(out)
}
