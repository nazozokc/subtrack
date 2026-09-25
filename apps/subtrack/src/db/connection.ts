import { DatabaseSync } from "node:sqlite"
import type { SQLInputValue } from "node:sqlite"
import {
  mkdirSync, existsSync, readFileSync, writeFileSync,
  readdirSync, statSync, openSync, writeSync, closeSync,
  unlinkSync, renameSync,
  constants,
} from "node:fs"
import { createHash, randomUUID } from "node:crypto"
import { gunzipSync } from "node:zlib"
import path from "node:path"
import { homedir } from "node:os"
import { consola } from "@subtrack/lib/logger"
import { encryptBuffer, decryptBuffer, isEncrypted } from "@subtrack/lib/crypto"
import type { BackupFileInfo } from "../types.ts"
import { runMigrations, SCHEMA_VERSION } from "./schema.ts"
import { writeDbHash, verifyDbHash, removeDbHash } from "./integrity.ts"

let _db: DatabaseSync | null = null
let _dbPath = ""
let _lockFd: number | null = null

/** Max decompressed size for backups (prevents zip-bomb / decompression-bomb DoS). */
const MAX_DECOMPRESSED_BYTES = 256 * 1024 * 1024 // 256 MB

/** Schema tables required for a current-version subtrack database file. */
const LEGACY_TABLES = ["subscriptions", "tags", "subscription_tags"]

const REQUIRED_TABLES = [
  ...LEGACY_TABLES,
  "llm_usage",
  "trials",
  "price_history",
  "audit_log",
  "suggestions",
]

/** Columns required by the current schema. */
const REQUIRED_COLUMNS: Record<string, readonly string[]> = {
  subscriptions: [
    "id", "name", "price", "currency", "cycle", "status", "billing_day", "created_at",
    "notes", "payment_method", "contract_start", "contract_end", "auto_renewal",
    "vendor_name", "vendor_url", "plan_tier", "discount_amount", "discount_type",
  ],
  tags: ["id", "name"],
  subscription_tags: ["subscription_id", "tag_id"],
  llm_usage: [
    "id", "provider", "model", "input_tokens", "output_tokens", "cost", "date", "description",
    "generation_id",
  ],
  trials: ["id", "name", "expires_at", "price", "currency", "cycle", "notes", "created_at"],
  price_history: [
    "id", "subscription_id", "old_price", "new_price", "old_currency", "new_currency", "changed_at",
  ],
  audit_log: ["id", "action", "target_type", "target_id", "details", "created_at"],
  suggestions: [
    "id", "name", "price", "currency", "cycle", "vendor_name", "vendor_url", "plan_tier",
    "payment_method", "source", "source_detail", "email_subject", "email_from", "email_date",
    "confidence", "status", "matched_sub_id", "created_at",
  ],
}

const LEGACY_COLUMNS: Record<string, readonly string[]> = {
  subscriptions: [
    "id", "name", "price", "currency", "cycle", "status", "billing_day", "created_at",
  ],
  tags: ["id", "name"],
  subscription_tags: ["subscription_id", "tag_id"],
}

/** Validate the schema before a restore can replace the live database. */
function validateRestoredDatabase(db: DatabaseSync): void {
  const versionRow = db.prepare("PRAGMA user_version").get() as
    | { user_version: number }
    | undefined
  const version = Number(versionRow?.user_version ?? 0)
  if (!Number.isInteger(version) || version < 0) {
    throw new Error("Invalid backup file: invalid schema version")
  }
  if (version > SCHEMA_VERSION) {
    throw new Error(
      `Invalid backup file: schema version ${version} is newer than supported version ${SCHEMA_VERSION}`,
    )
  }

  const tables = new Set(
    (db.prepare("SELECT name FROM sqlite_master WHERE type = 'table'").all() as { name: string }[])
      .map((row) => row.name),
  )
  const isLegacy = version < 1
  const requiredTables = isLegacy ? LEGACY_TABLES : REQUIRED_TABLES
  const missingTables = requiredTables.filter((table) => !tables.has(table))
  if (missingTables.length > 0) {
    const primary = missingTables.includes("subscriptions")
      ? "missing 'subscriptions' table"
      : `missing table(s): ${missingTables.join(", ")}`
    const rest = missingTables.filter((table) => table !== "subscriptions")
    throw new Error(
      `Invalid backup file: ${primary}${rest.length > 0 ? ` (also missing: ${rest.join(", ")})` : ""}`,
    )
  }

  const requiredColumns = isLegacy
    ? LEGACY_COLUMNS
    : REQUIRED_COLUMNS
  for (const [table, columns] of Object.entries(requiredColumns)) {
    const tableColumns = new Set(
      (db.prepare(`PRAGMA table_info(${table})`).all() as { name: string }[])
        .map((row) => row.name),
    )
    const missingColumns = columns.filter((column) => !tableColumns.has(column))
    if (missingColumns.length > 0) {
      throw new Error(
        `Invalid backup file: ${table} is missing column(s): ${missingColumns.join(", ")}`,
      )
    }
  }

  const foreignKeyErrors = db.prepare("PRAGMA foreign_key_check").all()
  if (foreignKeyErrors.length > 0) {
    throw new Error("Invalid backup file: foreign key integrity check failed")
  }
}

