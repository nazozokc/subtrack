import { test, expect, beforeAll, afterAll, beforeEach, vi } from "vitest"
import { DatabaseSync } from "node:sqlite"

// Mock consola to capture output
vi.mock("@subtrack/lib/logger", () => {
  const logMessages: string[] = []
  const infoMessages: string[] = []
  const successMessages: string[] = []
  const errorMessages: string[] = []
  const failMessages: string[] = []
  const warnMessages: string[] = []

  const _consola = { logMessages, infoMessages, successMessages, errorMessages, failMessages, warnMessages }

  const makeFn = (arr: string[]) => (...args: unknown[]) => {
    const str = args.map((a) => String(a)).join(" ")
    arr.push(str)
  }

  return {
    ..._consola,
    default: {
      log: makeFn(logMessages),
      info: makeFn(infoMessages),
      success: makeFn(successMessages),
      error: makeFn(errorMessages),
      fail: makeFn(failMessages),
      warn: makeFn(warnMessages),
    },
    consola: {
      log: makeFn(logMessages),
      info: makeFn(infoMessages),
      success: makeFn(successMessages),
      error: makeFn(errorMessages),
      fail: makeFn(failMessages),
      warn: makeFn(warnMessages),
    },
  }
})

// Mock prompts.ts to avoid interactive prompts
vi.mock("../prompts.ts", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../prompts.ts")>()
  const { createPromptMock } = await import("./prompt-mock.ts")
  return createPromptMock(actual)
})

import { checkbox, confirm } from "../prompts.ts"
import { logMessages, infoMessages, successMessages, errorMessages, failMessages, warnMessages } from "@subtrack/lib/logger"
import { runMigrations } from "../db/schema.ts"

let testDb: DatabaseSync

function resetMessages() {
  logMessages.length = 0
  infoMessages.length = 0
  successMessages.length = 0
  errorMessages.length = 0
  failMessages.length = 0
  warnMessages.length = 0
}

beforeAll(async () => {
  testDb = new DatabaseSync(":memory:")
  testDb.exec("PRAGMA foreign_keys = ON")
  runMigrations(testDb)

  const { __setDb } = await import("../db.ts")
  __setDb(testDb)
})

beforeEach(async () => {
  const db = await import("../db.ts")
  testDb = db.getDb()

  testDb.exec("DELETE FROM audit_log")
  testDb.exec("DELETE FROM price_history")
  testDb.exec("DELETE FROM subscription_tags")
  testDb.exec("DELETE FROM tags")
  testDb.exec("DELETE FROM subscriptions")
  testDb.exec("DELETE FROM sqlite_sequence")

  resetMessages()
  process.exitCode = 0

  vi.mocked(confirm).mockReset().mockResolvedValue(true)
  vi.mocked(checkbox).mockReset().mockResolvedValue([])
})

afterAll(async () => {
  const db = await import("../db.ts")
  try { db.getDb().close() } catch { /* may be closed */ }
})

// ── handleDelete ─────────────────────────────────────────

test("handleDelete deletes explicit ids", async () => {
  const db = await import("../db.ts")
  const { handleDelete } = await import("../subscription/core.ts")
  db.writeSubscription({ name: "S1", price: 100, currency: "USD", cycle: "monthly", tags: [], createdAt: "2026-01-01" })
  db.writeSubscription({ name: "S2", price: 200, currency: "USD", cycle: "monthly", tags: [], createdAt: "2026-01-01" })
  const [s1] = db.getSubscriptions()

  await handleDelete([s1.id])

  const remaining = db.getSubscriptions()
  expect(remaining).toHaveLength(1)
  expect(remaining[0].name).toBe("S2")
  expect(successMessages.some((m) => m.includes("S1"))).toBe(true)
})

test("handleDelete with non-existent id calls fail and deletes nothing", async () => {
  const db = await import("../db.ts")
  const { handleDelete } = await import("../subscription/core.ts")
  db.writeSubscription({ name: "S1", price: 100, currency: "USD", cycle: "monthly", tags: [], createdAt: "2026-01-01" })

  await handleDelete([999])

  expect(errorMessages.some((m) => m.includes("not found"))).toBe(true)
  expect(process.exitCode).toBe(1)
  expect(db.getSubscriptions()).toHaveLength(1)
})

