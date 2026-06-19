import initSqlJs from "sql.js"
import type { Database, SqlValue } from "sql.js"
import { mkdirSync, existsSync, readFileSync, writeFileSync } from "node:fs"
import path from "node:path"
import { homedir } from "node:os"
import type {
  Currency,
  Cycle,
  SharedArgs,
  AddSharedArgs,
  LlmUsageEntry,
  AddLlmUsageArgs,
  GetLlmUsageOptions,
} from "./types.ts"

let _db: Database | null = null
let _dbPath = ""

const _SQL = await initSqlJs()

function getDbDir(): string {
  return (
    process.env.SUBSC_CLI_DB_DIR ?? path.join(homedir(), ".config", "subtrack")
  )
}

export function saveDb(): void {
  if (!_db || !_dbPath) return
  writeFileSync(_dbPath, Buffer.from(_db.export()))
}

export function getDbPath(): string {
  getDb()
  return _dbPath
}

function makeObj(columns: string[], row: SqlValue[]): Record<string, unknown> {
  const obj: Record<string, unknown> = {}
  for (let i = 0; i < columns.length; i++) {
    obj[columns[i]] = row[i]
  }
  return obj
}

function execObjs<T>(db: Database, sql: string, params?: BindParams): T[] {
  const results = db.exec(sql, params)
  if (!results.length) return []
  const { columns, values } = results[0]
  return values.map((row) => makeObj(columns, row) as T)
}

function execObj<T>(
  db: Database,
  sql: string,
  params?: BindParams,
): T | undefined {
  const results = db.exec(sql, params)
  if (!results.length || !results[0].values.length) return undefined
  const { columns, values } = results[0]
  return makeObj(columns, values[0]) as T
}

function getDb(): Database {
  if (_db) return _db

  const dbdir = getDbDir()
  mkdirSync(dbdir, { recursive: true })
  _dbPath = path.join(dbdir, "subtrack.db")

  if (existsSync(_dbPath)) {
    const buf = readFileSync(_dbPath)
    _db = new _SQL.Database(buf)
  } else {
    _db = new _SQL.Database()
  }

  _db.run("PRAGMA foreign_keys = ON")
  _db.run(`CREATE TABLE IF NOT EXISTS subscriptions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    price INTEGER NOT NULL,
    currency TEXT NOT NULL,
    cycle TEXT NOT NULL
  )`)
  _db.run(`CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
  )`)
  _db.run(`CREATE TABLE IF NOT EXISTS subscription_tags (
    subscription_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,
    PRIMARY KEY (subscription_id, tag_id),
    FOREIGN KEY (subscription_id) REFERENCES subscriptions(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
  )`)
  _db.run(`CREATE TABLE IF NOT EXISTS llm_usage (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    cost REAL NOT NULL,
    date TEXT NOT NULL,
    description TEXT
  )`)

  return _db
}

/** Replace the DB instance for testing (e.g. with in-memory). */
export function __setDb(db: Database): void {
  _db = db
  _dbPath = ""
}