/** Gunzip with a strict output cap; throws a clear error when exceeded. */
function gunzipLimited(data: Buffer): Buffer {
  try {
    return gunzipSync(data, { maxOutputLength: MAX_DECOMPRESSED_BYTES })
  } catch (err) {
    const nodeErr = err as NodeJS.ErrnoException
    if (nodeErr.code === "ERR_BUFFER_TOO_LARGE") {
      throw new Error(
        `Backup decompresses to more than ${MAX_DECOMPRESSED_BYTES / 1024 / 1024} MB — refusing to restore (possible decompression bomb)`,
      )
    }
    throw err
  }
}

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
  // (on Windows, path.resolve("/") is a drive root like "D:\", so compare
  // against the filesystem root instead of a literal "/")
  const root = path.parse(normalized).root
  const forbidden = ["/etc", "/dev", "/proc", "/sys", "/tmp"]
  if (normalized === root || forbidden.includes(normalized)) {
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

/** Close the open DB instance, release the lock, and remove temp files. */
export function closeDb(): void {
  if (_db) {
    try { _db.close() } catch { /* ignore */ }
    _db = null
  }
  try { unlinkSync(getOpenDbPath()) } catch { /* ignore */ }
  releaseLock()
}

// Release lock + temp DB files on process exit
// (SIGINT/SIGTERM handled in index.ts for saveDb)
process.on("exit", () => {
  releaseLock()
  closeDb()
  try { unlinkSync(getSaveDbPath()) } catch { /* ignore */ }
})

// ── DB directory ──────────────────────────────────────────

export function getDbDir(): string {
  const dir = process.env.SUBSC_CLI_DB_DIR ?? path.join(homedir(), ".config", "subtrack")
  validateDbDir(dir)
  // Normalize separators so downstream path.join() produces consistent
  // paths on all platforms (e.g. "/tmp/x" on Windows becomes "\tmp\x")
  return path.normalize(dir)
}

export function getDefaultBackupDir(): string {
  return path.join(getDbDir(), "backups")
}

// ── Temp backing files (node:sqlite is file-based) ───────

/** Escape a path for embedding in a SQL string literal. */
function sqlQuotePath(p: string): string {
  return p.replace(/'/g, "''")
}

/** Backing file for the open DB instance (exists only while running). */
function getOpenDbPath(): string {
  return path.join(getDbDir(), ".subtrack.open.db")
}

/** Throwaway file used for VACUUM INTO exports. */
function getSaveDbPath(): string {
  return path.join(getDbDir(), ".subtrack.save.db")
}

/** Ensure the DB directory exists (needed before writing backing files). */
function ensureDbDir(): string {
  const dbdir = getDbDir()
  mkdirSync(dbdir, { recursive: true, mode: 0o700 })
  return dbdir
}

/** Serialize the DB contents to bytes (like sql.js `db.export()`). */
export function exportDbBytes(): Buffer {
  const db = getDb()
  ensureDbDir()
  const tmp = getSaveDbPath()
  try { unlinkSync(tmp) } catch { /* ignore */ }
  db.exec(`VACUUM INTO '${sqlQuotePath(tmp)}'`)
  const data = readFileSync(tmp)
  try { unlinkSync(tmp) } catch { /* ignore */ }
  return data
}

function replaceEncryptedDb(encrypted: Buffer, dbPath: string): void {
  const dbDir = path.dirname(dbPath)
  const token = randomUUID()
  const tempDbPath = path.join(dbDir, `.subtrack.db.${token}.tmp`)
  const tempHashPath = `${tempDbPath}.sha256`
  let dbReplaced = false

  try {
    // Write both files completely before replacing either live path. This
    // prevents a process interruption from truncating the active database.
    writeFileSync(tempDbPath, encrypted, { mode: 0o600 })
    writeDbHash(encrypted, tempDbPath)
    renameSync(tempDbPath, dbPath)
    dbReplaced = true
    renameSync(tempHashPath, `${dbPath}.sha256`)
  } catch (error) {
    if (dbReplaced) {
      // The new database is valid, but its old sidecar is now stale. Remove it
      // so the next open can regenerate a matching hash.
      try { removeDbHash(dbPath) } catch { /* best-effort recovery */ }
    }
    try { unlinkSync(tempDbPath) } catch { /* best-effort cleanup */ }
    try { unlinkSync(tempHashPath) } catch { /* best-effort cleanup */ }
    throw error
  }
}

export function saveDb(): void {
  if (!_db || !_dbPath) return
  const data = exportDbBytes()
  replaceEncryptedDb(encryptBuffer(data), _dbPath)
}

export function getDbPath(): string {
  getDb()
  return _dbPath
}

export function execObjs<T>(db: DatabaseSync, sql: string, params?: SQLInputValue[]): T[] {
  const rows = db.prepare(sql).all(...(params ?? []))
  return rows as unknown as T[]
}

export function execObj<T>(
  db: DatabaseSync,
  sql: string,
  params?: SQLInputValue[],
): T | undefined {
  const row = db.prepare(sql).get(...(params ?? []))
  return row as unknown as T | undefined
}

export function getDb(): DatabaseSync {
  if (_db) return _db

  const dbdir = ensureDbDir()
  _dbPath = path.join(dbdir, "subtrack.db")

  acquireLock()

  const openPath = getOpenDbPath()
  if (existsSync(_dbPath)) {
    const buf = readFileSync(_dbPath)
    // Verify on-disk integrity before decryption
    const integrity = verifyDbHash(buf, _dbPath)
    if (!integrity.ok && !integrity.recoverable) {
      consola.warn(
        `Database checksum mismatch (expected ${integrity.expected}, got ${integrity.actual}).\n` +
        "  The database file may be corrupted or tampered with.\n" +
        "  Attempting to load anyway — back up immediately if successful.",
      )
    }
    const encrypted = isEncrypted(buf)
    const data = encrypted ? decryptBuffer(buf) : buf
    if (integrity.recoverable) {
      try {
        // The authenticated decryption succeeded, so a missing/corrupt
        // sidecar can be safely regenerated from the verified file.
        writeDbHash(buf, _dbPath)
      } catch {
        consola.warn("Could not regenerate the database integrity hash; continuing without it")
      }
    }
    writeFileSync(openPath, data, { mode: 0o600 })
    _db = new DatabaseSync(openPath)
  } else {
    _db = new DatabaseSync(openPath)
  }

  _db.exec("PRAGMA foreign_keys = ON")
  _db.exec("PRAGMA secure_delete = ON")
  runMigrations(_db, dbdir)

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
    const decompressed = gunzipLimited(raw)
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
  const buf = isGz ? gunzipLimited(data) : data

  // Validate it's a valid SQLite DB with the schema this version can restore.
  ensureDbDir()
  const savePath = getSaveDbPath()
  writeFileSync(savePath, buf, { mode: 0o600 })
  let newDb: DatabaseSync | null = null
  try {
    newDb = new DatabaseSync(savePath)
    // Quick integrity check — catches corruption before it replaces the live DB
    const integrity = newDb.prepare("PRAGMA integrity_check").get() as Record<string, string> | undefined
    const integrityOk = integrity !== undefined && Object.values(integrity)[0] === "ok"
    if (!integrityOk) {
      throw new Error(
        `Invalid backup file: integrity check failed (${integrity ? Object.values(integrity)[0] : "unreadable"})`,
      )
    }
    validateRestoredDatabase(newDb)
    newDb.exec("PRAGMA foreign_keys = ON")
    // Complete migrations on the isolated backup before it can become live.
    runMigrations(newDb)
  } catch (error) {
    try { unlinkSync(savePath) } catch { /* best-effort cleanup */ }
    throw error
  } finally {
    if (newDb) {
      try { newDb.close() } catch { /* best-effort cleanup */ }
    }
  }

  // Read the migrated snapshot back so the encrypted live file and the open
  // backing file represent the same schema.
  const restoredBuf = readFileSync(savePath)

  // Flush current in-memory state to disk
  saveDb()

  // Overwrite the active database file atomically (no-op in test mode with __setDb)
  if (_dbPath) {
    replaceEncryptedDb(encryptBuffer(restoredBuf), _dbPath)
  }

  // Close old instance and its backing file
  const oldOpenPath = getOpenDbPath()
  if (_db) {
    try { _db.close() } catch { /* ignore */ }
    _db = null
    try { unlinkSync(oldOpenPath) } catch { /* ignore */ }
  }

  // Move the restored data to the open backing file
  try {
    renameSync(savePath, oldOpenPath)
  } catch {
    writeFileSync(oldOpenPath, restoredBuf, { mode: 0o600 })
    try { unlinkSync(savePath) } catch { /* ignore */ }
  }

  // Open the restored database
  _db = new DatabaseSync(oldOpenPath)
  _db.exec("PRAGMA foreign_keys = ON")
  _db.exec("PRAGMA secure_delete = ON")
  runMigrations(_db, getDbDir())
}

/** Replace the DB instance for testing (e.g. with in-memory). */
export function __setDb(db: DatabaseSync): void {
  _db = db
  _dbPath = ""
  _lockFd = null
}

// ── Backup integrity ──────────────────────────────────────

export function getBackupHashPath(backupPath: string): string {
  return `${backupPath}.sha256`
}

export function writeBackupHash(backupPath: string): void {
  const content = readFileSync(backupPath)
  const hash = createHash("sha256").update(content).digest("hex")
  writeFileSync(getBackupHashPath(backupPath), hash + "\n", { mode: 0o600 })
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
