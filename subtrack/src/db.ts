import initSqlJs from "sql.js"
import type { Database, SqlValue, BindParams } from "sql.js"
import {
  mkdirSync, existsSync, readFileSync, writeFileSync,
  readdirSync, statSync, openSync, writeSync, closeSync,
  unlinkSync, chmodSync,
  constants,
} from "node:fs"
import { createHash } from "node:crypto"
import { gzipSync, gunzipSync } from "node:zlib"
import path from "node:path"
import { homedir } from "node:os"
import { consola } from "consola"
import { encryptBuffer, decryptBuffer, isEncrypted } from "./crypto.ts"
import type {
  Currency,
  Cycle,
  SharedArgs,
  AddSharedArgs,
  LlmUsageEntry,
  AddLlmUsageArgs,
  AddLlmUsageFromLogArgs,
  BackupFileInfo,
  TrialEntry,
  AddTrialArgs,
} from "./types.ts"

let _db: Database | null = null
let _dbPath = ""
let _lockFd: number | null = null

const _SQL = await initSqlJs()

// ── Directory validation ──────────────────────────────────

/** Validate that SUBSC_CLI_DB_DIR is safe to use. */
function validateDbDir(dir: string): void {
  if (!dir || typeof dir !== "string") {
    throw new Error("SUBSC_CLI_DB_DIR must be a non-empty string")
  }
  if (dir.length > 4096) {
    throw new Error("SUBSC_CLI_DB_DIR path too long")
  }
  const normalized = path.resolve(dir)
  // Prevent pointing to sensitive system directories
  const forbidden = ["/", "/etc", "/dev", "/proc", "/sys", "/tmp"]
  if (forbidden.includes(normalized)) {
    throw new Error(`SUBSC_CLI_DB_DIR cannot be a system directory: ${normalized}`)
  }
}

// ── File locking ──────────────────────────────────────────

const LOCK_STALE_MS = 30_000 // 30 seconds

/** Check whether the process that owns a lock is still alive. */
function isLockOwnerAlive(pid: string): boolean {
  const pidNumber = Number(pid)
  if (!Number.isInteger(pidNumber) || pidNumber <= 0) return false

  try {
    // Signal 0 tests whether the process exists without sending a signal
    process.kill(pidNumber, 0)
    return true
  } catch (err) {
    const nodeErr = err as NodeJS.ErrnoException
    // ESRCH: no such process, EPERM: process exists but can't be signaled
    return nodeErr.code === "EPERM"
  }
}

function getLockPath(): string {
  return path.join(getDbDir(), ".subtrack.lock")
}

function readLockFile(lockPath: string): { pid: string; timestamp: number } | null {
  try {
    const content = readFileSync(lockPath, "utf-8").trim()
    const lines = content.split("\n")
    return { pid: lines[0] ?? "unknown", timestamp: Number(lines[1]) || 0 }
  } catch {
    return null
  }
}

function acquireLock(): void {
  const lockPath = getLockPath()
  try {
    _lockFd = openSync(
      lockPath,
      constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL,
      0o600,
    )
    // Write PID and timestamp for stale detection
    writeSync(_lockFd, `${process.pid}\n${Date.now()}\n`)
  } catch (err) {
    const nodeErr = err as NodeJS.ErrnoException
    if (nodeErr.code === "EEXIST") {
      const info = readLockFile(lockPath)
      const pid = info?.pid ?? "unknown"
      const elapsed = info?.timestamp ? Date.now() - info.timestamp : 0

      // Check if lock is stale (only remove if owner process is gone)
      if (info?.timestamp && elapsed > LOCK_STALE_MS && !isLockOwnerAlive(pid)) {
        consola.warn(
          `Removing stale lock from PID ${pid} (${Math.floor(elapsed / 1000)}s old)`,
        )
        try { unlinkSync(lockPath) } catch { /* ignore */ }
        // Retry once
        _lockFd = openSync(
          lockPath,
          constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL,
          0o600,
        )
        writeSync(_lockFd, `${process.pid}\n${Date.now()}\n`)
        return
      }

      consola.error(
        `Another subtrack instance (PID ${pid}) may be running.\n` +
        `  If this is incorrect, delete: ${lockPath}`,
      )
      throw new Error(`Cannot acquire lock: another instance may be running (PID ${pid})`)
    } else {
      throw err
    }
  }
}

function releaseLock(): void {
  if (_lockFd !== null) {
    try { closeSync(_lockFd) } catch { /* ignore */ }
    _lockFd = null
    try { unlinkSync(getLockPath()) } catch { /* ignore */ }
  }
}

