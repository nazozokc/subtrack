import type { SQLInputValue } from "node:sqlite"
import { getDb, execObjs, execObj, saveDb } from "./connection.ts"
import type { SharedArgs, AddSharedArgs } from "../types.ts"

const SORT_FIELDS = ["id", "name", "price", "currency", "cycle", "status"] as const

import type { SubscriptionQueryOptions } from "../types.ts"
export type { SubscriptionQueryOptions } from "../types.ts"

/** Column projection shared by all subscription queries. */
export const SUB_COLUMNS = `
  id, name, price, currency, cycle, status,
  billing_day AS billingDay, created_at AS createdAt, notes, payment_method AS paymentMethod,
  contract_start AS contractStart, contract_end AS contractEnd, auto_renewal AS autoRenewal,
  vendor_name AS vendorName, vendor_url AS vendorUrl, plan_tier AS planTier,
  discount_amount AS discountAmount, discount_type AS discountType
`.trim()

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
    // auto_renewal is stored as 0/1 in SQLite; normalize to boolean
    sub.autoRenewal = !!sub.autoRenewal
  }

  return subs
}

export const getSubscriptions = (
  options?: SubscriptionQueryOptions,
): SharedArgs[] => {
  const db = getDb()
  const field = options?.sort && (SORT_FIELDS as readonly string[]).includes(options.sort) ? options.sort : "id"
  const order = options?.desc ? "DESC" : "ASC"

  const conditions: string[] = []
  if (!options?.includeArchived) {
    conditions.push("status != 'archived'")
  }
  if (options?.status) {
    conditions.push("status = ?")
  }
  if (options?.minPrice !== undefined) {
    conditions.push("price >= ?")
  }
  if (options?.maxPrice !== undefined) {
    conditions.push("price <= ?")
  }

  const where = conditions.length > 0 ? `WHERE ${conditions.join(" AND ")}` : ""
  let limitClause = ""
  let offsetClause = ""
  const params: SQLInputValue[] = []

  if (options?.status) {
    params.push(options.status)
  }
  if (options?.minPrice !== undefined) {
    params.push(options.minPrice)
  }
  if (options?.maxPrice !== undefined) {
    params.push(options.maxPrice)
  }
  if (options?.limit !== undefined) {
    limitClause = " LIMIT ?"
    params.push(options.limit)
  }
  if (options?.offset !== undefined) {
    offsetClause = " OFFSET ?"
    params.push(options.offset)
  }

  // SQLite requires LIMIT before OFFSET — emit LIMIT -1 (unlimited) when only an offset is given
  const pagination = limitClause
    ? `${limitClause}${offsetClause}`
    : offsetClause
      ? ` LIMIT -1${offsetClause}`
      : ""

  const subs = execObjs<SharedArgs>(
    db,
    `SELECT ${SUB_COLUMNS} FROM subscriptions ${where} ORDER BY ${field} ${order}${pagination}`,
    params.length > 0 ? params : undefined,
  )
  return mapTags(subs)
}

/** All subscriptions except cancelled ones — the "payable" set used by analytics. */
export const getNonCancelledSubscriptions = (): SharedArgs[] =>
  getSubscriptions().filter((s) => s.status !== "cancelled")

