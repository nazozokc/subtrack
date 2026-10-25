use rusqlite::{Connection, params};

use super::{open_db, save_db};

pub fn get_tags_with_count() -> std::io::Result<Vec<(String, i64)>> {
    let conn = open_db()?;
    let sql = "SELECT tags.name, (SELECT COUNT(*) FROM subscription_tags \
               WHERE subscription_tags.tag_id = tags.id) AS count FROM tags ORDER BY name";
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                subtrack_core::validation::sanitize_text(&row.get::<_, String>(0)?),
                row.get::<_, i64>(1)?,
            ))
        })
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| std::io::Error::other(e.to_string()))?)
    }
    Ok(out)
}

/// A price-history row joined with its subscription name.
pub fn rename_tag(old_name: &str, new_name: &str) -> std::io::Result<bool> {
    let old_name = subtrack_core::validation::sanitize_text(old_name);
    let new_name = subtrack_core::validation::sanitize_text(new_name);
    if new_name.trim().is_empty() {
        return Err(std::io::Error::other("Tag name cannot be empty"));
    }
    if old_name == new_name {
        return Ok(true);
    }
    let conn = open_db()?;
    let old_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM tags WHERE name = ?",
            params![&old_name],
            |r| r.get(0),
        )
        .ok();
    let Some(old_id) = old_id else {
        return Ok(false);
    };
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM tags WHERE name = ?",
            params![&new_name],
            |r| r.get(0),
        )
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
                params![&new_name, old_id],
            )?;
        }
        Ok(())
    })();
    match result {
        Ok(()) => {
            conn.execute_batch("COMMIT")
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            save_db(&conn)?;
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
        .query_row("SELECT id FROM tags WHERE name = ?", params![name], |r| {
            r.get(0)
        })
        .ok();
    let Some(tag_id) = tag_id else {
        return Ok(false);
    };
    conn.execute(
        "DELETE FROM subscription_tags WHERE tag_id = ?",
        params![tag_id],
    )
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