// Release lock on process exit (SIGINT/SIGTERM handled in index.ts for saveDb)
process.on("exit", releaseLock)

// ── DB directory ──────────────────────────────────────────

export function getDbDir(): string {
  const dir = process.env.SUBSC_CLI_DB_DIR ?? path.join(homedir(), ".config", "subtrack")
  validateDbDir(dir)
  return dir
}

export function getDefaultBackupDir(): string {
  return path.join(getDbDir(), "backups")
}

export function saveDb(): void {
  if (!_db || !_dbPath) return
  const data = Buffer.from(_db.export())
  writeFileSync(_dbPath, encryptBuffer(data), { mode: 0o600 })
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

/** Apply schema creation and migrations to a database instance. */
function runMigrations(db: Database): void {
  db.run(`CREATE TABLE IF NOT EXISTS subscriptions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    price INTEGER NOT NULL,
    currency TEXT NOT NULL,
    cycle TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    billing_day INTEGER,
    created_at TEXT NOT NULL DEFAULT (date('now')),
    notes TEXT
  )`)
  db.run(`CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
  )`)
  db.run(`CREATE TABLE IF NOT EXISTS subscription_tags (
    subscription_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,
    PRIMARY KEY (subscription_id, tag_id),
    FOREIGN KEY (subscription_id) REFERENCES subscriptions(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
  )`)
  db.run(`CREATE TABLE IF NOT EXISTS llm_usage (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    cost REAL NOT NULL,
    date TEXT NOT NULL,
    description TEXT,
    generation_id TEXT
  )`)
  db.run(`CREATE TABLE IF NOT EXISTS trials (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    price INTEGER,
    currency TEXT,
    cycle TEXT,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (date('now'))
  )`)

  // Migration: add generation_id column if missing (pre-4.1.0 databases)
  const llmCols = db.exec("PRAGMA table_info(llm_usage)")
  const hasGenId = llmCols.length > 0 && llmCols[0].values.some(
    (row) => String(row[1]) === "generation_id",
  )
  if (!hasGenId) {
    db.run("ALTER TABLE llm_usage ADD COLUMN generation_id TEXT")
    db.run("CREATE UNIQUE INDEX IF NOT EXISTS idx_llm_usage_generation_id ON llm_usage(generation_id)")
  }

  // Migration: add notes column if missing (pre-6.x databases)
  const subCols = db.exec("PRAGMA table_info(subscriptions)")
  const hasNotes = subCols.length > 0 && subCols[0].values.some(
    (row) => String(row[1]) === "notes",
  )
  if (!hasNotes) {
    db.run("ALTER TABLE subscriptions ADD COLUMN notes TEXT")
  }

  // Migration: add payment_method column if missing
  const hasPaymentMethod = subCols.length > 0 && subCols[0].values.some(
    (row) => String(row[1]) === "payment_method",
  )
  if (!hasPaymentMethod) {
    db.run("ALTER TABLE subscriptions ADD COLUMN payment_method TEXT")
  }

  // Verify database integrity on startup
  const integrityResult = db.exec("PRAGMA integrity_check")
  if (
    integrityResult.length > 0 &&
    integrityResult[0].values.length > 0 &&
    String(integrityResult[0].values[0][0]) !== "ok"
  ) {
    consola.warn(
      `Database integrity check failed: ${String(integrityResult[0].values[0][0])}\n` +
      "  Run 'subtrack backup' immediately and restore from a known-good backup.",
    )
  }
}

export function getDb(): Database {
  if (_db) return _db

  const dbdir = getDbDir()
  mkdirSync(dbdir, { recursive: true, mode: 0o700 })
  _dbPath = path.join(dbdir, "subtrack.db")

  acquireLock()

  if (existsSync(_dbPath)) {
    const buf = readFileSync(_dbPath)
    const data = isEncrypted(buf) ? decryptBuffer(buf) : buf
    _db = new _SQL.Database(data)
  } else {
    _db = new _SQL.Database()
  }

  _db.run("PRAGMA foreign_keys = ON")
  _db.run("PRAGMA secure_delete = ON")
  runMigrations(_db)

  return _db
}

export function getBackupFiles(dir: string): BackupFileInfo[] {
  if (!existsSync(dir)) return []

  const entries = readdirSync(dir)
  const activeDb = path.basename(getDbPath())
  const skipNames = new Set<string>(["subtrack.db"])
  if (activeDb) skipNames.add(activeDb)

  return entries
    .filter((f) => {
      const lower = f.toLowerCase()
      return (lower.endsWith(".db") || lower.endsWith(".db.gz") || lower.endsWith(".db.enc")) &&
        !skipNames.has(f) &&
        !f.includes("_before_restore.db")
    })
    .map((f) => {
      const fullPath = path.join(dir, f)
      const st = statSync(fullPath)
      return {
        name: f,
        path: fullPath,
        mtime: st.mtime,
        size: st.size,
      }
    })
    .sort((a, b) => b.mtime.getTime() - a.mtime.getTime())
}

export function restoreDb(backupPath: string): void {
  const raw = readFileSync(backupPath)
  const lower = backupPath.toLowerCase()

  // Determine file type by extension and content
  const isGzipFile = lower.endsWith(".gz")
  const isEncryptedFile = lower.endsWith(".db.enc")
  const hasGzipMagic = raw.length >= 2 && raw[0] === 0x1f && raw[1] === 0x8b

  let data: Buffer

  if (isEncryptedFile) {
    // Encrypted backup: decrypt -> (possibly gzipped inside)
    try {
      data = decryptBuffer(raw)
    } catch {
      throw new Error(
        "Failed to decrypt backup. The encryption key may have changed or the backup is corrupted.\n" +
        "  If you changed your database passphrase or .key file, restore using the old key.",
      )
    }
  } else if (isGzipFile || hasGzipMagic) {
    // Gzip backup: decompress -> (possibly encrypted inside, though unusual)
    const decompressed = gunzipSync(raw)
    if (isEncrypted(decompressed)) {
      try {
        data = decryptBuffer(decompressed)
      } catch {
        throw new Error(
          "Failed to decrypt backup. The encryption key may have changed or the backup is corrupted.\n" +
          "  If you changed your database passphrase or .key file, restore using the old key.",
        )
      }
    } else {
      data = decompressed
    }
  } else if (isEncrypted(raw)) {
    // No recognized extension but looks encrypted — try decrypt
    try {
      data = decryptBuffer(raw)
    } catch {
      throw new Error(
        "Failed to decrypt backup. The encryption key may have changed or the backup is corrupted.\n" +
        "  If you changed your database passphrase or .key file, restore using the old key.",
      )
    }
  } else {
    // Plain SQLite
    data = raw
  }

  // If the result is still gzipped (e.g. encrypted file that was gzip inside -> already decrypted)
  // But if data is now gzip, decompress it
  const isGz = data.length >= 2 && data[0] === 0x1f && data[1] === 0x8b
  const buf = isGz ? gunzipSync(data) : data

  // Verify it's a valid SQLite DB with correct schema
  const newDb = new _SQL.Database(buf)
  const tables = newDb.exec(
    "SELECT name FROM sqlite_master WHERE type='table' AND name='subscriptions'",
  )
  const hasSubscriptions =
    tables.length > 0 && tables[0].values.length > 0
  if (!hasSubscriptions) {
    throw new Error(
      "Invalid backup file: missing 'subscriptions' table — not a subtrack database",
    )
  }

  // Flush current in-memory state to disk
  saveDb()

  // Overwrite the active database file (no-op in test mode with __setDb)
  if (_dbPath) {
    writeFileSync(_dbPath, encryptBuffer(buf), { mode: 0o600 })
  }

  // Replace in-memory instance
  newDb.run("PRAGMA foreign_keys = ON")
  newDb.run("PRAGMA secure_delete = ON")
  _db = newDb
  runMigrations(newDb)
}

/** Replace the DB instance for testing (e.g. with in-memory). */
export function __setDb(db: Database): void {
  _db = db
  _dbPath = ""
}

// ── Backup integrity ──────────────────────────────────────

export function getBackupHashPath(backupPath: string): string {
  return `${backupPath}.sha256`
}

export function writeBackupHash(backupPath: string): void {
  const content = readFileSync(backupPath)
  const hash = createHash("sha256").update(content).digest("hex")
  writeFileSync(getBackupHashPath(backupPath), hash + "\n")
}

export function verifyBackupHash(backupPath: string): boolean {
  const hashPath = getBackupHashPath(backupPath)
  if (!existsSync(hashPath)) {
    consola.warn(
      `No integrity hash found for "${path.basename(backupPath)}" — integrity cannot be verified.`,
    )
    return true // backward compat: skip if no sidecar
  }
  const expected = readFileSync(hashPath, "utf-8").trim()
  const content = readFileSync(backupPath)
  const actual = createHash("sha256").update(content).digest("hex")
  return expected === actual
}

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

const SORT_FIELDS = ["id", "name", "price", "currency", "cycle", "status"] as const

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
    `SELECT id, name, price, currency, cycle, status, billing_day AS billingDay, created_at AS createdAt, notes, payment_method AS paymentMethod FROM subscriptions
     WHERE id IN (${idPlaceholders})`,
    ids,
  )

  return mapTags(subs)
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

