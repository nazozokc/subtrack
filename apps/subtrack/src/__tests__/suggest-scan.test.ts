import { test, expect, describe, vi, beforeAll, beforeEach, afterEach } from "vitest"
import { DatabaseSync } from "node:sqlite"
import { consola } from "@subtrack/lib/logger"
import type { SubtrackConfig } from "../types.ts"
import type { RawEmail } from "../suggest/types.ts"

vi.mock("../config.ts", () => ({
  loadConfig: vi.fn(),
  saveConfig: vi.fn(),
  resetConfig: vi.fn(),
}))

vi.mock("../suggest/imap.ts", () => ({
  connectAndSearch: vi.fn(),
}))

import { loadConfig, saveConfig } from "../config.ts"
import { connectAndSearch } from "../suggest/imap.ts"
import { autoScan, handleSuggestScan } from "../suggest/scan.ts"

let testDb: DatabaseSync
let mockConfig: SubtrackConfig

const logMessages: string[] = []
const infoMessages: string[] = []
const successMessages: string[] = []
const errorMessages: string[] = []
const failMessages: string[] = []
const warnMessages: string[] = []

const IMAP = { host: "imap.example.com", port: 993, tls: true, username: "me@example.com" }

function makeEmail(overrides: Partial<RawEmail> = {}): RawEmail {
  return {
    id: "1",
    from: "Netflix <info@netflix.com>",
    subject: "Your Netflix receipt",
    date: new Date("2026-06-15T00:00:00Z"),
    textBody: "Thank you for your payment of $9.99 monthly.",
    ...overrides,
  }
}

beforeAll(async () => {
  testDb = new DatabaseSync(":memory:")
  testDb.exec("PRAGMA foreign_keys = ON")
  const { runMigrations } = await import("../db/schema.ts")
  runMigrations(testDb)
  const db = await import("../db.ts")
  db.__setDb(testDb)
})

beforeEach(() => {
  process.exitCode = 0
  testDb.exec("DELETE FROM suggestions")

  logMessages.length = 0
  infoMessages.length = 0
  successMessages.length = 0
  errorMessages.length = 0
  failMessages.length = 0
  warnMessages.length = 0

  const stripAnsi = (s: string) => s.replace(/\x1b\[[0-9;]*m/g, "")
  consola.mockTypes((type: string) => {
    return (...args: unknown[]) => {
      const str = args.map((a) => String(a)).join(" ")
      const clean = stripAnsi(str)
      if (type === "log") logMessages.push(clean)
      if (type === "info") infoMessages.push(clean)
      if (type === "success") successMessages.push(clean)
      if (type === "error") errorMessages.push(clean)
      if (type === "fail") failMessages.push(clean)
      if (type === "warn") warnMessages.push(clean)
    }
  })

  process.env.SUBTRACK_IMAP_PASSWORD = "test-secret"

  vi.mocked(loadConfig).mockReset()
  vi.mocked(saveConfig).mockReset()
  vi.mocked(connectAndSearch).mockReset()

  mockConfig = { defaultCurrency: "USD", monthlyBudget: 0, theme: "default", notifyDays: 7 }
  vi.mocked(loadConfig).mockReturnValue(mockConfig)
})

afterEach(() => {
  consola.mockTypes()
  delete process.env.SUBTRACK_IMAP_PASSWORD
})

describe("autoScan", () => {
  test("does nothing when IMAP is not configured", async () => {
    await autoScan()
    expect(connectAndSearch).not.toHaveBeenCalled()
    expect(infoMessages).toHaveLength(0)
  })

  test("skips when within the 1-hour cooldown", async () => {
    mockConfig.imap = { ...IMAP }
    mockConfig.suggestLastScan = new Date().toISOString()

    await autoScan()
    expect(connectAndSearch).not.toHaveBeenCalled()
  })

  test("skips when the IMAP password is missing", async () => {
    mockConfig.imap = { ...IMAP }
    delete process.env.SUBTRACK_IMAP_PASSWORD

    await autoScan()
    expect(connectAndSearch).not.toHaveBeenCalled()
  })

  test("stores candidates in DB and updates last scan time on success", async () => {
    mockConfig.imap = { ...IMAP }
    vi.mocked(connectAndSearch).mockResolvedValue([makeEmail()])

    await autoScan()

    expect(connectAndSearch).toHaveBeenCalledTimes(1)
    expect(successMessages.some((m) => m.includes("1 new suggestion"))).toBe(true)
    expect(vi.mocked(saveConfig)).toHaveBeenCalled()

    const db = await import("../db.ts")
    const suggestions = db.getSuggestions("pending")
    expect(suggestions).toHaveLength(1)
    expect(suggestions[0].name).toBe("Netflix")
    expect(suggestions[0].price).toBe(999)
    expect(suggestions[0].currency).toBe("USD")
    expect(suggestions[0].cycle).toBe("monthly")
    expect(mockConfig.suggestLastScan).toBeDefined()
  })

  test("swallows connect errors silently", async () => {
    mockConfig.imap = { ...IMAP }
    vi.mocked(connectAndSearch).mockRejectedValue(new Error("connection refused"))

    await autoScan()
    expect(process.exitCode).toBe(0)
    expect(errorMessages).toHaveLength(0)
  })
})

describe("handleSuggestScan", () => {
  test("fails when IMAP is not configured", async () => {
    const count = await handleSuggestScan()
    expect(count).toBe(0)
    expect(connectAndSearch).not.toHaveBeenCalled()
    expect(errorMessages.some((m) => m.includes("IMAP not configured"))).toBe(true)
    expect(process.exitCode).toBe(1)
  })

  test("fails when the IMAP password is missing", async () => {
    mockConfig.imap = { ...IMAP }
    delete process.env.SUBTRACK_IMAP_PASSWORD

    const count = await handleSuggestScan()
    expect(count).toBe(0)
    expect(connectAndSearch).not.toHaveBeenCalled()
    expect(errorMessages.some((m) => m.includes("IMAP password not set"))).toBe(true)
    expect(process.exitCode).toBe(1)
  })

  test("returns 0 and logs info when no relevant emails found", async () => {
    mockConfig.imap = { ...IMAP }
    vi.mocked(connectAndSearch).mockResolvedValue([])

    const count = await handleSuggestScan()
    expect(count).toBe(0)
    expect(infoMessages.some((m) => m.includes("No relevant emails found"))).toBe(true)
  })

  test("returns count and stores candidates on success", async () => {
    mockConfig.imap = { ...IMAP }
    vi.mocked(connectAndSearch).mockResolvedValue([makeEmail()])

    const count = await handleSuggestScan()
    expect(count).toBe(1)
    expect(successMessages.some((m) => m.includes("Scan complete"))).toBe(true)

    const db = await import("../db.ts")
    expect(db.getSuggestions("pending")).toHaveLength(1)
    expect(mockConfig.suggestLastScan).toBeDefined()
  })

  test("fails when the IMAP scan throws", async () => {
    mockConfig.imap = { ...IMAP }
    vi.mocked(connectAndSearch).mockRejectedValue(new Error("timeout"))

    const count = await handleSuggestScan()
    expect(count).toBe(0)
    expect(errorMessages.some((m) => m.includes("Scan failed"))).toBe(true)
    expect(process.exitCode).toBe(1)
  })
})