test("handleDelete deletes multiple ids in order", async () => {
  const db = await import("../db.ts")
  const { handleDelete } = await import("../subscription/core.ts")
  db.writeSubscription({ name: "S1", price: 100, currency: "USD", cycle: "monthly", tags: [], createdAt: "2026-01-01" })
  db.writeSubscription({ name: "S2", price: 200, currency: "USD", cycle: "monthly", tags: [], createdAt: "2026-01-01" })
  db.writeSubscription({ name: "S3", price: 300, currency: "USD", cycle: "monthly", tags: [], createdAt: "2026-01-01" })
  const [s1, , s3] = db.getSubscriptions()

  await handleDelete([s1.id, s3.id])

  const remaining = db.getSubscriptions()
  expect(remaining).toHaveLength(1)
  expect(remaining[0].name).toBe("S2")
  expect(successMessages.some((m) => m.includes("S1"))).toBe(true)
  expect(successMessages.some((m) => m.includes("S3"))).toBe(true)
})

test("handleDelete interactive passes all subs to checkbox and deletes selection", async () => {
  const db = await import("../db.ts")
  const { handleDelete } = await import("../subscription/core.ts")
  db.writeSubscription({ name: "S1", price: 100, currency: "USD", cycle: "monthly", tags: [], createdAt: "2026-01-01" })
  db.writeSubscription({ name: "S2", price: 200, currency: "USD", cycle: "monthly", tags: [], createdAt: "2026-01-01" })
  db.writeSubscription({ name: "S3", price: 300, currency: "USD", cycle: "monthly", tags: [], createdAt: "2026-01-01" })
  const [s1, s2] = db.getSubscriptions()

  vi.mocked(checkbox).mockResolvedValue([s1, s2])
  vi.mocked(confirm).mockResolvedValue(true)

  await handleDelete()

  const choices = vi.mocked(checkbox).mock.calls[0][0]?.choices
  expect(choices).toHaveLength(3)
  expect(choices.map((c) => (c.value as { name: string }).name)).toEqual(["S1", "S2", "S3"])

  const remaining = db.getSubscriptions()
  expect(remaining).toHaveLength(1)
  expect(remaining[0].name).toBe("S3")
  expect(successMessages.some((m) => m.includes("S2"))).toBe(true)
})

// ── handleClone ──────────────────────────────────────────

test("handleClone duplicates an existing subscription with (copy)", async () => {
  const db = await import("../db.ts")
  const { handleClone } = await import("../subscription/core.ts")
  db.writeSubscription({ name: "Netflix", price: 100, currency: "USD", cycle: "monthly", tags: ["video"], createdAt: "2026-01-01" })
  const [orig] = db.getSubscriptions()

  await handleClone(orig.id)

  const all = db.getSubscriptions()
  expect(all).toHaveLength(2)
  expect(all[1].name).toBe("Netflix (copy)")
  expect(all[1].price).toBe(100)
  expect(all[1].currency).toBe("USD")
  expect(all[1].cycle).toBe("monthly")
  expect(all[1].status).toBe("active")
  expect(successMessages.some((m) => m.includes("Netflix (copy)"))).toBe(true)
})

test("handleClone uses custom name from flags", async () => {
  const db = await import("../db.ts")
  const { handleClone } = await import("../subscription/core.ts")
  db.writeSubscription({ name: "Netflix", price: 100, currency: "USD", cycle: "monthly", tags: [], createdAt: "2026-01-01" })
  const [orig] = db.getSubscriptions()

  await handleClone(orig.id, { name: "Netflix Family" })

  const all = db.getSubscriptions()
  expect(all.map((s) => s.name)).toEqual(["Netflix", "Netflix Family"])
  expect(successMessages.some((m) => m.includes("Netflix Family"))).toBe(true)
})

test("handleClone changes price with flags", async () => {
  const db = await import("../db.ts")
  const { handleClone } = await import("../subscription/core.ts")
  db.writeSubscription({ name: "Netflix", price: 100, currency: "USD", cycle: "monthly", tags: [], createdAt: "2026-01-01" })
  const [orig] = db.getSubscriptions()

  await handleClone(orig.id, { price: "1500" })

  const all = db.getSubscriptions()
  expect(all).toHaveLength(2)
  expect(all[1].price).toBe(1500)
})

test("handleClone with non-existent id calls fail", async () => {
  const db = await import("../db.ts")
  const { handleClone } = await import("../subscription/core.ts")

  await handleClone(999)

  expect(errorMessages.some((m) => m.includes("999"))).toBe(true)
  expect(process.exitCode).toBe(1)
  expect(db.getSubscriptions()).toHaveLength(0)
})