/** Add from log import if not duplicate (by generation_id). Returns true if added, false if duplicate. */
export const addLlmUsageFromLog = (data: AddLlmUsageFromLogArgs): boolean => {
  const db = getDb()

  // Dedup: skip if generation_id already exists
  if (data.generation_id) {
    const existing = execObj<{ id: number }>(
      db,
      "SELECT id FROM llm_usage WHERE generation_id = ?",
      [data.generation_id],
    )
    if (existing) return false
  }

  db.run(
    `INSERT INTO llm_usage (provider, model, input_tokens, output_tokens, cost, date, description, generation_id)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
    [
      data.provider,
      data.model,
      data.input_tokens,
      data.output_tokens,
      data.cost,
      data.date,
      data.description,
      data.generation_id ?? null,
    ],
  )
  saveDb()
  return true
}

/**
 * Batch add usage entries from log sources.
 * Checks all generation_ids upfront, inserts new entries in a single transaction.
 * Much faster than calling addLlmUsageFromLog individually for each entry.
 */
export const batchAddLlmUsageFromLog = (
  entries: AddLlmUsageFromLogArgs[],
): { added: number; skipped: number } => {
  if (entries.length === 0) return { added: 0, skipped: 0 }

  const db = getDb()

  // Collect existing generation_ids for dedup
  const existing = new Set<string>()
  const rows = execObjs<{ generation_id: string | null }>(
    db,
    "SELECT generation_id FROM llm_usage WHERE generation_id IS NOT NULL",
  )
  for (const row of rows) {
    if (row.generation_id) existing.add(row.generation_id)
  }

  let added = 0
  let skipped = 0

  db.run("BEGIN TRANSACTION")
  try {
    for (const entry of entries) {
      if (existing.has(entry.generation_id)) {
        skipped++
        continue
      }
      existing.add(entry.generation_id)

      db.run(
        `INSERT INTO llm_usage (provider, model, input_tokens, output_tokens, cost, date, description, generation_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
        [
          entry.provider,
          entry.model,
          entry.input_tokens,
          entry.output_tokens,
          entry.cost,
          entry.date,
          entry.description,
          entry.generation_id,
        ],
      )
      added++
    }
    db.run("COMMIT")
    saveDb()
  } catch (error) {
    db.run("ROLLBACK")
    throw error
  }

  return { added, skipped }
}

