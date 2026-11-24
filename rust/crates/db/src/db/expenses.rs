//! Daily expense summaries: one row per day, line items stored as JSON.
//! Aggregates use SQLite's `json_each` so no extra item table is needed.

use rusqlite::{params, Connection};

use subtrack_core::models::{Expense, ExpenseItem, NewExpenseItem};

use super::{open_db, save_db};

fn io_err(e: rusqlite::Error) -> std::io::Error {
    std::io::Error::other(e.to_string())
}

fn sanitize(v: &str) -> String {
    subtrack_core::validation::sanitize_text(v)
}

// ── JSON encoding ───────────────────────────────────────────

fn items_to_json(items: &[ExpenseItem]) -> String {
    let values: Vec<serde_json::Value> = items
        .iter()
        .map(|item| {
            let mut map = serde_json::Map::new();
            map.insert("id".to_string(), serde_json::json!(item.id));
            map.insert("amount".to_string(), serde_json::json!(item.amount));
            map.insert("currency".to_string(), serde_json::json!(item.currency));
            map.insert("category".to_string(), serde_json::json!(item.category));
            map.insert("method".to_string(), serde_json::json!(item.method));
            map.insert(
                "note".to_string(),
                match &item.note {
                    Some(note) => serde_json::json!(note),
                    None => serde_json::Value::Null,
                },
            );
            serde_json::Value::Object(map)
        })
        .collect();
    serde_json::Value::Array(values).to_string()
}

fn parse_items(raw: &str) -> std::io::Result<Vec<ExpenseItem>> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| std::io::Error::other(e.to_string()))?;
    let array = value
        .as_array()
        .ok_or_else(|| std::io::Error::other("expense items must be a JSON array"))?;
    array
        .iter()
        .map(|v| {
            let text = |key: &str| -> String {
                v.get(key)
                    .and_then(serde_json::Value::as_str)
                    .map(sanitize)
                    .unwrap_or_default()
            };
            Ok(ExpenseItem {
                id: v.get("id").and_then(serde_json::Value::as_i64).unwrap_or(0),
                amount: v
                    .get("amount")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0),
                currency: text("currency"),
                category: text("category"),
                method: text("method"),
                note: v
                    .get("note")
                    .and_then(serde_json::Value::as_str)
                    .map(sanitize),
            })
        })
        .collect()
}

// ── Reads ───────────────────────────────────────────────────

fn rows_to_expenses(rows: Vec<(i64, String, String)>) -> std::io::Result<Vec<Expense>> {
    rows.into_iter()
        .map(|(id, date, raw)| {
            Ok(Expense {
                id,
                date: sanitize(&date),
                items: parse_items(&raw)?,
            })
        })
        .collect()
}

fn query_expenses(
    conn: &Connection,
    sql: &str,
    values: Vec<rusqlite::types::Value>,
) -> std::io::Result<Vec<Expense>> {
    let mut stmt = conn.prepare(sql).map_err(io_err)?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(values.iter()), |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .map_err(io_err)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(io_err)?);
    }
    rows_to_expenses(out)
}

/// `all = true` ignores the date window; otherwise bounds are inclusive and
/// missing bounds are open-ended.
pub fn get_expenses(
    from: Option<&str>,
    to: Option<&str>,
    all: bool,
) -> std::io::Result<Vec<Expense>> {
    let conn = open_db()?;
    if all {
        return query_expenses(
            &conn,
            "SELECT id, date, items FROM expenses ORDER BY date DESC, id DESC",
            Vec::new(),
        );
    }
    let from = from.unwrap_or("0000-01-01");
    let to = to.unwrap_or("9999-12-31");
    query_expenses(
        &conn,
        "SELECT id, date, items FROM expenses WHERE date >= ? AND date <= ? \
         ORDER BY date DESC, id DESC",
        vec![from.to_string().into(), to.to_string().into()],
    )
}

fn selector_params(selector: &str) -> Vec<rusqlite::types::Value> {
    match selector.trim().parse::<i64>() {
        Ok(id) => vec![id.into()],
        Err(_) => vec![selector.to_string().into()],
    }
}

fn selector_clause(selector: &str) -> &'static str {
    if selector.trim().parse::<i64>().is_ok() {
        "id = ?"
    } else {
        "date = ?"
    }
}

pub fn get_expense(selector: &str) -> std::io::Result<Option<Expense>> {
    let conn = open_db()?;
    let sql = format!(
        "SELECT id, date, items FROM expenses WHERE {}",
        selector_clause(selector)
    );
    let found = query_expenses(&conn, &sql, selector_params(selector))?;
    Ok(found.into_iter().next())
}

