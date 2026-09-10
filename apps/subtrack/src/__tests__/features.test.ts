import { test, expect, beforeAll, afterAll, beforeEach, afterEach, vi } from "vitest"
import initSqlJs from "sql.js"
import type { Database } from "sql.js"
import { mkdtempSync, writeFileSync, existsSync, rmSync } from "node:fs"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { consola } from "consola"

vi.mock("@inquirer/prompts", () => ({
  input: vi.fn(),
  confirm: vi.fn(),
  checkbox: vi.fn(),
  select: vi.fn(),
  search: vi.fn(),
}))

import { confirm, select } from "@inquirer/prompts"

import {
  handlePause, handleResume, handleRenew, handleReview,
  handleYearly, handleCheck, handleChanges, handleReceipt, handleTemplate,
} from "../features.ts"

const logMessages: string[] = []
const infoMessages: string[] = []
const successMessages: string[] = []
const errorMessages: string[] = []

let testDb: Database
let dbModule: typeof import("../db.ts")
let tmpDir: string
let originalEnv: string | undefined

async function captureStdout(fn: () => Promise<void> | void): Promise<string> {
  const writes: string[] = []
  const origWrite = process.stdout.write.bind(process.stdout)
  process.stdout.write = ((chunk: string | Uint8Array) => {
    writes.push(String(chunk))
    return true
  }) as typeof process.stdout.write
  try {
    await fn()
  } finally {
    process.stdout.write = origWrite
  }
  return writes.join("")
}

/** Local-date string N days from now (YYYY-MM-DD). */
function daysFromNow(days: number): string {
  const d = new Date()
  d.setDate(d.getDate() + days)
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`
}

function insertSub(overrides: Record<string, unknown> = {}): number {
  const db = dbModule.getDb()
  const fields = {
    name: "Test Sub",
    price: 1000,
    currency: "JPY",
    cycle: "monthly",
    status: "active",
    billingDay: 1,
    createdAt: "2026-01-01",
    notes: null,
    paymentMethod: null,
    contractStart: null,
    contractEnd: null,
    autoRenewal: 1,
    ...overrides,
  }
  db.run(
    `INSERT INTO subscriptions (name, price, currency, cycle, status, billing_day, created_at, notes, payment_method, contract_start, contract_end, auto_renewal)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    [fields.name, fields.price, fields.currency, fields.cycle, fields.status, fields.billingDay, fields.createdAt, fields.notes, fields.paymentMethod, fields.contractStart, fields.contractEnd, fields.autoRenewal],
  )
  const row = db.exec("SELECT last_insert_rowid() AS id")
  return Number(row[0].values[0][0])
}

function insertTrial(overrides: Record<string, unknown> = {}): number {
  const db = dbModule.getDb()
  const fields = { name: "Trial Sub", expiresAt: daysFromNow(10), price: 500, currency: "JPY", cycle: "monthly", notes: null, createdAt: "2026-01-01", ...overrides }
  db.run(
    "INSERT INTO trials (name, expires_at, price, currency, cycle, notes, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
    [fields.name, fields.expiresAt, fields.price, fields.currency, fields.cycle, fields.notes, fields.createdAt],
  )
  const row = db.exec("SELECT last_insert_rowid() AS id")
  return Number(row[0].values[0][0])
}

