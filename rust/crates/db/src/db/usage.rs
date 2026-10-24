use rusqlite::{Connection, params};

use super::subscriptions::to_sql;
use super::{LlmUsage, SqlValue, open_db, save_db};

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
            Ok((
                subtrack_core::validation::sanitize_text(&row.get::<_, String>(0)?),
                row.get::<_, f64>(1)?.round() as i64,
            ))
        })
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| std::io::Error::other(e.to_string()))?)
    }
    Ok(out)
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
            let mut usage = LlmUsage {
                id: row.get(0)?,
                provider: row.get(1)?,
                model: row.get(2)?,
                input_tokens: row.get(3)?,
                output_tokens: row.get(4)?,
                cost: row.get::<_, f64>(5)?,
                date: row.get(6)?,
                description: row.get(7)?,
            };
            usage.provider = subtrack_core::validation::sanitize_text(&usage.provider);
            usage.model = subtrack_core::validation::sanitize_text(&usage.model);
            usage.date = subtrack_core::validation::sanitize_text(&usage.date);
            usage.description = usage
                .description
                .map(|v| subtrack_core::validation::sanitize_text(&v));
            Ok(usage)
        })
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| std::io::Error::other(e.to_string()))?)
    }
    Ok(out)
}

/// A free-trial entry (`price` is in major units, nullable).
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
                subtrack_core::validation::sanitize_text(&row.get::<_, String>(0)?),
                subtrack_core::validation::sanitize_text(&row.get::<_, String>(1)?),
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
pub fn remove_llm_usage(id: i64) -> std::io::Result<bool> {
    let conn = open_db()?;
    let n = conn
        .execute("DELETE FROM llm_usage WHERE id = ?", params![id])
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(n > 0)
}

#[allow(clippy::too_many_arguments)]
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

/// Update an `llm_usage` row with the given column/value pairs.
pub fn update_llm_usage(
    conn: &Connection,
    id: i64,
    sets: &[(&str, SqlValue)],
    persist: bool,
) -> std::io::Result<bool> {
    let exists: bool = conn
        .query_row("SELECT 1 FROM llm_usage WHERE id = ?", params![id], |_| {
            Ok(true)
        })
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
            sets.iter().map(|(_, v)| to_sql(v)).collect();
        params_vec.push(rusqlite::types::Value::Integer(id));
        conn.execute(
            &format!("UPDATE llm_usage SET {} WHERE id = ?", clause),
            rusqlite::params_from_iter(params_vec),
        )
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    }
    if persist {
        save_db(conn)?;
    }
    Ok(true)
}

/// Port of `addLlmUsageFromLog` — insert with fractional-cent cost and dedup by
/// `generation_id`. Returns `true` when a row was inserted, `false` for a
/// duplicate.
///
/// Same port-fidelity rationale as `insert_llm_usage` for the long signature.
#[allow(clippy::too_many_arguments)]
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
