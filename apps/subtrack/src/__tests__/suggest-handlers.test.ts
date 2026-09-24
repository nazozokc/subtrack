import { test, expect, beforeAll, afterAll, beforeEach, vi } from "vitest"
import { DatabaseSync } from "node:sqlite"
import { mkdtempSync, rmSync, existsSync } from "node:fs"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { consola } from "@subtrack/lib/logger"

vi.mock("../suggest/interactor.ts", () => ({
  reviewSuggestions: vi.fn(),
}))

import {
  handleSuggestList, handleSuggestView, handleSuggestReview,
  handleSuggestDismiss, handleSuggestDismissAll, handleSuggestAdd,
} from "../suggest/suggest.ts"
import { writeSuggestion } from "../db.ts"
import { getSuggestions, getSubscription } from "../db.ts"

const infoMessages: string[] = []
const successMessages: string[] = []
const errorMessages: string[] = []
const logMessages: string[] = []

let testDb: DatabaseSync
let dbModule: typeof import("../db.ts")
let tmpDir: string
let originalEnv: string | undefined

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

function seedSuggestion(overrides: Partial<Parameters<typeof writeSuggestion>[0]> = {}): number {
  return writeSuggestion({
    name: "Netflix",
    price: 1990,
    currency: "JPY",
    cycle: "monthly",
    source: "email",
    sourceDetail: "Parsed from email: charge",
    confidence: 0.9,
    ...overrides,
  })
}

beforeAll(async () => {
  tmpDir = mkdtempSync(join(tmpdir(), "subtrack-suggest-"))
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
  infoMessages.length = 0
  successMessages.length = 0
  errorMessages.length = 0
  logMessages.length = 0

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

// ── handleSuggestList ────────────────────────────────────

test("handleSuggestList reports when there are no pending suggestions", () => {
  handleSuggestList()
  expect(infoMessages).toContain("No pending suggestions.")
})

test("handleSuggestList reports no suggestions at all with --all", () => {
  handleSuggestList({ all: true })
  expect(infoMessages).toContain("No suggestions found.")
})

test("handleSuggestList renders pending suggestions", () => {
  seedSuggestion({ name: "Spotify" })
  handleSuggestList()
  const out = logMessages.join("\n")
  expect(out).toContain("Suggestions (1)")
  expect(out).toContain("#1")
  expect(out).toContain("Spotify")
  expect(out).toContain("pending")
})

test("handleSuggestList includes dismissed suggestions with --all", () => {
  seedSuggestion({ name: "Old" })
  const { dismissSuggestion } = dbModule
  dismissSuggestion(1)
  handleSuggestList({ all: true })
  const out = logMessages.join("\n")
  expect(out).toContain("dismissed")
})

test("handleSuggestList --json outputs suggestions", async () => {
  seedSuggestion()
  const out = await captureStdout(() => handleSuggestList({ json: true }))
  const parsed = JSON.parse(out) as Array<{ name: string; status: string }>
  expect(parsed).toHaveLength(1)
  expect(parsed[0]!.name).toBe("Netflix")
  expect(parsed[0]!.status).toBe("pending")
})

// ── handleSuggestView ────────────────────────────────────

test("handleSuggestView shows a suggestion's details", () => {
  seedSuggestion({ vendorName: "Netflix Inc.", planTier: "Standard", paymentMethod: "card" })
  handleSuggestView(1)
  const out = logMessages.join("\n")
  expect(out).toContain("Suggestion #1")
  expect(out).toContain("Netflix Inc.")
  expect(out).toContain("Standard")
  expect(out).toContain("card")
  expect(out).toContain("90%")
})

test("handleSuggestView errors for an unknown id", () => {
  handleSuggestView(999)
  expect(errorMessages.some((m) => m.includes("Suggestion #999 not found."))).toBe(true)
  expect(process.exitCode).toBe(1)
})

// ── handleSuggestReview ──────────────────────────────────

test("handleSuggestReview reports when there is nothing to review", async () => {
  await handleSuggestReview()
  expect(infoMessages).toContain("No pending suggestions to review.")
})

// ── handleSuggestDismiss ─────────────────────────────────

test("handleSuggestDismiss dismisses a pending suggestion", () => {
  seedSuggestion()
  handleSuggestDismiss(1)
  expect(successMessages).toContain("Suggestion #1 dismissed.")
  expect(getSuggestions("pending")).toHaveLength(0)
})

test("handleSuggestDismiss errors for an unknown id", () => {
  handleSuggestDismiss(404)
  expect(errorMessages.some((m) => m.includes("Suggestion #404 not found or already processed."))).toBe(true)
  expect(process.exitCode).toBe(1)
})

// ── handleSuggestDismissAll ──────────────────────────────

test("handleSuggestDismissAll is a no-op without pending suggestions", () => {
  handleSuggestDismissAll()
  expect(infoMessages).toContain("No pending suggestions to dismiss.")
})

test("handleSuggestDismissAll dismisses all pending suggestions", () => {
  seedSuggestion({ name: "A" })
  seedSuggestion({ name: "B" })
  handleSuggestDismissAll()
  expect(successMessages).toContain("2 suggestions dismissed.")
  expect(getSuggestions("pending")).toHaveLength(0)
})

// ── handleSuggestAdd ─────────────────────────────────────

test("handleSuggestAdd creates a subscription and marks the suggestion added", () => {
  seedSuggestion()
  handleSuggestAdd(1)
  expect(successMessages).toContain('Added "Netflix" as subscription #1.')
  const sub = getSubscription(1)
  expect(sub?.name).toBe("Netflix")
  expect(sub?.price).toBe(1990)
  expect(sub?.currency).toBe("JPY")
  const suggestion = getSuggestions("added")
  expect(suggestion).toHaveLength(1)
  expect(suggestion[0]!.matchedSubId).toBe(1)
})

test("handleSuggestAdd respects suggestion vendor/plan/payment fields", () => {
  seedSuggestion({ vendorName: "Netflix Inc.", planTier: "Premium", paymentMethod: "amex" })
  handleSuggestAdd(1)
  const sub = getSubscription(1)
  expect(sub?.vendorName).toBe("Netflix Inc.")
  expect(sub?.planTier).toBe("Premium")
  expect(sub?.paymentMethod).toBe("amex")
})

test("handleSuggestAdd errors for an unknown id", () => {
  handleSuggestAdd(999)
  expect(errorMessages.some((m) => m.includes("Suggestion #999 not found."))).toBe(true)
  expect(process.exitCode).toBe(1)
})

test("handleSuggestAdd errors when the suggestion is already processed", () => {
  seedSuggestion()
  handleSuggestAdd(1)
  process.exitCode = 0
  handleSuggestAdd(1)
  expect(errorMessages.some((m) => m.includes("Suggestion #1 is already added."))).toBe(true)
  expect(process.exitCode).toBe(1)
})