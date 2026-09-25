import type { DatabaseSync } from "node:sqlite"
import { readFileSync, writeFileSync } from "node:fs"
import path from "node:path"
import { consola } from "@subtrack/lib/logger"
import { createAuditTable } from "./audit.ts"

/** Current schema version. Bump when adding a new migration below. */
export const SCHEMA_VERSION = 1

/**
 * Marker file name for the daily integrity check timestamp.
 * Lives in the DB directory (next to `subtrack.db`).
 */
const INTEGRITY_MARKER = ".subtrack.integrity"

/** Read the current PRAGMA user_version of a database. */
export function getSchemaVersion(db: DatabaseSync): number {
  const row = db.prepare("PRAGMA user_version").get() as
    | { user_version: number }
    | undefined
  return row ? Number(row.user_version) : 0
}

/**
 * `PRAGMA integrity_check` scans every page of the database, which is wasteful
 * on every CLI invocation. The file hash check (`verifyDbHash`) already runs
 * on each open, so the full scan is throttled to once per calendar day.
 */
function integrityCheckDue(dbDir: string | undefined): boolean {
  if (!dbDir) return true // no directory info (e.g. tests) → always check
  const marker = path.join(dbDir, INTEGRITY_MARKER)
  try {
    return readFileSync(marker, "utf-8").trim() !== today()
  } catch {
    return true
  }
}

function markIntegrityChecked(dbDir: string | undefined): void {
  if (!dbDir) return
  try {
    writeFileSync(path.join(dbDir, INTEGRITY_MARKER), today() + "\n", { mode: 0o600 })
  } catch { /* best-effort */ }
}

function today(): string {
  return new Date().toISOString().slice(0, 10)
}

/** Apply schema creation and migrations to a database instance. */
export function runMigrations(db: DatabaseSync, dbDir?: string): void {
  // v0 → v1: baseline schema. Idempotent, so it also repairs databases
  // created before versioning was introduced (user_version = 0).
  const version = getSchemaVersion(db)
  if (version < SCHEMA_VERSION) {
    db.exec("BEGIN TRANSACTION")
    try {
      if (version < 1) {
        migrateToV1(db)
      }

      // Future migrations go here:
      // if (version < 2) migrateToV2(db)
      // if (version < 3) migrateToV3(db)

      // Record the latest version so migration steps only run once
      db.exec(`PRAGMA user_version = ${SCHEMA_VERSION}`)
      db.exec("COMMIT")
    } catch (error) {
      db.exec("ROLLBACK")
      throw error
    }
  }

  // Verify database integrity on startup — at most once per day.
  // (The per-open file hash check in connection.ts covers every other run.)
  if (integrityCheckDue(dbDir)) {
    const integrity = db.prepare("PRAGMA integrity_check").get() as
      | { integrity_check: string }
      | undefined
    markIntegrityChecked(dbDir)
    if (integrity && String(integrity.integrity_check) !== "ok") {
      consola.warn(
        `Database integrity check failed: ${String(integrity.integrity_check)}\n` +
        "  Run 'subtrack backup' immediately and restore from a known-good backup.",
      )
    }
  }
}

/** v1 baseline: tables, columns, and indexes. */
function migrateToV1(db: DatabaseSync): void {
  db.exec(`CREATE TABLE IF NOT EXISTS subscriptions (
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
  db.exec(`CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
  )`)
  db.exec(`CREATE TABLE IF NOT EXISTS subscription_tags (
    subscription_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,
    PRIMARY KEY (subscription_id, tag_id),
    FOREIGN KEY (subscription_id) REFERENCES subscriptions(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
  )`)
  db.exec(`CREATE TABLE IF NOT EXISTS llm_usage (
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
  db.exec(`CREATE TABLE IF NOT EXISTS trials (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    price INTEGER,
    currency TEXT,
    cycle TEXT,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (date('now'))
  )`)
  db.exec(`CREATE TABLE IF NOT EXISTS price_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    subscription_id INTEGER NOT NULL,
    old_price INTEGER,
    new_price INTEGER NOT NULL,
    old_currency TEXT,
    new_currency TEXT NOT NULL,
    changed_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (subscription_id) REFERENCES subscriptions(id) ON DELETE CASCADE
  )`)

  const execObjs = (sql: string): Record<string, unknown>[] =>
    db.prepare(sql).all() as unknown as Record<string, unknown>[]

  // Migration: add generation_id column if missing (pre-4.1.0 databases)
  const llmCols = execObjs("PRAGMA table_info(llm_usage)")
  const hasGenId = llmCols.some((row) => String(row.name) === "generation_id")
  if (!hasGenId) {
    db.exec("ALTER TABLE llm_usage ADD COLUMN generation_id TEXT")
    db.exec("CREATE UNIQUE INDEX IF NOT EXISTS idx_llm_usage_generation_id ON llm_usage(generation_id)")
  }

  // Migration: add notes column if missing (pre-6.x databases)
  const subCols = execObjs("PRAGMA table_info(subscriptions)")
  const hasNotes = subCols.some((row) => String(row.name) === "notes")
  if (!hasNotes) {
    db.exec("ALTER TABLE subscriptions ADD COLUMN notes TEXT")
  }

  // Migration: add payment_method column if missing
  const hasPaymentMethod = subCols.some((row) => String(row.name) === "payment_method")
  if (!hasPaymentMethod) {
    db.exec("ALTER TABLE subscriptions ADD COLUMN payment_method TEXT")
  }

  // Migration: add extended subscription columns if missing (contract, vendor, discount, auto-renewal)
  const extendedCols: [string, string][] = [
    ["contract_start", "TEXT"],
    ["contract_end", "TEXT"],
    ["auto_renewal", "INTEGER NOT NULL DEFAULT 1"],
    ["vendor_name", "TEXT"],
    ["vendor_url", "TEXT"],
    ["plan_tier", "TEXT"],
    ["discount_amount", "INTEGER"],
    ["discount_type", "TEXT"],
  ]
  for (const [name, ddl] of extendedCols) {
    const has = subCols.some((row) => String(row.name) === name)
    if (!has) {
      db.exec(`ALTER TABLE subscriptions ADD COLUMN ${name} ${ddl}`)
    }
  }

  // Audit log table
  createAuditTable(db)

  // Suggestions table for email scan results
  db.exec(`CREATE TABLE IF NOT EXISTS suggestions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    price INTEGER,
    currency TEXT,
    cycle TEXT,
    vendor_name TEXT,
    vendor_url TEXT,
    plan_tier TEXT,
    payment_method TEXT,
    source TEXT NOT NULL DEFAULT 'email',
    source_detail TEXT,
    email_subject TEXT,
    email_from TEXT,
    email_date TEXT,
    confidence REAL DEFAULT 0.0,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'dismissed', 'added')),
    matched_sub_id INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (matched_sub_id) REFERENCES subscriptions(id) ON DELETE SET NULL
  )`)
}
