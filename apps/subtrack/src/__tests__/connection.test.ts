import { test, expect, vi, beforeAll, afterAll, afterEach } from "vitest"
import { DatabaseSync } from "node:sqlite"
import {
  mkdtempSync, rmSync, existsSync, readFileSync, writeFileSync,
} from "node:fs"
import { join, parse } from "node:path"
import { tmpdir } from "node:os"
import { gzipSync } from "node:zlib"
import { decryptBuffer, isEncrypted } from "@subtrack/lib/crypto"

// Restore tests do heavy real-file I/O (write → validate → VACUUM INTO →
// encrypt → replace → reopen). On Windows CI (NTFS + real-time AV scanning)
// a single restoreDb can exceed the 15s global default, so give this suite
// generous headroom.
vi.setConfig({ testTimeout: 60_000 })

const conn = await import("../db/connection.ts")

// connection.ts caches _db after the first successful getDb(), so all
// file-backed tests share one persistent directory. Per-test dirs are
// only used where no lasting state is needed (lock conflict).
let mainDir: string
const tempDirs: string[] = []

beforeAll(() => {
  mainDir = mkdtempSync(join(tmpdir(), "subtrack-conn-main-"))
  process.env.SUBSC_CLI_DB_DIR = mainDir
})

/** Remove a directory, retrying briefly when Windows still holds a lock. */
async function removeDirWithRetry(dir: string): Promise<void> {
  const deadline = Date.now() + 5_000
  for (;;) {
    try {
      rmSync(dir, { recursive: true, force: true })
      return
    } catch (err) {
      const nodeErr = err as NodeJS.ErrnoException
      // EBUSY/EPERM: node:sqlite backing files stay locked for a moment
      // after close on Windows — retry instead of failing the suite.
      if (nodeErr.code !== "EBUSY" && nodeErr.code !== "EPERM") throw err
      if (Date.now() >= deadline) throw err
      await new Promise((resolve) => setTimeout(resolve, 100))
    }
  }
}

afterAll(async () => {
  // Close the cached DatabaseSync handle first: Windows cannot unlink a
  // file that is still open (POSIX allows it, which is why this only
  // fails on Windows).
  conn.closeDb()
  delete process.env.SUBSC_CLI_DB_DIR
  for (const dir of [mainDir, ...tempDirs]) {
    if (existsSync(dir)) await removeDirWithRetry(dir)
  }
})

afterEach(() => {
  delete process.env.SUBSC_CLI_DB_PASSPHRASE
})

// ── Directory validation ───────────────────────────────

test("getDbDir rejects system directories", () => {
  const bad = process.platform === "win32"
    ? [parse(process.cwd()).root]
    : ["/", "/etc", "/dev", "/proc", "/sys", "/tmp"]
  for (const dir of bad) {
    process.env.SUBSC_CLI_DB_DIR = dir
    expect(() => conn.getDbDir()).toThrow(/system directory/)
  }
  process.env.SUBSC_CLI_DB_DIR = mainDir
})

test("getDbDir rejects empty env value", () => {
  process.env.SUBSC_CLI_DB_DIR = ""
  expect(() => conn.getDbDir()).toThrow(/non-empty string/)
  process.env.SUBSC_CLI_DB_DIR = mainDir
})

test("getDbDir accepts a valid temp directory", () => {
  expect(conn.getDbDir()).toBe(mainDir)
})

// ── Lock handling ─────────────────────────────────────
// NOTE: must run before any successful getDb() call, because connection.ts
// caches _db and skips lock acquisition afterwards.

test("getDb throws when another instance holds the lock", () => {
  const dir = mkdtempSync(join(tmpdir(), "subtrack-conn-lock-"))
  tempDirs.push(dir)
  process.env.SUBSC_CLI_DB_DIR = dir
  // Simulate a live lock owned by this process
  writeFileSync(join(dir, ".subtrack.lock"), `${process.pid}\n${Date.now()}\n`)

  expect(() => conn.getDb()).toThrow(/another instance may be running/i)
})

test("getDb acquires lock, creates lock file, and sets db path", () => {
  process.env.SUBSC_CLI_DB_DIR = mainDir

  conn.getDb()
  const lockContent = readFileSync(join(mainDir, ".subtrack.lock"), "utf-8")
  const lines = lockContent.trim().split("\n")
  expect(lines[0]).toBe(String(process.pid))
  expect(Number(lines[1])).toBeGreaterThan(0)

  expect(conn.getDbPath()).toBe(join(mainDir, "subtrack.db"))
})

// ── saveDb ────────────────────────────────────────────