pub fn get_expense_item(
    selector: &str,
    item_id: i64,
) -> std::io::Result<Option<(Expense, ExpenseItem)>> {
    let Some(day) = get_expense(selector)? else {
        return Ok(None);
    };
    Ok(day
        .items
        .iter()
        .find(|item| item.id == item_id)
        .cloned()
        .map(|item| (day, item)))
}

/// `(group key, currency, amount)` ordered by amount descending. `group` is
/// one of `category`, `method`, `day` — the caller validates before calling.
fn groups_conn(
    conn: &Connection,
    from: &str,
    to: &str,
    group: &str,
) -> std::io::Result<Vec<(String, String, f64)>> {
    let key_expr = match group {
        "method" => "json_extract(j.value, '$.method')",
        "day" => "e.date",
        _ => "json_extract(j.value, '$.category')",
    };
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {key_expr} AS grp, json_extract(j.value, '$.currency') AS currency, \
                    SUM(json_extract(j.value, '$.amount')) AS total \
             FROM expenses e, json_each(e.items) j \
             WHERE e.date >= ? AND e.date <= ? \
             GROUP BY grp, currency ORDER BY total DESC, grp ASC"
        ))
        .map_err(io_err)?;
    let rows = stmt
        .query_map(params![from, to], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
            ))
        })
        .map_err(io_err)?;
    let mut out = Vec::new();
    for row in rows {
        let (key, currency, total) = row.map_err(io_err)?;
        out.push((sanitize(&key), sanitize(&currency), total));
    }
    Ok(out)
}

pub fn get_expense_groups(
    from: &str,
    to: &str,
    group: &str,
) -> std::io::Result<Vec<(String, String, f64)>> {
    let conn = open_db()?;
    groups_conn(&conn, from, to, group)
}

fn totals_conn(conn: &Connection, from: &str, to: &str) -> std::io::Result<Vec<(String, f64)>> {
    let mut stmt = conn
        .prepare(
            "SELECT json_extract(j.value, '$.currency') AS currency, \
                    SUM(json_extract(j.value, '$.amount')) AS total \
             FROM expenses e, json_each(e.items) j \
             WHERE e.date >= ? AND e.date <= ? \
             GROUP BY currency ORDER BY currency ASC",
        )
        .map_err(io_err)?;
    let rows = stmt
        .query_map(params![from, to], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        })
        .map_err(io_err)?;
    let mut out = Vec::new();
    for row in rows {
        let (currency, total) = row.map_err(io_err)?;
        out.push((sanitize(&currency), total));
    }
    Ok(out)
}

pub fn get_expense_totals(from: &str, to: &str) -> std::io::Result<Vec<(String, f64)>> {
    let conn = open_db()?;
    totals_conn(&conn, from, to)
}

// ── Writes ──────────────────────────────────────────────────

fn write_items(conn: &Connection, date: &str, items: &[ExpenseItem]) -> std::io::Result<()> {
    conn.execute(
        "INSERT INTO expenses (date, items, updated_at) VALUES (?, ?, datetime('now')) \
         ON CONFLICT(date) DO UPDATE SET items = excluded.items, \
         updated_at = excluded.updated_at",
        params![date, items_to_json(items)],
    )
    .map_err(io_err)?;
    Ok(())
}

fn find_day(conn: &Connection, selector: &str) -> std::io::Result<Option<Expense>> {
    let sql = format!(
        "SELECT id, date, items FROM expenses WHERE {}",
        selector_clause(selector)
    );
    let found = query_expenses(conn, &sql, selector_params(selector))?;
    Ok(found.into_iter().next())
}

fn append_items_conn(
    conn: &Connection,
    date: &str,
    new_items: &[NewExpenseItem],
) -> std::io::Result<()> {
    let mut items = find_day(conn, date)?
        .map(|day| day.items)
        .unwrap_or_default();
    let next_id = items.iter().map(|item| item.id).max().unwrap_or(0) + 1;
    for (id, new) in (next_id..).zip(new_items.iter()) {
        items.push(ExpenseItem {
            id,
            amount: new.amount,
            currency: new.currency.clone(),
            category: new.category.clone(),
            method: new.method.clone(),
            note: new.note.clone(),
        });
    }
    write_items(conn, date, &items)
}