export const writeSubscription = (
  data: AddSharedArgs,
  options: { persist?: boolean } = {},
): number => {
  const db = getDb()
  const uniqueTags = Array.from(new Set(data.tags))

  db.exec("BEGIN TRANSACTION")
  try {
    db.prepare(
      `INSERT INTO subscriptions (
        name, price, currency, cycle, status, billing_day, created_at, notes, payment_method,
        contract_start, contract_end, auto_renewal,
        vendor_name, vendor_url, plan_tier,
        discount_amount, discount_type
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    ).run(
      data.name, data.price, data.currency, data.cycle, data.status ?? "active",
      data.billingDay ?? null, data.createdAt ?? new Date().toISOString().split("T")[0],
      data.notes ?? null, data.paymentMethod ?? null,
      data.contractStart ?? null, data.contractEnd ?? null,
      data.autoRenewal === false ? 0 : 1,
      data.vendorName ?? null, data.vendorUrl ?? null, data.planTier ?? null,
      data.discountAmount ?? null, data.discountType ?? null,
    )

    const idRow = execObj<Record<string, number>>(
      db,
      "SELECT last_insert_rowid() AS id",
    )
    if (!idRow) throw new Error("Failed to get last insert id")
    const subscriptionId = Number(idRow.id)

    for (const t of uniqueTags) {
      db.prepare("INSERT OR IGNORE INTO tags (name) VALUES (?)").run(t)
      const tagRow = execObj<{ id: number }>(
        db,
        "SELECT id FROM tags WHERE name = ?",
        [t],
      )
      if (tagRow) {
        db.prepare(
          "INSERT INTO subscription_tags (subscription_id, tag_id) VALUES (?, ?)",
        ).run(subscriptionId, tagRow.id)
      }
    }

    db.exec("COMMIT")
    if (options.persist !== false) saveDb()
    return subscriptionId
  } catch (error) {
    try {
      db.exec("ROLLBACK")
    } catch {
      /* rollback failed, nothing to do */
    }
    throw error
  }
}

export const deleteSubscription = (id: number, options: { persist?: boolean } = {}): boolean => {
  const db = getDb()
  const { changes } = db.prepare("DELETE FROM subscriptions WHERE id = ?").run(id)
  const modified = Number(changes) > 0
  if (modified && options.persist !== false) saveDb()
  return modified
}

export const getSubscription = (id: number): SharedArgs | undefined => {
  const db = getDb()
  const sub = execObj<SharedArgs>(
    db,
    `SELECT ${SUB_COLUMNS} FROM subscriptions WHERE id = ?`,
    [id],
  )
  if (!sub) return undefined
  return mapTags([sub])[0]
}

export const updateSubscription = (
  id: number,
  fields: Partial<AddSharedArgs>,
  options: { persist?: boolean } = {},
): boolean => {
  const db = getDb()

  db.exec("BEGIN TRANSACTION")
  try {
    const existing = db.prepare("SELECT 1 AS found FROM subscriptions WHERE id = ?").get(id)
    if (!existing) {
      db.exec("ROLLBACK")
      return false
    }

    const sets: string[] = []
    const params: SQLInputValue[] = []

    if (fields.name !== undefined) { sets.push("name = ?"); params.push(fields.name) }
    if (fields.price !== undefined) { sets.push("price = ?"); params.push(fields.price) }
    if (fields.currency !== undefined) { sets.push("currency = ?"); params.push(fields.currency) }
    if (fields.cycle !== undefined) { sets.push("cycle = ?"); params.push(fields.cycle) }
    if (fields.status !== undefined) { sets.push("status = ?"); params.push(fields.status) }
    if (fields.billingDay !== undefined) { sets.push("billing_day = ?"); params.push(fields.billingDay) }
    if (fields.notes !== undefined) { sets.push("notes = ?"); params.push(fields.notes || null) }
    if (fields.paymentMethod !== undefined) { sets.push("payment_method = ?"); params.push(fields.paymentMethod || null) }
    if (fields.contractStart !== undefined) { sets.push("contract_start = ?"); params.push(fields.contractStart || null) }
    if (fields.contractEnd !== undefined) { sets.push("contract_end = ?"); params.push(fields.contractEnd || null) }
    if (fields.autoRenewal !== undefined) { sets.push("auto_renewal = ?"); params.push(fields.autoRenewal ? 1 : 0) }
    if (fields.vendorName !== undefined) { sets.push("vendor_name = ?"); params.push(fields.vendorName || null) }
    if (fields.vendorUrl !== undefined) { sets.push("vendor_url = ?"); params.push(fields.vendorUrl || null) }
    if (fields.planTier !== undefined) { sets.push("plan_tier = ?"); params.push(fields.planTier || null) }
    if (fields.discountAmount !== undefined) { sets.push("discount_amount = ?"); params.push(fields.discountAmount ?? null) }
    if (fields.discountType !== undefined) { sets.push("discount_type = ?"); params.push(fields.discountType || null) }

    if (sets.length > 0) {
      params.push(id)
      db.prepare(`UPDATE subscriptions SET ${sets.join(", ")} WHERE id = ?`).run(...params)
    }

    if (fields.tags !== undefined) {
      const uniqueTags = Array.from(new Set(fields.tags))
      db.prepare("DELETE FROM subscription_tags WHERE subscription_id = ?").run(id)
      for (const t of uniqueTags) {
        db.prepare("INSERT OR IGNORE INTO tags (name) VALUES (?)").run(t)
        const tagRow = execObj<{ id: number }>(
          db,
          "SELECT id FROM tags WHERE name = ?",
          [t],
        )
        if (tagRow) {
          db.prepare(
            "INSERT INTO subscription_tags (subscription_id, tag_id) VALUES (?, ?)",
          ).run(id, tagRow.id)
        }
      }
    }

    db.exec("COMMIT")
    if (options.persist !== false) saveDb()
    return true
  } catch (error) {
    try { db.exec("ROLLBACK") } catch { /* ok */ }
    throw error
  }
}

export const archiveSubscription = (id: number): boolean => {
  const db = getDb()
  const { changes } = db.prepare(
    "UPDATE subscriptions SET status = 'archived' WHERE id = ? AND status != 'archived'",
  ).run(id)
  const modified = Number(changes) > 0
  if (modified) saveDb()
  return modified
}

export const unarchiveSubscription = (id: number): boolean => {
  const db = getDb()
  const { changes } = db.prepare(
    "UPDATE subscriptions SET status = 'active' WHERE id = ? AND status = 'archived'",
  ).run(id)
  const modified = Number(changes) > 0
  if (modified) saveDb()
  return modified
}

/**
 * Find a subscription by exact name (case-insensitive).
 * Returns the first match if found, or undefined.
 */
export const findSubscriptionByName = (name: string): SharedArgs | undefined => {
  const db = getDb()
  const subs = execObjs<SharedArgs>(
    db,
    `SELECT ${SUB_COLUMNS} FROM subscriptions WHERE LOWER(name) = LOWER(?) LIMIT 1`,
    [name],
  )
  if (subs.length === 0) return undefined
  return mapTags(subs)[0]
}

/**
 * Merge a duplicate subscription into another (transactional).
 * Tags from the removed subscription are transferred to the kept one,
 * then the removed subscription is deleted (price_history cascades).
 */
export const mergeSubscriptions = (keepId: number, removeId: number): boolean => {
  const db = getDb()
  if (keepId === removeId) return false

  db.exec("BEGIN TRANSACTION")
  try {
    db.prepare(
      `INSERT OR IGNORE INTO subscription_tags (subscription_id, tag_id)
       SELECT ?, tag_id FROM subscription_tags WHERE subscription_id = ?`,
    ).run(keepId, removeId)
    const { changes } = db.prepare("DELETE FROM subscriptions WHERE id = ?").run(removeId)
    const modified = Number(changes) > 0
    if (!modified) {
      db.exec("ROLLBACK")
      return false
    }
    db.exec("COMMIT")
    saveDb()
    return true
  } catch (error) {
    try { db.exec("ROLLBACK") } catch { /* ok */ }
    throw error
  }
}