test("saveDb writes encrypted database with integrity hash", async () => {
  process.env.SUBSC_CLI_DB_DIR = mainDir

  const db = conn.getDb()
  db.exec("CREATE TABLE IF NOT EXISTS t (x INTEGER)")
  db.exec("INSERT INTO t VALUES (42)")
  conn.saveDb()

  const dbPath = conn.getDbPath()
  const file = readFileSync(dbPath)
  expect(isEncrypted(file)).toBe(true)

  // Reload the encrypted file and verify contents
  const verifyPath = join(mainDir, ".subtrack-verify.db")
  writeFileSync(verifyPath, decryptBuffer(file))
  const loaded = new DatabaseSync(verifyPath, { readOnly: true })
  const row = loaded.prepare("SELECT x FROM t").get() as { x: number } | undefined
  expect(Number(row?.x)).toBe(42)
  loaded.close()
  rmSync(verifyPath, { force: true })

  // Integrity sidecar exists and verifies
  const { verifyDbHash } = await import("../db/integrity.ts")
  expect(existsSync(`${dbPath}.sha256`)).toBe(true)
  expect(verifyDbHash(file, dbPath).ok).toBe(true)

  // Tampering is detected
  const tampered = Buffer.from(file)
  tampered[tampered.length - 1] = (tampered[tampered.length - 1]! ^ 0xff) as number
  expect(verifyDbHash(tampered, dbPath).ok).toBe(false)
})

// ── restoreDb variants ────────────────────────────────

async function makeBackupBytes(): Promise<Buffer> {
  const srcPath = join(mainDir, `.subtrack-src-${Date.now()}.db`)
  const backup = new DatabaseSync(srcPath)
  backup.exec("CREATE TABLE subscriptions (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, price INTEGER NOT NULL, currency TEXT NOT NULL, cycle TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'active', billing_day INTEGER, created_at TEXT NOT NULL DEFAULT (date('now')), notes TEXT)")
  backup.exec("CREATE TABLE tags (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE)")
  backup.exec("CREATE TABLE subscription_tags (subscription_id INTEGER NOT NULL, tag_id INTEGER NOT NULL, PRIMARY KEY (subscription_id, tag_id))")
  backup.exec("INSERT INTO subscriptions (name, price, currency, cycle) VALUES ('GzService', 999, 'USD', 'monthly')")
  backup.close()
  const buf = readFileSync(srcPath)
  rmSync(srcPath, { force: true })
  return buf
}

test("restoreDb restores from a gzipped backup (.db.gz)", async () => {
  process.env.SUBSC_CLI_DB_DIR = mainDir

  const backupBytes = await makeBackupBytes()
  const backupPath = join(mainDir, "backup.db.gz")
  writeFileSync(backupPath, gzipSync(backupBytes))

  conn.restoreDb(backupPath)

  const row = conn.getDb().prepare("SELECT name FROM subscriptions WHERE name = 'GzService'").get() as
    | { name: string }
    | undefined
  expect(row).toBeTruthy()
  expect(String(row?.name)).toBe("GzService")
})

test("restoreDb restores from an encrypted backup (.db.enc)", async () => {
  process.env.SUBSC_CLI_DB_DIR = mainDir

  const backupBytes = await makeBackupBytes()
  const backupPath = join(mainDir, "backup.db.enc")
  writeFileSync(backupPath, Buffer.from((await import("@subtrack/lib/crypto")).encryptBuffer(backupBytes)))

  conn.restoreDb(backupPath)

  const row = conn.getDb().prepare("SELECT name FROM subscriptions WHERE name = 'GzService'").get() as
    | { name: string }
    | undefined
  expect(row).toBeTruthy()
  expect(String(row?.name)).toBe("GzService")
})

test("restoreDb rejects an encrypted backup with the wrong key", async () => {
  process.env.SUBSC_CLI_DB_DIR = mainDir

  const backupBytes = await makeBackupBytes()
  const backupPath = join(mainDir, "backup-wrong-key.db.enc")
  // Encrypt with a passphrase-derived key (different from the .key file)
  process.env.SUBSC_CLI_DB_PASSPHRASE = "wrong-passphrase"
  writeFileSync(backupPath, Buffer.from((await import("@subtrack/lib/crypto")).encryptBuffer(backupBytes)))

  // Restore with the default key — decryption must fail
  delete process.env.SUBSC_CLI_DB_PASSPHRASE
  expect(() => conn.restoreDb(backupPath)).toThrow(/Failed to decrypt/i)
})

test("restoreDb rejects a decompression bomb (zip bomb) backup", async () => {
  process.env.SUBSC_CLI_DB_DIR = mainDir

  // Highly-compressible payload that decompresses far beyond the 256 MB cap
  // (gzip of 300 MB of zeros is only a few hundred KB on disk)
  const bomb = gzipSync(Buffer.alloc(300 * 1024 * 1024))
  const backupPath = join(mainDir, "bomb.db.gz")
  writeFileSync(backupPath, bomb)

  expect(() => conn.restoreDb(backupPath)).toThrow(/decompression bomb|refusing/i)
})

test("restoreDb rejects a gzip whose payload decompresses over the cap even when encrypted", async () => {
  process.env.SUBSC_CLI_DB_DIR = mainDir

  // Encrypted variant: decrypts fine, but the inner gzip blows past the cap
  const bomb = gzipSync(Buffer.alloc(300 * 1024 * 1024))
  const encrypted = Buffer.from((await import("@subtrack/lib/crypto")).encryptBuffer(bomb))
  const backupPath = join(mainDir, "bomb.db.enc")
  writeFileSync(backupPath, encrypted)

  expect(() => conn.restoreDb(backupPath)).toThrow(/decompression bomb|refusing/i)
})
