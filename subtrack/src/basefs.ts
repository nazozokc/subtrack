import initSqlJs from "sql.js"
import type { Database, SqlValue, BindParams } from "sql.js"
import { mkdirSync, existsSync, readFileSync, writeFileSync } from "node:fs"
import path from "node:path"
import { homedir } from "node:os"
import { consola } from "consola"

export type Currency =
  | "JPY" | "USD" | "EUR" | "GBP"
  | "AUD" | "CAD" | "KRW" | "CNY"
  | "SGD" | "HKD"

export type Cycle =
  | "weekly" | "bi-weekly" | "monthly"
  | "quarterly" | "semi-annual" | "yearly"

export type SharedArgs = {
  id: number
  name: string
  price: number
  currency: Currency
  cycle: Cycle
  tags: string[]
}

export type AddSharedArgs = Omit<SharedArgs, "id">

let _db: Database | null = null
let _dbPath = ""

const _SQL = await initSqlJs()

function getDbDir(): string {
  return (
    process.env.SUBSC_CLI_DB_DIR ?? path.join(homedir(), ".config", "subtrack")
  )
}

function saveDb(): void {
  if (!_db || !_dbPath) return
  writeFileSync(_dbPath, Buffer.from(_db.export()))
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
    FOREIGN KEY (tag_id) REFERENCES tags(id)
  )`)

  return _db
}

// For testing: replace the DB instance (e.g. with in-memory)
export function __setDb(db: Database): void {
  _db = db
  _dbPath = ""
}

function mapTags(subs: SharedArgs[]): SharedArgs[] {
  if (subs.length === 0) return subs

  const db = getDb()
  for (const sub of subs) {
    const rows = execObjs<{ name: string }>(
      db,
      `SELECT tags.name FROM tags
       JOIN subscription_tags ON subscription_tags.tag_id = tags.id
       WHERE subscription_tags.subscription_id = ?`,
      [sub.id],
    )
    sub.tags = rows.map((r) => r.name)
  }

  return subs
}

export const getSubscriptions = (): SharedArgs[] => {
  try {
    const db = getDb()
    const subs = execObjs<SharedArgs>(
      db,
      "SELECT id, name, price, currency, cycle FROM subscriptions ORDER BY id",
    )
    return mapTags(subs)
  } catch (error) {
    consola.error("Failed to fetch subscriptions:", error)
    throw error
  }
}

export const writeSubscription = (data: AddSharedArgs): void => {
  const db = getDb()
  const uniqueTags = Array.from(new Set(data.tags))

  try {
    db.run("BEGIN TRANSACTION")
    db.run(
      "INSERT INTO subscriptions (name, price, currency, cycle) VALUES (?, ?, ?, ?)",
      [data.name, data.price, data.currency, data.cycle],
    )

    const idRow = execObj<Record<string, SqlValue>>(db, "SELECT last_insert_rowid() AS id")
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
    try { db.run("ROLLBACK") } catch {}
    consola.error("Failed to add subscription:", error)
    throw error
  }
}

export const deleteSubscription = (id: number): void => {
  try {
    const db = getDb()
    db.run("DELETE FROM subscriptions WHERE id = ?", [id])

    if (db.getRowsModified() === 0) {
      consola.warn(`No subscription found with id ${id}`)
    } else {
      saveDb()
    }
  } catch (error) {
    consola.error("Failed to delete subscription:", error)
    throw error
  }
}

export const getAllTags = (): string[] => {
  try {
    const db = getDb()
    const rows = execObjs<{ name: string }>(db, "SELECT name FROM tags ORDER BY name")
    return rows.map(r => r.name)
  } catch (error) {
    consola.error("Failed to fetch tags:", error)
    throw error
  }
}

export const tagsSubscription = (tag: string[] | string): SharedArgs[] => {
  try {
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
  } catch (error) {
    consola.error("Failed to filter by tags:", error)
    throw error
  }
}
