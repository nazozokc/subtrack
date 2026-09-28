import { test, expect, beforeAll, afterAll, beforeEach } from "vitest"
import { DatabaseSync } from "node:sqlite"
import { mkdtempSync, rmSync, existsSync } from "node:fs"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { consola } from "@subtrack/lib/logger"

import {
  handleAuditList, handleAuditPrune,
} from "../audit.ts"
import { logAudit } from "../audit-log.ts"
import { addAuditLog, getAuditLogs } from "../db/audit.ts"
import type { AddAuditArgs } from "../db/audit.ts"

const logMessages: string[] = []
const infoMessages: string[] = []
const warnMessages: string[] = []
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

/** Rewrite all audit rows to an old created_at (N days ago). */
function ageAuditRows(days: number): void {
  dbModule.getDb().prepare(
    `UPDATE audit_log SET created_at = datetime(?, '${days >= 0 ? "+" : ""}${days} days')`,
  ).run(daysFromNow(0))
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

function insertAudit(args: AddAuditArgs): void {
  addAuditLog(args)
}

beforeAll(async () => {
  tmpDir = mkdtempSync(join(tmpdir(), "subtrack-audit-"))
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
  warnMessages.length = 0
  successMessages.length = 0
  errorMessages.length = 0

  const stripAnsi = (s: string) => s.replace(/\x1b\[[0-9;]*m/g, "")

  consola.mockTypes((_type: string, _defaults: object) => {
    return (...args: unknown[]) => {
      const str = args.map((a) => String(a)).join(" ")
      const clean = stripAnsi(str)
      if (_type === "log") logMessages.push(clean)
      if (_type === "info") infoMessages.push(clean)
      if (_type === "warn") warnMessages.push(clean)
      if (_type === "success") successMessages.push(clean)
      if (_type === "error") errorMessages.push(clean)
    }
  })

  // Fresh in-memory DB with the full schema
  testDb = new DatabaseSync(":memory:")
  testDb.exec("PRAGMA foreign_keys = ON")
  const { runMigrations } = await import("../db/schema.ts")
  runMigrations(testDb)
  dbModule.__setDb(testDb)
})

// ── logAudit (audit-log.ts) ──────────────────────────────

test("logAudit writes an audit entry", () => {
  logAudit("subscription.add", { targetType: "subscription", targetId: 7, details: "created" })
  const entries = getAuditLogs({})
  expect(entries).toHaveLength(1)
  expect(entries[0]!.action).toBe("subscription.add")
  expect(entries[0]!.targetId).toBe(7)
  expect(entries[0]!.details).toBe("created")
})

// ── handleAuditList ──────────────────────────────────────

test("handleAuditList shows a message when there are no entries", () => {
  handleAuditList({})
  expect(infoMessages).toContain("No audit log entries found")
})

test("handleAuditList renders a table with action labels and counts", () => {
  insertAudit({ action: "subscription.add", targetType: "subscription", targetId: 1, details: "added Netflix" })
  insertAudit({ action: "config.set", targetType: "config", details: "defaultCurrency=JPY" })
  handleAuditList({})
  expect(infoMessages).toHaveLength(0)
  const table = logMessages.join("\n")
  expect(table).toContain("ID")
  expect(table).toContain("Add")
  expect(table).toContain("Config Set")
  expect(table).toContain("subscription #1")
  expect(table).toContain("added Netflix")
  expect(table).toContain("2 total entries · showing 2")
})

test("handleAuditList falls back to the raw action for unknown actions", () => {
  insertAudit({ action: "foo.bar", details: "x" })
  handleAuditList({})
  expect(logMessages.join("\n")).toContain("foo.bar")
})

test("handleAuditList filters by action when requested", () => {
  insertAudit({ action: "subscription.add", details: "a" })
  insertAudit({ action: "subscription.delete", details: "b" })
  handleAuditList({ action: "subscription.delete" })
  const table = logMessages.join("\n")
  expect(table).toContain("Delete")
  expect(table).not.toContain("Delete\na")
  expect(table).toContain("1 total entries · showing 1")
})

test("handleAuditList respects the limit option", () => {
  for (let i = 0; i < 5; i++) {
    insertAudit({ action: "usage.add", details: `entry ${i}` })
  }
  handleAuditList({ limit: 2 })
  expect(logMessages.join("\n")).toContain("5 total entries · showing 2")
})

test("handleAuditList outputs raw JSON with --json", async () => {
  insertAudit({ action: "subscription.add", details: "json test" })
  const out = await captureStdout(() => handleAuditList({ json: true }))
  const parsed = JSON.parse(out) as Array<{ action: string; details: string | null }>
  expect(parsed).toHaveLength(1)
  expect(parsed[0]!.action).toBe("subscription.add")
  expect(parsed[0]!.details).toBe("json test")
})

// ── handleAuditPrune ─────────────────────────────────────

test("handleAuditPrune reports when nothing is old enough", () => {
  insertAudit({ action: "subscription.add", details: "fresh" })
  handleAuditPrune({})
  expect(infoMessages).toContain("No audit log entries older than 90 days")
})

test("handleAuditPrune warns without --force and deletes with it", () => {
  insertAudit({ action: "subscription.add", details: "old" })
  ageAuditRows(-200)

  handleAuditPrune({})
  expect(warnMessages.some((m) => m.includes("This will delete 1 audit log entry older than 90 days"))).toBe(true)
  expect(getAuditLogs({})).toHaveLength(1)

  handleAuditPrune({ force: true })
  expect(successMessages.some((m) => m.includes("Pruned 1 audit log entry"))).toBe(true)
  expect(getAuditLogs({})).toHaveLength(0)
})

test("handleAuditPrune honors a custom days window", () => {
  insertAudit({ action: "usage.add", details: "30d old" })
  ageAuditRows(-30)
  // 30 days old is outside a 7-day window but inside the default 90-day one
  handleAuditPrune({ days: 7 })
  expect(warnMessages.some((m) => m.includes("older than 7 days"))).toBe(true)
  handleAuditPrune({ days: 7, force: true })
  expect(getAuditLogs({})).toHaveLength(0)
})

test("handleAuditPrune pluralizes the count correctly", () => {
  insertAudit({ action: "usage.add", details: "one" })
  insertAudit({ action: "usage.delete", details: "two" })
  ageAuditRows(-200)
  handleAuditPrune({ force: true })
  expect(successMessages.some((m) => m.includes("Pruned 2 audit log entries"))).toBe(true)
})