function mapTags(subs: SharedArgs[]): SharedArgs[] {
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

const SORT_FIELDS = ["id", "name", "price", "currency", "cycle"] as const

export const getSubscriptions = (sort?: string, desc?: boolean): SharedArgs[] => {
  const db = getDb()
  const field = sort && (SORT_FIELDS as readonly string[]).includes(sort) ? sort : "id"
  const order = desc ? "DESC" : "ASC"
  const subs = execObjs<SharedArgs>(
    db,
    `SELECT id, name, price, currency, cycle FROM subscriptions ORDER BY ${field} ${order}`,
  )
  return mapTags(subs)
}

export const writeSubscription = (data: AddSharedArgs): void => {
  const db = getDb()
  const uniqueTags = Array.from(new Set(data.tags))

  db.run("BEGIN TRANSACTION")
  try {
    db.run(
      "INSERT INTO subscriptions (name, price, currency, cycle) VALUES (?, ?, ?, ?)",
      [data.name, data.price, data.currency, data.cycle],
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
    `SELECT id, name, price, currency, cycle FROM subscriptions
     WHERE id IN (${idPlaceholders})`,
    ids,
  )

  return mapTags(subs)
}

export const getSubscription = (id: number): SharedArgs | undefined => {
  const db = getDb()
  const sub = execObj<SharedArgs>(
    db,
    "SELECT id, name, price, currency, cycle FROM subscriptions WHERE id = ?",
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
  db.run("BEGIN TRANSACTION")
  try {
    const oldRow = execObj<{ id: number }>(
      db,
      "SELECT id FROM tags WHERE name = ?",
      [oldName],
    )
    if (!oldRow) { db.run("ROLLBACK"); return false }

    const existingRow = execObj<{ id: number }>(
      db,
      "SELECT id FROM tags WHERE name = ?",
      [newName],
    )
    if (existingRow) {
      // Merge: point all references to the existing tag, delete old
      db.run(
        "UPDATE OR IGNORE subscription_tags SET tag_id = ? WHERE tag_id = ?",
        [existingRow.id, oldRow.id],
      )
      db.run("DELETE FROM subscription_tags WHERE tag_id = ?", [oldRow.id])
      db.run("DELETE FROM tags WHERE id = ?", [oldRow.id])
    } else {
      db.run("UPDATE tags SET name = ? WHERE id = ?", [newName, oldRow.id])
    }
    db.run("COMMIT")
    saveDb()
    return true
  } catch (error) {
    try { db.run("ROLLBACK") } catch { /* ok */ }
    throw error
  }
}

export const deleteTag = (name: string): boolean => {
  const db = getDb()
  db.run("DELETE FROM tags WHERE name = ?", [name])
  const modified = db.getRowsModified() > 0
  if (modified) saveDb()
  return modified
}

export const pruneTags = (): number => {
  const db = getDb()
  db.run(
    "DELETE FROM tags WHERE id NOT IN (SELECT DISTINCT tag_id FROM subscription_tags)",
  )
  const count = db.getRowsModified()
  if (count > 0) saveDb()
  return count
}

// ── LLM Usage ──────────────────────────────────────────────

export const addLlmUsage = (data: AddLlmUsageArgs): void => {
  const db = getDb()
  db.run(
    `INSERT INTO llm_usage (provider, model, input_tokens, output_tokens, cost, date, description)
     VALUES (?, ?, ?, ?, ?, ?, ?)`,
    [
      data.provider,
      data.model,
      data.input_tokens,
      data.output_tokens,
      data.cost,
      data.date,
      data.description,
    ],
  )
  saveDb()
}

export type GetLlmUsageOptions = {
  provider?: string
  from?: string
  to?: string
  limit?: number
  offset?: number
}

export const getLlmUsage = (options?: GetLlmUsageOptions): LlmUsageEntry[] => {
  const db = getDb()

  const conditions: string[] = []
  const params: SqlValue[] = []

  if (options?.provider) {
    conditions.push("provider = ?")
    params.push(options.provider)
  }
  if (options?.from) {
    conditions.push("date >= ?")
    params.push(options.from)
  }
  if (options?.to) {
    conditions.push("date <= ?")
    params.push(options.to)
  }

  const where = conditions.length > 0 ? `WHERE ${conditions.join(" AND ")}` : ""
  const limitClause = options?.limit ? ` LIMIT ?` : ""
  if (options?.limit) params.push(options.limit)
  const offsetClause = options?.offset ? ` OFFSET ?` : ""
  if (options?.offset) params.push(options.offset)

  return execObjs<LlmUsageEntry>(
    db,
    `SELECT id, provider, model, input_tokens, output_tokens, cost, date, description
     FROM llm_usage ${where} ORDER BY date DESC, id DESC${limitClause}${offsetClause}`,
    params,
  )
}

export const deleteLlmUsage = (id: number): boolean => {
  const db = getDb()
  db.run("DELETE FROM llm_usage WHERE id = ?", [id])
  const modified = db.getRowsModified() > 0
  if (modified) saveDb()
  return modified
}

/** Sum `cost` for all entries whose `date` falls within [from, to]. Returns USD cents. */
export const getLlmUsageTotal = (from: string, to: string): number => {
  const db = getDb()
  const row = execObj<{ total: number }>(
    db,
    "SELECT COALESCE(SUM(cost), 0) AS total FROM llm_usage WHERE date >= ? AND date <= ?",
    [from, to],
  )
  return row?.total ?? 0
}

/** Get the sum of `cost` grouped by provider for a date range. */
export const getLlmUsageTotalByProvider = (
  from: string,
  to: string,
): { provider: string; total: number }[] => {
  const db = getDb()
  return execObjs<{ provider: string; total: number }>(
    db,
    `SELECT provider, SUM(cost) AS total
     FROM llm_usage
     WHERE date >= ? AND date <= ?
     GROUP BY provider
     ORDER BY total DESC`,
    [from, to],
  )
}
