import { test, expect, beforeAll, afterAll, beforeEach } from "vitest"
import { DatabaseSync } from "node:sqlite"
import { mkdtempSync, rmSync, existsSync } from "node:fs"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { consola } from "@subtrack/lib/logger"

import { handleCleanup } from "../cleanup.ts"
import { getAuditLogs } from "../db/audit.ts"

const logMessages: string[] = []
const infoMessages: string[] = []
const successMessages: string[] = []
const errorMessages: string[] = []

let testDb: DatabaseSync
let dbModule: typeof import("../db.ts")
let tmpDir: string
let originalEnv: string | undefined

/** Local-date string N days from now (YYYY-MM-DD). */
function daysFromNow(days: number): string {
  const d = new Date()
  d.setDate(d.getDate() + days)
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`
}

async function captureStdout(fn: () => void): Promise<string> {
  const writes: string[] = []
  const origWrite = process.stdout.write.bind(process.stdout)
  process.stdout.write = ((chunk: string | Uint8Array) => {
    writes.push(String(chunk))
    return true
  }) as typeof process.stdout.write
  try {
    fn()
  } finally {
    process.stdout.write = origWrite
  }
  return writes.join("")
}

/** Insert an old audit entry (N days ago). */
function insertOldAudit(daysAgo: number, action = "usage.add"): void {
  dbModule.getDb().prepare(
    `INSERT INTO audit_log (action, details, created_at) VALUES (?, ?, datetime(?, '${-daysAgo} days'))`,
  ).run(action, "old entry", daysFromNow(0))
}

/** Insert an orphaned tag (not linked to any subscription). */
function insertOrphanTag(name: string): void {
  dbModule.getDb().prepare("INSERT INTO tags (name) VALUES (?)").run(name)
}

beforeAll(async () => {
  tmpDir = mkdtempSync(join(tmpdir(), "subtrack-cleanup-"))
  originalEnv = process.env.SUBSC_CLI_DB_DIR
  process.env.SUBSC_CLI_DB_DIR = tmpDir
  dbModule = await import("../db.ts")
})

afterAll(() => {
  if (originalEnv === undefined) delete process.env.SUBSC_CLI_DB_DIR
  else process.env.SUBSC_CLI_DB_DIR = originalEnv
  if (existsSync(tmpDir)) rmSync(tmpDir, { recursive: true, force: true })
})

beforeEach(async () => {
  logMessages.length = 0
  infoMessages.length = 0
  successMessages.length = 0
  errorMessages.length = 0

  const stripAnsi = (s: string) => s.replace(/\x1b\[[0-9;]*m/g, "")

  consola.mockTypes((_type: string, _defaults: object) => {
    return (...args: unknown[]) => {
      const str = args.map((a) => String(a)).join(" ")
      const clean = stripAnsi(str)
      if (_type === "log") logMessages.push(clean)
      if (_type === "info") infoMessages.push(clean)
      if (_type === "success") successMessages.push(clean)
      if (_type === "error") errorMessages.push(clean)
    }
  })

  testDb = new DatabaseSync(":memory:")
  testDb.exec("PRAGMA foreign_keys = ON")
  const { runMigrations } = await import("../db/schema.ts")
  runMigrations(testDb)
  dbModule.__setDb(testDb)
})

test("handleCleanup passes integrity check and reclaims nothing on a fresh db", () => {
  handleCleanup()
  expect(successMessages).toContain("Integrity check: passed")
  expect(infoMessages).toContain("VACUUM: no space reclaimed (already optimized)")
  expect(infoMessages).toContain("Audit log: nothing to prune")
  expect(infoMessages).toContain("Tags: no orphans found")
  expect(process.exitCode ?? 0).toBe(0)
})

test("handleCleanup prunes old audit entries and orphaned tags", () => {
  insertOldAudit(200)
  insertOrphanTag("orphan")
  handleCleanup()

  expect(successMessages.some((m) => m.includes("Pruned 1 audit log entry older than 90 days"))).toBe(true)
  expect(successMessages.some((m) => m.includes("Removed 1 orphaned tag"))).toBe(true)
  // The cleanup itself is recorded in the audit log
  const audits = getAuditLogs({ action: "cleanup" })
  expect(audits).toHaveLength(1)
  expect(audits[0]!.details).toContain("integrity=ok")
})

test("handleCleanup honors auditDays and leaves newer entries alone", () => {
  insertOldAudit(30)
  handleCleanup({ auditDays: 90 })
  expect(infoMessages.some((m) => m.includes("nothing to prune"))).toBe(true)
})

test("handleCleanup --json outputs structured results", async () => {
  insertOldAudit(200)
  insertOrphanTag("orphan")
  const out = await captureStdout(() => handleCleanup({ json: true }))
  const parsed = JSON.parse(out) as {
    integrityCheck: string
    vacuum: { beforeBytes: number; afterBytes: number; savedBytes: number }
    auditPruned: number
    tagsPruned: number
  }
  expect(parsed.integrityCheck).toBe("passed")
  expect(parsed.vacuum).toHaveProperty("beforeBytes")
  expect(parsed.vacuum).toHaveProperty("afterBytes")
  expect(parsed.auditPruned).toBe(1)
  expect(parsed.tagsPruned).toBe(1)
})

test("handleCleanup with vacuum disabled skips the vacuum step", () => {
  handleCleanup({ vacuum: false })
  expect(successMessages).toContain("Integrity check: passed")
  expect(infoMessages.some((m) => m.includes("Audit log:"))).toBe(true)
})