beforeAll(async () => {
  const SQL = await initSqlJs()
  tmpDir = mkdtempSync(join(tmpdir(), "subtrack-features-"))
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
  // Fresh in-memory DB with the full schema
  const SQL = await initSqlJs()
  testDb = new SQL.Database()
  testDb.run("PRAGMA foreign_keys = ON")
  const { runMigrations } = await import("../db/schema.ts")
  runMigrations(testDb)
  dbModule.__setDb(testDb)

  // Isolated config (templates persist to config.json)
  const { resetConfig, getConfigPath } = await import("../config.ts")
  resetConfig()
  if (existsSync(getConfigPath())) rmSync(getConfigPath())

  logMessages.length = 0
  infoMessages.length = 0
  successMessages.length = 0
  errorMessages.length = 0
  process.exitCode = 0

  consola.mockTypes((type: string, _defaults: object) => {
    const stripAnsi = (s: string) => s.replace(/\x1b\[[0-9;]*m/g, "")
    return (...args: unknown[]) => {
      const str = args.map((a) => String(a)).join(" ")
      const clean = stripAnsi(str)
      if (type === "log") logMessages.push(clean)
      else if (type === "info") infoMessages.push(clean)
      else if (type === "success") successMessages.push(clean)
      else if (type === "error" || type === "fail") errorMessages.push(clean)
    }
  })
})

afterEach(() => {
  consola.mockTypes()
})

// ── pause / resume ─────────────────────────────────────

test("handlePause pauses the selected subscription with force", async () => {
  const id = insertSub({ name: "Netflix" })
  await handlePause([id], true)
  expect(dbModule.getSubscription(id)?.status).toBe("paused")
  expect(successMessages.some((m) => m.includes("Updated 1"))).toBe(true)
  expect(dbModule.getAuditLogs({ action: "subscription.pause" }).length).toBe(1)
  expect(confirm).not.toHaveBeenCalled()
})

test("handlePause asks for confirmation and aborts when declined", async () => {
  const id = insertSub({ name: "Netflix" })
  vi.mocked(confirm).mockResolvedValueOnce(false)
  await handlePause([id], false)
  expect(dbModule.getSubscription(id)?.status).toBe("active")
  expect(infoMessages.some((m) => m.includes("Cancelled"))).toBe(true)
})

test("handleResume reactivates a paused subscription", async () => {
  const id = insertSub({ name: "Netflix", status: "paused" })
  await handleResume([id], true)
  expect(dbModule.getSubscription(id)?.status).toBe("active")
  expect(dbModule.getAuditLogs({ action: "subscription.resume" }).length).toBe(1)
})

test("handlePause is a no-op when nothing needs changing", async () => {
  const id = insertSub({ name: "Netflix", status: "archived" })
  await handlePause([id], true)
  expect(infoMessages.some((m) => m.includes("No subscriptions need changing"))).toBe(true)
  expect(dbModule.getSubscription(id)?.status).toBe("archived")
})

test("handlePause on unknown id reports no subs to change", async () => {
  await handlePause([999], true)
  expect(infoMessages.some((m) => m.includes("No subscriptions need changing"))).toBe(true)
})

// ── renew ──────────────────────────────────────────────

test("handleRenew fails for a missing subscription", async () => {
  await handleRenew(999, {})
  expect(errorMessages.some((m) => m.includes("not found"))).toBe(true)
  expect(process.exitCode).toBe(1)
})

test("handleRenew updates the price and records price history and audit", async () => {
  const id = insertSub({ name: "Netflix" })
  await handleRenew(id, { price: "1080" })
  const sub = dbModule.getSubscription(id)
  expect(sub?.price).toBe(1080)
  expect(sub?.status).toBe("active")
  const history = dbModule.getPriceHistory(id)
  expect(history.length).toBe(1)
  expect(history[0]?.oldPrice).toBe(1000)
  expect(history[0]?.newPrice).toBe(1080)
  expect(dbModule.getAuditLogs({ action: "subscription.renew", targetId: id }).length).toBe(1)
  expect(successMessages.some((m) => m.includes("Renewed: Netflix"))).toBe(true)
})

test("handleRenew with unchanged price writes no price history", async () => {
  const id = insertSub({ name: "Netflix" })
  await handleRenew(id, { price: "1000" })
  expect(dbModule.getPriceHistory(id).length).toBe(0)
})

test("handleRenew rejects a negative price", async () => {
  const id = insertSub({ name: "Netflix" })
  await handleRenew(id, { price: "-1" })
  expect(errorMessages.some((m) => m.includes("non-negative"))).toBe(true)
  expect(dbModule.getSubscription(id)?.price).toBe(1000)
})

test("handleRenew accepts cycle and contract flags", async () => {
  const id = insertSub({ name: "Netflix" })
  await handleRenew(id, { cycle: "yearly", contractEnd: "2027-01-01" })
  const sub = dbModule.getSubscription(id)
  expect(sub?.cycle).toBe("yearly")
  expect(sub?.contractEnd).toBe("2027-01-01")
})

// ── template ───────────────────────────────────────────

test("handleTemplate add stores a template with full fields", async () => {
  handleTemplate("add", "spotify", { price: 980, currency: "JPY", cycle: "monthly", tags: ["music"] })
  expect(successMessages.some((m) => m.includes("Added template: spotify"))).toBe(true)
  handleTemplate("list")
  expect(logMessages.some((m) => m.includes("spotify: 980 JPY/monthly"))).toBe(true)
  expect(dbModule.getAuditLogs({ action: "template.add" }).length).toBe(1)
})

test("handleTemplate add defaults currency and cycle instead of storing undefined", async () => {
  handleTemplate("add", "basic", { price: 500 })
  expect(successMessages.some((m) => m.includes("Added template: basic"))).toBe(true)
  handleTemplate("list")
  expect(logMessages.some((m) => m.includes("basic: 500 USD/monthly"))).toBe(true)
  expect(logMessages.some((m) => m.includes("undefined"))).toBe(false)
})

test("handleTemplate add rejects a missing or NaN price", async () => {
  handleTemplate("add", "bad", { price: NaN })
  expect(errorMessages.some((m) => m.includes("Template price must be a positive number"))).toBe(true)
  handleTemplate("add", "bad")
  expect(errorMessages.some((m) => m.includes("Template price must be a positive number"))).toBe(true)
  handleTemplate("list")
  expect(infoMessages.some((m) => m.includes("No templates found"))).toBe(true)
})

test("handleTemplate add rejects an invalid cycle", async () => {
  handleTemplate("add", "bad", { price: 100, cycle: "lemon" as never })
  expect(errorMessages.some((m) => m.includes("Invalid cycle: lemon"))).toBe(true)
})

test("handleTemplate add rejects an out-of-range billing day", async () => {
  handleTemplate("add", "bad", { price: 100, billingDay: 32 })
  expect(errorMessages.some((m) => m.includes("billingDay"))).toBe(true)
})

test("handleTemplate add rejects duplicates", async () => {
  handleTemplate("add", "spotify", { price: 980 })
  handleTemplate("add", "spotify", { price: 1080 })
  expect(errorMessages.some((m) => m.includes("Template already exists: spotify"))).toBe(true)
})

test("handleTemplate edit updates fields", async () => {
  handleTemplate("add", "spotify", { price: 980, currency: "JPY", cycle: "monthly" })
  handleTemplate("edit", "spotify", { price: 1080 })
  expect(successMessages.some((m) => m.includes("Updated template: spotify"))).toBe(true)
  handleTemplate("list")
  expect(logMessages.some((m) => m.includes("spotify: 1080 JPY/monthly"))).toBe(true)
  expect(dbModule.getAuditLogs({ action: "template.edit" }).length).toBe(1)
})

test("handleTemplate delete removes a template and fails for unknown ones", async () => {
  handleTemplate("add", "spotify", { price: 980 })
  handleTemplate("delete", "spotify")
  expect(successMessages.some((m) => m.includes("Deleted template: spotify"))).toBe(true)
  expect(dbModule.getAuditLogs({ action: "template.delete" }).length).toBe(1)
  handleTemplate("list")
  expect(infoMessages.some((m) => m.includes("No templates found"))).toBe(true)

  handleTemplate("delete", "ghost")
  expect(errorMessages.some((m) => m.includes("Template not found: ghost"))).toBe(true)
})

test("handleTemplate use creates a subscription from the template", async () => {
  handleTemplate("add", "spotify", { price: 980, currency: "JPY", cycle: "monthly", tags: ["music"] })
  handleTemplate("use", "spotify")
  expect(successMessages.some((m) => m.includes("Added subscription from template: spotify"))).toBe(true)
  const subs = dbModule.getSubscriptions()
  expect(subs.length).toBe(1)
  expect(subs[0]?.name).toBe("spotify")
  expect(subs[0]?.price).toBe(980)
  expect(subs[0]?.currency).toBe("JPY")
  expect(subs[0]?.cycle).toBe("monthly")
  expect(dbModule.getAuditLogs({ action: "template.use" }).length).toBe(1)
})

test("handleTemplate use fails for an unknown template", async () => {
  handleTemplate("use", "ghost")
  expect(errorMessages.some((m) => m.includes("Template not found: ghost"))).toBe(true)
})

// ── check ──────────────────────────────────────────────

test("handleCheck reports no problems for a healthy record", () => {
  insertSub({ name: "Netflix", billingDay: 5 })
  handleCheck()
  expect(successMessages.some((m) => m.includes("No problems found"))).toBe(true)
})

test("handleCheck flags a missing billing day", () => {
  insertSub({ name: "Netflix", billingDay: null })
  handleCheck()
  expect(logMessages.some((m) => m.includes("no billing day"))).toBe(true)
})

test("handleCheck flags invalid and expired contracts", () => {
  insertSub({ name: "Bad Range", contractStart: daysFromNow(10), contractEnd: daysFromNow(5) })
  insertSub({ name: "Old", contractEnd: daysFromNow(-3) })
  handleCheck()
  expect(logMessages.some((m) => m.includes("error") && m.includes("contract start is after contract end"))).toBe(true)
  expect(logMessages.some((m) => m.includes("warning") && m.includes("contract ended on"))).toBe(true)
})

test("handleCheck flags duplicate subscription names", () => {
  insertSub({ name: "Netflix" })
  insertSub({ name: " netflix " })
  handleCheck()
  expect(logMessages.some((m) => m.includes("Duplicate subscription name"))).toBe(true)
})

test("handleCheck outputs findings as JSON", async () => {
  insertSub({ name: "Netflix", billingDay: null })
  const out = await captureStdout(() => handleCheck({ json: true }))
  const parsed = JSON.parse(out) as Array<{ code: string }>
  expect(parsed.some((f) => f.code === "missing-billing-day")).toBe(true)
})

test("handleCheck with strict exits non-zero when findings exist", () => {
  insertSub({ name: "Netflix", billingDay: null })
  handleCheck({ strict: true })
  expect(process.exitCode).toBe(1)
})

// ── changes ────────────────────────────────────────────

test("handleChanges shows price changes", async () => {
  const id = insertSub({ name: "Netflix" })
  dbModule.writePriceHistory(id, 1000, 1500, "JPY", "JPY")
  const out = await captureStdout(() => handleChanges({ json: true }))
  const parsed = JSON.parse(out) as Array<{ type: string; details: string }>
  expect(parsed.some((e) => e.type === "price" && e.details.includes("Netflix: 1000 → 1500"))).toBe(true)
})

test("handleChanges shows audit entries", async () => {
  const id = insertSub({ name: "Netflix" })
  const { logAudit } = await import("../audit.ts")
  logAudit("subscription.pause", { targetType: "subscription", targetId: id, details: "paused in review" })
  const out = await captureStdout(() => handleChanges({ json: true }))
  const parsed = JSON.parse(out) as Array<{ type: string; details: string }>
  expect(parsed.some((e) => e.type === "audit" && e.details.includes("paused in review"))).toBe(true)
})

test("handleChanges id filter finds entries beyond the 50 most recent audit rows", async () => {
  const subA = insertSub({ name: "Sub A" })
  const subB = insertSub({ name: "Sub B" })
  const { logAudit } = await import("../audit.ts")
  logAudit("subscription.pause", { targetType: "subscription", targetId: subA, details: "old-entry" })
  for (let i = 0; i < 60; i++) {
    logAudit("subscription.renew", { targetType: "subscription", targetId: subB, details: `b-${i}` })
  }
  const out = await captureStdout(() => handleChanges({ id: subA, json: true }))
  const parsed = JSON.parse(out) as Array<{ type: string; id: number | null; details: string }>
  expect(parsed.some((e) => e.type === "audit" && e.details.includes("old-entry"))).toBe(true)
  expect(parsed.every((e) => e.type !== "audit" || e.id === subA)).toBe(true)
})

test("handleChanges respects the limit", async () => {
  const id = insertSub({ name: "Netflix" })
  dbModule.writePriceHistory(id, 1000, 1100, "JPY", "JPY")
  dbModule.writePriceHistory(id, 1100, 1200, "JPY", "JPY")
  const out = await captureStdout(() => handleChanges({ limit: 1, json: true }))
  const parsed = JSON.parse(out) as unknown[]
  expect(parsed.length).toBe(1)
})

test("handleChanges reports when nothing changed", async () => {
  handleChanges()
  expect(infoMessages.some((m) => m.includes("No changes found"))).toBe(true)
})

// ── review ─────────────────────────────────────────────

test("handleReview json lists subscriptions with contracts ending soon", async () => {
  insertSub({ name: "Netflix", contractEnd: daysFromNow(30) })
  insertSub({ name: "Safe", contractEnd: daysFromNow(400) })
  const out = await captureStdout(() => handleReview({ json: true }))
  const parsed = JSON.parse(out) as Array<{ name: string; kind: string; reason: string }>
  const item = parsed.find((e) => e.name === "Netflix")
  expect(item).toBeDefined()
  expect(item?.kind).toBe("subscription")
  expect(item?.reason).toContain("contract ends")
  expect(parsed.some((e) => e.name === "Safe")).toBe(false)
})

test("handleReview json is empty when nothing needs review", async () => {
  insertSub({ name: "Safe", billingDay: null })
  const out = await captureStdout(() => handleReview({ json: true }))
  expect(JSON.parse(out)).toHaveLength(0)
})

test("handleReview archive action archives the subscription", async () => {
  const id = insertSub({ name: "Netflix", contractEnd: daysFromNow(30) })
  vi.mocked(select).mockResolvedValueOnce("archive")
  const { handleReview } = await import("../review.ts")
  await handleReview()
  expect(dbModule.getSubscription(id)?.status).toBe("archived")
  expect(dbModule.getAuditLogs({ action: "subscription.archive", targetId: id }).length).toBe(1)
})

test("handleReview pause action pauses the subscription", async () => {
  const id = insertSub({ name: "Netflix", contractEnd: daysFromNow(30) })
  vi.mocked(select).mockResolvedValueOnce("pause")
  const { handleReview } = await import("../review.ts")
  await handleReview()
  expect(dbModule.getSubscription(id)?.status).toBe("paused")
})

test("handleReview dismiss action removes an expiring trial", async () => {
  insertTrial({ name: "Trialify" })
  vi.mocked(select).mockResolvedValueOnce("dismiss")
  const { handleReview } = await import("../review.ts")
  await handleReview()
  expect(dbModule.getTrials().length).toBe(0)
})

test("handleReview add action converts a trial into a subscription", async () => {
  insertTrial({ name: "Trialify", price: 980, currency: "JPY", cycle: "monthly", notes: "from trial" })
  vi.mocked(select).mockResolvedValueOnce("add")
  const { handleReview } = await import("../review.ts")
  await handleReview()
  expect(dbModule.getTrials().length).toBe(0)
  const subs = dbModule.getSubscriptions()
  expect(subs.length).toBe(1)
  expect(subs[0]?.name).toBe("Trialify")
  expect(subs[0]?.price).toBe(980)
  expect(subs[0]?.currency).toBe("JPY")
})

test("handleReview reports when nothing needs review", async () => {
  insertSub({ name: "Safe", billingDay: null })
  await handleReview()
  expect(infoMessages.some((m) => m.includes("Nothing needs review"))).toBe(true)
})

// ── yearly ─────────────────────────────────────────────

test("handleYearly outputs the annual total for the year", async () => {
  insertSub({ name: "Netflix", price: 1200, currency: "JPY", createdAt: "2026-01-01" })
  const out = await captureStdout(() => handleYearly(2026, undefined, true))
  const parsed = JSON.parse(out) as { year: number; total: number; byCurrency: Record<string, number> }
  expect(parsed.year).toBe(2026)
  expect(parsed.total).toBe(14400)
  expect(parsed.byCurrency.JPY).toBe(14400)
})

// ── receipt ────────────────────────────────────────────

function writeReceipt(name: string, text: string): string {
  const file = join(tmpDir, name)
  writeFileSync(file, text, "utf8")
  return file
}

test("handleReceipt imports candidates and records an audit entry", async () => {
  const file = writeReceipt("receipt.txt", "Netflix subscription\nTotal: $9.99 monthly\nThank you for your payment.")
  await handleReceipt(file)
  expect(successMessages.some((m) => m.includes("Imported 1 receipt candidate"))).toBe(true)
  expect(dbModule.getSuggestions().length).toBe(1)
  expect(dbModule.getAuditLogs({ action: "suggestion.receipt" }).length).toBe(1)
})

test("handleReceipt dryRun previews without writing", async () => {
  const file = writeReceipt("dry.txt", "Total: ¥1,980 monthly")
  const out = await captureStdout(() => handleReceipt(file, { dryRun: true }))
  const parsed = JSON.parse(out) as Array<{ price: number; currency: string }>
  expect(parsed.length).toBe(1)
  expect(parsed[0]?.currency).toBe("JPY")
  expect(dbModule.getSuggestions().length).toBe(0)
})

test("handleReceipt json output is a pure preview like dryRun", async () => {
  const file = writeReceipt("json.txt", "Total: $9.99 monthly")
  const out = await captureStdout(() => handleReceipt(file, { json: true }))
  const parsed = JSON.parse(out) as unknown[]
  expect(parsed.length).toBe(1)
  expect(dbModule.getSuggestions().length).toBe(0)
  expect(successMessages.length).toBe(0)
})

test("handleReceipt fails for a missing file", async () => {
  await handleReceipt(join(tmpDir, "nope.txt"))
  expect(errorMessages.some((m) => m.includes("Failed to read receipt"))).toBe(true)
  expect(process.exitCode).toBe(1)
})

test("handleReceipt accepts a JSON array of emails", async () => {
  const file = writeReceipt("emails.json", JSON.stringify([{ subject: "Spotify", body: "Total: $9.99 monthly" }]))
  await handleReceipt(file)
  expect(dbModule.getSuggestions().length).toBe(1)
})