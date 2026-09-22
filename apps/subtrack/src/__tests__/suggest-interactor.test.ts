import { test, expect, describe, vi, beforeAll, beforeEach, afterEach } from "vitest"
import { DatabaseSync } from "node:sqlite"
import { consola } from "@subtrack/lib/logger"

vi.mock("@inquirer/prompts", () => ({
  input: vi.fn(),
  confirm: vi.fn(),
  checkbox: vi.fn(),
  select: vi.fn(),
  search: vi.fn(),
}))

import { input, confirm, select } from "@inquirer/prompts"
import { reviewSuggestions } from "../suggest/interactor.ts"

let testDb: DatabaseSync

const logMessages: string[] = []
const infoMessages: string[] = []
const successMessages: string[] = []
const warnMessages: string[] = []

const stripAnsi = (s: string) => s.replace(/\x1b\[[0-9;]*m/g, "")

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
  testDb.exec("DELETE FROM price_history")
  testDb.exec("DELETE FROM subscription_tags")
  testDb.exec("DELETE FROM tags")
  testDb.exec("DELETE FROM subscriptions")
  testDb.exec("DELETE FROM suggestions")

  logMessages.length = 0
  infoMessages.length = 0
  successMessages.length = 0
  warnMessages.length = 0

  consola.mockTypes((type: string) => {
    return (...args: unknown[]) => {
      const str = args.map((a) => String(a)).join(" ")
      const clean = stripAnsi(str)
      if (type === "log") logMessages.push(clean)
      if (type === "info") infoMessages.push(clean)
      if (type === "success") successMessages.push(clean)
      if (type === "warn") warnMessages.push(clean)
    }
  })

  vi.mocked(input).mockReset()
  vi.mocked(confirm).mockReset()
  vi.mocked(select).mockReset()
})

afterEach(() => {
  consola.mockTypes()
})

async function addSuggestion(data: {
  name: string
  price?: number | null
  currency?: string | null
  cycle?: string | null
  confidence?: number
}): Promise<import("../suggest/types.ts").Suggestion> {
  const db = await import("../db.ts")
  db.writeSuggestion({
    name: data.name,
    price: data.price ?? null,
    currency: data.currency ?? null,
    cycle: data.cycle ?? null,
    source: "email",
    sourceDetail: "test",
    confidence: data.confidence ?? 0.8,
  })
  return db.getSuggestions("pending")[0]
}

describe("reviewSuggestions", () => {
  test("does nothing when there are no pending suggestions", async () => {
    await reviewSuggestions([])
    expect(infoMessages.some((m) => m.includes("No pending suggestions"))).toBe(true)
    expect(select).not.toHaveBeenCalled()
  })

  test("add flow creates a subscription and marks the suggestion as added", async () => {
    const db = await import("../db.ts")
    const suggestion = await addSuggestion({ name: "Notion", price: 1200, currency: "JPY", cycle: "monthly" })

    vi.mocked(select).mockResolvedValue("add")
    await reviewSuggestions([suggestion])

    const subs = db.getSubscriptions()
    expect(subs).toHaveLength(1)
    expect(subs[0]).toMatchObject({ name: "Notion", price: 1200, currency: "JPY", cycle: "monthly" })
    expect(db.getSuggestion(suggestion.id)?.status).toBe("added")
    expect(successMessages.some((m) => m.includes("Notion"))).toBe(true)
  })

  test("add flow with missing cycle prompts for a billing cycle", async () => {
    const db = await import("../db.ts")
    const suggestion = await addSuggestion({ name: "Tool", price: 500, currency: "USD", cycle: null })

    vi.mocked(select).mockResolvedValueOnce("add").mockResolvedValueOnce("yearly")
    await reviewSuggestions([suggestion])

    const subs = db.getSubscriptions()
    expect(subs).toHaveLength(1)
    expect(subs[0].cycle).toBe("yearly")
  })

  test("add flow defaults price to 0 and currency to USD", async () => {
    const db = await import("../db.ts")
    const suggestion = await addSuggestion({ name: "Freebie", price: null, currency: null, cycle: "monthly" })

    vi.mocked(select).mockResolvedValue("add")
    await reviewSuggestions([suggestion])

    const subs = db.getSubscriptions()
    expect(subs).toHaveLength(1)
    expect(subs[0]).toMatchObject({ name: "Freebie", price: 0, currency: "USD" })
  })

  test("skip flow dismisses the suggestion", async () => {
    const db = await import("../db.ts")
    const suggestion = await addSuggestion({ name: "Foo", price: 100, currency: "USD", cycle: "monthly" })

    vi.mocked(select).mockResolvedValue("skip")
    await reviewSuggestions([suggestion])

    expect(db.getSuggestion(suggestion.id)?.status).toBe("dismissed")
    expect(db.getSubscriptions()).toHaveLength(0)
    expect(infoMessages.some((m) => m.includes("Suggestion dismissed"))).toBe(true)
  })

  test("quit flow stops processing remaining suggestions", async () => {
    const db = await import("../db.ts")
    await addSuggestion({ name: "First", price: 100, currency: "USD", cycle: "monthly" })
    await addSuggestion({ name: "Second", price: 200, currency: "USD", cycle: "monthly" })
    const suggestions = db.getSuggestions("pending")

    vi.mocked(select).mockResolvedValue("quit")
    await reviewSuggestions(suggestions)

    expect(select).toHaveBeenCalledTimes(1)
    for (const s of suggestions) {
      expect(db.getSuggestion(s.id)?.status).toBe("pending")
    }
  })

  test("exact-match flow warns and skipping leaves the suggestion pending", async () => {
    const db = await import("../db.ts")
    db.writeSubscription({ name: "Netflix", price: 1490, currency: "JPY", cycle: "monthly", tags: [] })
    const suggestion = await addSuggestion({ name: "Netflix", price: 1000, currency: "USD", cycle: "monthly" })

    vi.mocked(select).mockResolvedValue("skip")
    await reviewSuggestions([suggestion])

    expect(warnMessages.some((m) => m.includes("Already exists"))).toBe(true)
    expect(select).toHaveBeenCalledTimes(1)
    expect(db.getSuggestion(suggestion.id)?.status).toBe("pending")
    expect(db.getSubscriptions()).toHaveLength(1)
  })

  test("edit flow lets the user change fields before adding", async () => {
    const db = await import("../db.ts")
    const suggestion = await addSuggestion({ name: "Draft", price: null, currency: null, cycle: null })

    vi.mocked(select)
      .mockResolvedValueOnce("edit")
      .mockResolvedValueOnce("USD")
      .mockResolvedValueOnce("monthly")
    vi.mocked(input).mockResolvedValueOnce("My Service").mockResolvedValueOnce("10")
    vi.mocked(confirm).mockResolvedValueOnce(true)
    await reviewSuggestions([suggestion])

    const subs = db.getSubscriptions()
    expect(subs).toHaveLength(1)
    expect(subs[0]).toMatchObject({ name: "My Service", price: 10, currency: "USD", cycle: "monthly" })
    expect(db.getSuggestion(suggestion.id)?.status).toBe("added")
  })
})