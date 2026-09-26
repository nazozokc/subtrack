import { getDb, execObjs, execObj, saveDb } from "./connection.ts"
import type { SharedArgs } from "../types.ts"
import { mapTags, SUB_COLUMNS } from "./subscriptions.ts"

export const getAllTags = (): string[] => {
  const db = getDb()
  const rows = execObjs<{ name: string }>(
    db,
    "SELECT name FROM tags ORDER BY name",
  )
  return rows.map((r) => r.name)
}

export const tagsSubscription = (tag: string[] | string): SharedArgs[] => {
  const db = getDb()
  const tags = Array.from(new Set(Array.isArray(tag) ? tag : [tag]))
  if (tags.length === 0) return []

  const placeholders = tags.map(() => "?").join(",")

  const rows = execObjs<{ subscription_id: number }>(
    db,
    `SELECT subscription_tags.subscription_id
     FROM subscription_tags
     JOIN tags ON tags.id = subscription_tags.tag_id
     WHERE tags.name IN (${placeholders})
     GROUP BY subscription_tags.subscription_id
     HAVING COUNT(DISTINCT tags.name) = ?`,
    [...tags, tags.length],
  )

  const ids = rows.map((r) => r.subscription_id)
  if (ids.length === 0) return []

  const idPlaceholders = ids.map(() => "?").join(",")
  const subs = execObjs<SharedArgs>(
    db,
    `SELECT ${SUB_COLUMNS} FROM subscriptions WHERE id IN (${idPlaceholders})`,
    ids,
  )

  return mapTags(subs)
}

export const getTagsWithCount = (): { name: string; count: number }[] => {
  const db = getDb()
  return execObjs<{ name: string; count: number }>(
    db,
    `SELECT tags.name, (SELECT COUNT(*) FROM subscription_tags WHERE subscription_tags.tag_id = tags.id) AS count FROM tags ORDER BY name`,
  )
}

export const renameTag = (oldName: string, newName: string): boolean => {
  const db = getDb()
  if (oldName === newName) return true
  db.exec("BEGIN TRANSACTION")
  try {
    const oldRow = execObj<{ id: number }>(
      db,
      "SELECT id FROM tags WHERE name = ?",
      [oldName],
    )
    if (!oldRow) { db.exec("ROLLBACK"); return false }

    const existingRow = execObj<{ id: number }>(
      db,
      "SELECT id FROM tags WHERE name = ?",
      [newName],
    )
    if (existingRow) {
      // Merge: point all references to the existing tag, delete old
      db.prepare(
        "UPDATE OR IGNORE subscription_tags SET tag_id = ? WHERE tag_id = ?",
      ).run(existingRow.id, oldRow.id)
      db.prepare("DELETE FROM subscription_tags WHERE tag_id = ?").run(oldRow.id)
      db.prepare("DELETE FROM tags WHERE id = ?").run(oldRow.id)
    } else {
      db.prepare("UPDATE tags SET name = ? WHERE id = ?").run(newName, oldRow.id)
    }
    db.exec("COMMIT")
    saveDb()
    return true
  } catch (error) {
    try { db.exec("ROLLBACK") } catch { /* ok */ }
    throw error
  }
}

export const deleteTag = (name: string): boolean => {
  const db = getDb()
  const { changes } = db.prepare("DELETE FROM tags WHERE name = ?").run(name)
  const modified = Number(changes) > 0
  if (modified) saveDb()
  return modified
}

export const mergeTag = (source: string, target: string): boolean => {
  const db = getDb()
  // Merging a tag into itself is a no-op, but the source must still exist —
  // otherwise the caller would report success for a tag that was never there.
  if (source === target) {
    return !!execObj<{ id: number }>(db, "SELECT id FROM tags WHERE name = ?", [source])
  }

  db.exec("BEGIN TRANSACTION")
  try {
    const srcRow = execObj<{ id: number }>(
      db,
      "SELECT id FROM tags WHERE name = ?",
      [source],
    )
    if (!srcRow) { db.exec("ROLLBACK"); return false }

    // Ensure target tag exists
    db.prepare("INSERT OR IGNORE INTO tags (name) VALUES (?)").run(target)
    const tgtRow = execObj<{ id: number }>(
      db,
      "SELECT id FROM tags WHERE name = ?",
      [target],
    )
    if (!tgtRow) { db.exec("ROLLBACK"); return false }

    // Repoint all subscription_tags from source to target
    db.prepare(
      "UPDATE OR IGNORE subscription_tags SET tag_id = ? WHERE tag_id = ?",
    ).run(tgtRow.id, srcRow.id)
    // Remove duplicate entries that pointed to both tags
    db.prepare("DELETE FROM subscription_tags WHERE tag_id = ?").run(srcRow.id)
    // Delete the source tag
    db.prepare("DELETE FROM tags WHERE id = ?").run(srcRow.id)

    db.exec("COMMIT")
    saveDb()
    return true
  } catch (error) {
    try { db.exec("ROLLBACK") } catch { /* ok */ }
    throw error
  }
}

export const pruneTags = (): number => {
  const db = getDb()
  const { changes } = db.prepare(
    "DELETE FROM tags WHERE id NOT IN (SELECT DISTINCT tag_id FROM subscription_tags)",
  ).run()
  const count = Number(changes)
  if (count > 0) saveDb()
  return count
}