export type GetLlmUsageOptions = {
  provider?: string
  from?: string
  to?: string
  limit?: number
  offset?: number
  minCost?: number
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
  if (options?.minCost !== undefined) {
    conditions.push("cost >= ?")
    params.push(options.minCost)
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

// ── Trial CRUD ──────────────────────────────────────────────

export const writeTrial = (data: AddTrialArgs): void => {
  const db = getDb()
  db.run(
    "INSERT INTO trials (name, expires_at, price, currency, cycle, notes) VALUES (?, ?, ?, ?, ?, ?)",
    [data.name, data.expiresAt, data.price ?? null, data.currency ?? null, data.cycle ?? null, data.notes ?? null],
  )
  saveDb()
}

export const getTrials = (): TrialEntry[] => {
  const db = getDb()
  return execObjs<TrialEntry>(
    db,
    "SELECT id, name, expires_at AS expiresAt, price, currency, cycle, notes, created_at AS createdAt FROM trials ORDER BY expires_at ASC",
  )
}

export const getTrial = (id: number): TrialEntry | undefined => {
  const db = getDb()
  return execObj<TrialEntry>(
    db,
    "SELECT id, name, expires_at AS expiresAt, price, currency, cycle, notes, created_at AS createdAt FROM trials WHERE id = ?",
    [id],
  )
}

export const deleteTrial = (id: number): boolean => {
  const db = getDb()
  db.run("DELETE FROM trials WHERE id = ?", [id])
  const modified = db.getRowsModified() > 0
  if (modified) saveDb()
  return modified
}

/** Return trial entries expiring within the next `days` days. */
export const getTrialsExpiringSoon = (days: number): TrialEntry[] => {
  const db = getDb()
  return execObjs<TrialEntry>(
    db,
    `SELECT id, name, expires_at AS expiresAt, price, currency, cycle, notes, created_at AS createdAt FROM trials
     WHERE expires_at >= date('now') AND expires_at <= date('now', '+' || ? || ' days')
     ORDER BY expires_at ASC`,
    [days],
  )
}
