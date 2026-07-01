import type { SqlValue } from "sql.js"
import { getDb, execObjs, execObj, saveDb } from "./connection.ts"
import type { SharedArgs, AddSharedArgs } from "../types.ts"

const SORT_FIELDS = ["id", "name", "price", "currency", "cycle", "status"] as const

export function mapTags(subs: SharedArgs[]): SharedArgs[] {
  if (subs.length === 0) return subs

  const db = getDb()
  const ids = subs.map((s) => s.id)
  const placeholders = ids.map(() => "?").join(",")
  const rows = execObjs<{ subscription_id: number; name: string }>(
    db,
    `SELECT subscription_tags.subscription_id, tags.name FROM tags
     JOIN subscription_tags ON subscription_tags.tag_id = tags.id
     WHERE subscription_tags.subscription_id IN (${placeholders})`,
    ids,
  )

  // Group tags by subscription id
  const tagMap = new Map<number, string[]>()
  for (const row of rows) {
    const list = tagMap.get(row.subscription_id)
    if (list) {
      list.push(row.name)
    } else {
      tagMap.set(row.subscription_id, [row.name])
    }
  }

  for (const sub of subs) {
    sub.tags = tagMap.get(sub.id) ?? []
  }

  return subs
}

export const getSubscriptions = (sort?: string, desc?: boolean): SharedArgs[] => {
  const db = getDb()
  const field = sort && (SORT_FIELDS as readonly string[]).includes(sort) ? sort : "id"
  const order = desc ? "DESC" : "ASC"
  const subs = execObjs<SharedArgs>(
    db,
    `SELECT id, name, price, currency, cycle, status, billing_day AS billingDay, created_at AS createdAt, notes, payment_method AS paymentMethod FROM subscriptions ORDER BY ${field} ${order}`,
  )
  return mapTags(subs)
}

export const writeSubscription = (data: AddSharedArgs): void => {
  const db = getDb()
  const uniqueTags = Array.from(new Set(data.tags))

  db.run("BEGIN TRANSACTION")
  try {
    db.run(
      "INSERT INTO subscriptions (name, price, currency, cycle, status, billing_day, created_at, notes, payment_method) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
      [data.name, data.price, data.currency, data.cycle, data.status ?? "active", data.billingDay ?? null, data.createdAt ?? new Date().toISOString().split("T")[0], data.notes ?? null, data.paymentMethod ?? null],
    )

    const idRow = execObj<Record<string, SqlValue>>(
      db,
      "SELECT last_insert_rowid() AS id",
    )
    if (!idRow) throw new Error("Failed to get last insert id")
    const subscriptionId = Number(idRow.id)

    for (const t of uniqueTags) {
      db.run("INSERT OR IGNORE INTO tags (name) VALUES (?)", [t])
      const tagRow = execObj<{ id: number }>(
        db,
        "SELECT id FROM tags WHERE name = ?",
        [t],
      )
      if (tagRow) {
        db.run(
          "INSERT INTO subscription_tags (subscription_id, tag_id) VALUES (?, ?)",
          [subscriptionId, tagRow.id],
        )
      }
    }

    db.run("COMMIT")
    saveDb()
  } catch (error) {
    try {
      db.run("ROLLBACK")
    } catch {
      /* rollback failed, nothing to do */
    }
    throw error
  }
}

export const deleteSubscription = (id: number): boolean => {
  const db = getDb()
  db.run("DELETE FROM subscriptions WHERE id = ?", [id])
  const modified = db.getRowsModified() > 0
  if (modified) saveDb()
  return modified
}

export const getSubscription = (id: number): SharedArgs | undefined => {
  const db = getDb()
  const sub = execObj<SharedArgs>(
    db,
    "SELECT id, name, price, currency, cycle, status, billing_day AS billingDay, created_at AS createdAt, notes, payment_method AS paymentMethod FROM subscriptions WHERE id = ?",
    [id],
  )
  if (!sub) return undefined
  return mapTags([sub])[0]
}

export const updateSubscription = (
  id: number,
  fields: Partial<AddSharedArgs>,
): boolean => {
  const db = getDb()

  db.run("BEGIN TRANSACTION")
  try {
    const sets: string[] = []
    const params: SqlValue[] = []

    if (fields.name !== undefined) { sets.push("name = ?"); params.push(fields.name) }
    if (fields.price !== undefined) { sets.push("price = ?"); params.push(fields.price) }
    if (fields.currency !== undefined) { sets.push("currency = ?"); params.push(fields.currency) }
    if (fields.cycle !== undefined) { sets.push("cycle = ?"); params.push(fields.cycle) }
    if (fields.status !== undefined) { sets.push("status = ?"); params.push(fields.status) }
    if (fields.billingDay !== undefined) { sets.push("billing_day = ?"); params.push(fields.billingDay) }
    if (fields.notes !== undefined) { sets.push("notes = ?"); params.push(fields.notes || null) }
    if (fields.paymentMethod !== undefined) { sets.push("payment_method = ?"); params.push(fields.paymentMethod || null) }

    if (sets.length > 0) {
      params.push(id)
      db.run(`UPDATE subscriptions SET ${sets.join(", ")} WHERE id = ?`, params)
    }

    if (fields.tags !== undefined) {
      const uniqueTags = Array.from(new Set(fields.tags))
      db.run("DELETE FROM subscription_tags WHERE subscription_id = ?", [id])
      for (const t of uniqueTags) {
        db.run("INSERT OR IGNORE INTO tags (name) VALUES (?)", [t])
        const tagRow = execObj<{ id: number }>(
          db,
          "SELECT id FROM tags WHERE name = ?",
          [t],
        )
        if (tagRow) {
          db.run(
            "INSERT INTO subscription_tags (subscription_id, tag_id) VALUES (?, ?)",
            [id, tagRow.id],
          )
        }
      }
    }

    db.run("COMMIT")
    saveDb()
    return true
  } catch (error) {
    try { db.run("ROLLBACK") } catch { /* ok */ }
    throw error
  }
}