test("handleClone with invalid price calls fail", async () => {
  const db = await import("../db.ts")
  const { handleClone } = await import("../subscription/core.ts")
  db.writeSubscription({ name: "Netflix", price: 100, currency: "USD", cycle: "monthly", tags: [], createdAt: "2026-01-01" })
  const [orig] = db.getSubscriptions()

  await handleClone(orig.id, { price: "abc" })

  expect(errorMessages.some((m) => m.includes("Invalid price"))).toBe(true)
  expect(process.exitCode).toBe(1)
  expect(db.getSubscriptions()).toHaveLength(1)
})

// ── handleArchive ────────────────────────────────────────

test("handleArchive archives a subscription", async () => {
  const db = await import("../db.ts")
  const { handleArchive } = await import("../subscription/core.ts")
  const id = db.writeSubscription({ name: "S1", price: 100, currency: "USD", cycle: "monthly", tags: [], createdAt: "2026-01-01" })

  handleArchive(id)

  expect(db.getSubscription(id)?.status).toBe("archived")
  expect(successMessages.some((m) => m.includes("S1"))).toBe(true)
})

test("handleArchive is a no-op for already archived subscription", async () => {
  const db = await import("../db.ts")
  const { handleArchive } = await import("../subscription/core.ts")
  const id = db.writeSubscription({ name: "S1", price: 100, currency: "USD", cycle: "monthly", tags: [], status: "archived", createdAt: "2026-01-01" })

  handleArchive(id)

  expect(infoMessages.some((m) => m.includes("already archived"))).toBe(true)
  expect(successMessages.length).toBe(0)
  expect(db.getSubscription(id)?.status).toBe("archived")
})

test("handleArchive with non-existent id calls fail", async () => {
  const { handleArchive } = await import("../subscription/core.ts")

  handleArchive(999)

  expect(errorMessages.some((m) => m.includes("not found"))).toBe(true)
  expect(process.exitCode).toBe(1)
})

// ── handleUnarchive ──────────────────────────────────────

test("handleUnarchive restores an archived subscription", async () => {
  const db = await import("../db.ts")
  const { handleUnarchive } = await import("../subscription/core.ts")
  const id = db.writeSubscription({ name: "S1", price: 100, currency: "USD", cycle: "monthly", tags: [], status: "archived", createdAt: "2026-01-01" })

  handleUnarchive(id)

  expect(db.getSubscription(id)?.status).toBe("active")
  expect(successMessages.some((m) => m.includes("S1"))).toBe(true)
})

test("handleUnarchive is a no-op for active subscription", async () => {
  const db = await import("../db.ts")
  const { handleUnarchive } = await import("../subscription/core.ts")
  const id = db.writeSubscription({ name: "S1", price: 100, currency: "USD", cycle: "monthly", tags: [], createdAt: "2026-01-01" })

  handleUnarchive(id)

  expect(infoMessages.some((m) => m.includes("not archived"))).toBe(true)
  expect(successMessages.length).toBe(0)
  expect(db.getSubscription(id)?.status).toBe("active")
})

test("handleUnarchive with non-existent id calls fail", async () => {
  const { handleUnarchive } = await import("../subscription/core.ts")

  handleUnarchive(999)

  expect(errorMessages.some((m) => m.includes("not found"))).toBe(true)
  expect(process.exitCode).toBe(1)
})

// ── handleTags ───────────────────────────────────────────

test("handleTags filters and spreads matching subs", async () => {
  const db = await import("../db.ts")
  const { handleTags } = await import("../subscription/core.ts")
  db.writeSubscription({ name: "A", price: 100, currency: "USD", cycle: "monthly", tags: ["video"], createdAt: "2026-01-01" })
  db.writeSubscription({ name: "B", price: 200, currency: "USD", cycle: "monthly", tags: ["audio"], createdAt: "2026-01-01" })

  await handleTags(["video"])

  const combined = logMessages.join("\n")
  expect(combined).toContain("A")
  expect(combined).not.toContain("B")
})

test("handleTags with empty list is safe", async () => {
  const db = await import("../db.ts")
  const { handleTags } = await import("../subscription/core.ts")
  db.writeSubscription({ name: "A", price: 100, currency: "USD", cycle: "monthly", tags: ["video"], createdAt: "2026-01-01" })

  await handleTags([])
})