fn delete_day_conn(conn: &Connection, selector: &str) -> std::io::Result<bool> {
    let Some(day) = find_day(conn, selector)? else {
        return Ok(false);
    };
    let changed = conn
        .execute("DELETE FROM expenses WHERE id = ?", params![day.id])
        .map_err(io_err)?;
    Ok(changed > 0)
}

fn delete_item_conn(conn: &Connection, selector: &str, item_id: i64) -> std::io::Result<bool> {
    let Some(mut day) = find_day(conn, selector)? else {
        return Ok(false);
    };
    let before = day.items.len();
    day.items.retain(|item| item.id != item_id);
    if day.items.len() == before {
        return Ok(false);
    }
    if day.items.is_empty() {
        return delete_day_conn(conn, &day.id.to_string());
    }
    write_items(conn, &day.date, &day.items)?;
    Ok(true)
}

fn update_item_conn(
    conn: &Connection,
    selector: &str,
    item_id: i64,
    updated: &ExpenseItem,
) -> std::io::Result<bool> {
    let Some(mut day) = find_day(conn, selector)? else {
        return Ok(false);
    };
    let Some(position) = day.items.iter().position(|item| item.id == item_id) else {
        return Ok(false);
    };
    day.items[position] = updated.clone();
    write_items(conn, &day.date, &day.items)?;
    Ok(true)
}

/// Run a read-modify-write in one transaction, then persist the encrypted DB.
fn mutate<T>(op: impl FnOnce(&Connection) -> std::io::Result<T>) -> std::io::Result<T> {
    let conn = open_db()?;
    conn.execute_batch("BEGIN IMMEDIATE").map_err(io_err)?;
    match op(&conn) {
        Ok(value) => {
            conn.execute_batch("COMMIT").map_err(io_err)?;
            save_db(&conn)?;
            Ok(value)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

pub fn append_expense_items(date: &str, items: &[NewExpenseItem]) -> std::io::Result<()> {
    mutate(|conn| append_items_conn(conn, date, items))
}

pub fn delete_expense_day(selector: &str) -> std::io::Result<bool> {
    mutate(|conn| delete_day_conn(conn, selector))
}

pub fn delete_expense_item(selector: &str, item_id: i64) -> std::io::Result<bool> {
    mutate(|conn| delete_item_conn(conn, selector, item_id))
}

pub fn update_expense_item(
    selector: &str,
    item_id: i64,
    updated: &ExpenseItem,
) -> std::io::Result<bool> {
    mutate(|conn| update_item_conn(conn, selector, item_id, updated))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        super::super::migrations::run_migrations(&conn).unwrap();
        conn
    }

    fn new_item(amount: f64, currency: &str, category: &str) -> NewExpenseItem {
        NewExpenseItem {
            amount,
            currency: currency.to_string(),
            category: category.to_string(),
            method: "cash".to_string(),
            note: None,
        }
    }

    #[test]
    fn items_round_trip_with_day_local_ids() {
        let conn = memory_db();
        append_items_conn(&conn, "2026-11-23", &[new_item(1200.0, "JPY", "food")]).unwrap();
        append_items_conn(&conn, "2026-11-23", &[new_item(200.0, "JPY", "transport")]).unwrap();

        let day = find_day(&conn, "2026-11-23").unwrap().unwrap();
        assert_eq!(day.items.len(), 2);
        assert_eq!(day.items[0].id, 1);
        assert_eq!(day.items[1].id, 2);
        assert_eq!(day.items[1].category, "transport");
    }

    #[test]
    fn deleting_last_item_removes_the_day() {
        let conn = memory_db();
        append_items_conn(&conn, "2026-11-23", &[new_item(1200.0, "JPY", "food")]).unwrap();
        assert!(delete_item_conn(&conn, "2026-11-23", 1).unwrap());
        assert!(find_day(&conn, "2026-11-23").unwrap().is_none());
    }

    #[test]
    fn aggregates_group_items_by_currency_and_key() {
        let conn = memory_db();
        append_items_conn(&conn, "2026-11-23", &[new_item(1200.0, "JPY", "food")]).unwrap();
        append_items_conn(&conn, "2026-11-24", &[new_item(300.0, "JPY", "food")]).unwrap();
        append_items_conn(&conn, "2026-11-24", &[new_item(5.0, "USD", "food")]).unwrap();

        let totals = totals_conn(&conn, "2026-11-01", "2026-11-30").unwrap();
        assert_eq!(
            totals,
            vec![("JPY".to_string(), 1500.0), ("USD".to_string(), 5.0)]
        );

        let groups = groups_conn(&conn, "2026-11-01", "2026-11-30", "category").unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].2, 1500.0);
    }
}
