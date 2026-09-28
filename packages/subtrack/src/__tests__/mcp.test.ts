import { describe, test, expect, beforeAll, beforeEach, afterEach, vi } from "vitest"
import { DatabaseSync } from "node:sqlite"

vi.mock("../fx.ts", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../fx.ts")>()
  return {
    ...actual,
    fetchFxRates: vi.fn(),
  }
})

let testDb: DatabaseSync
let dbModule: typeof import("../db.ts")

function dateStr(d: Date): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`
}

beforeAll(async () => {
  testDb = new DatabaseSync(":memory:")
  testDb.exec("PRAGMA foreign_keys = ON")

  testDb.exec(`CREATE TABLE IF NOT EXISTS subscriptions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    price INTEGER NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    cycle TEXT NOT NULL DEFAULT 'monthly',
    status TEXT NOT NULL DEFAULT 'active',
    billing_day INTEGER,
    created_at TEXT NOT NULL DEFAULT (date('now')),
    notes TEXT,
    payment_method TEXT,
    contract_start TEXT,
    contract_end TEXT,
    auto_renewal INTEGER NOT NULL DEFAULT 1,
    vendor_name TEXT,
    vendor_url TEXT,
    plan_tier TEXT,
    discount_amount INTEGER,
    discount_type TEXT
  )`)
  testDb.exec(`CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
  )`)
  testDb.exec(`CREATE TABLE IF NOT EXISTS subscription_tags (
    subscription_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,
    PRIMARY KEY (subscription_id, tag_id),
    FOREIGN KEY (subscription_id) REFERENCES subscriptions(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
  )`)
  testDb.exec(`CREATE TABLE IF NOT EXISTS price_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    subscription_id INTEGER NOT NULL,
    old_price INTEGER,
    new_price INTEGER NOT NULL,
    old_currency TEXT,
    new_currency TEXT NOT NULL,
    changed_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (subscription_id) REFERENCES subscriptions(id) ON DELETE CASCADE
  )`)

  dbModule = await import("../db.ts")
  dbModule.runMigrations(testDb)
  dbModule.__setDb(testDb)
})

beforeEach(() => {
  testDb.exec("DELETE FROM price_history")
  testDb.exec("DELETE FROM subscription_tags")
  testDb.exec("DELETE FROM tags")
  testDb.exec("DELETE FROM subscriptions")
  testDb.exec("DELETE FROM trials")
  testDb.exec("DELETE FROM sqlite_sequence")
})

describe("MCP helper functions", () => {
  test("formatDateISO formats date to ISO string", async () => {
    const { formatDateISO } = await import("../mcp.ts")
    expect(formatDateISO(new Date("2026-06-15"))).toBe("2026-06-15")
  })

  test("nextDateForCycle — monthly basic case", async () => {
    const { nextDateForCycle } = await import("../upcoming.ts")
    // Use local date constructors for timezone safety
    const anchor = new Date(2026, 0, 15)  // Jan 15
    const from = new Date(2026, 5, 1)     // Jun 1
    const next = nextDateForCycle(15, anchor, "monthly", from)
    // Should be Jun 15 (within same month)
    expect(next.getMonth()).toBe(5)  // June
    expect(next.getDate()).toBe(15)
  })

  test("nextDateForCycle — monthly rolls to next month", async () => {
    const { nextDateForCycle } = await import("../upcoming.ts")
    const anchor = new Date(2026, 0, 15)  // Jan 15
    const from = new Date(2026, 5, 20)    // Jun 20 (past billing day 15)
    const next = nextDateForCycle(15, anchor, "monthly", from)
    expect(next.getMonth()).toBe(6)  // July
    expect(next.getDate()).toBe(15)
  })

  test("nextDateForCycle — yearly returns next year", async () => {
    const { nextDateForCycle } = await import("../upcoming.ts")
    const anchor = new Date(2026, 2, 10)  // Mar 10
    const from = new Date(2026, 5, 1)     // Jun 1
    const next = nextDateForCycle(10, anchor, "yearly", from)
    expect(next.getFullYear()).toBe(2027)
    expect(next.getMonth()).toBe(2)  // March
    expect(next.getDate()).toBe(10)
  })

  test("nextDateForCycle — weekly returns next week", async () => {
    const { nextDateForCycle } = await import("../upcoming.ts")
    const anchor = new Date(2026, 5, 1)   // Jun 1 (Monday)
    const from = new Date(2026, 5, 15)    // Jun 15
    const next = nextDateForCycle(1, anchor, "weekly", from)
    // Should be a Monday on or after Jun 15
    expect(next.getDay()).toBe(1)  // Monday
    expect(next.getTime()).toBeGreaterThanOrEqual(from.getTime())
    const diffDays = (next.getTime() - from.getTime()) / (24 * 60 * 60 * 1000)
    expect(diffDays).toBeLessThanOrEqual(7)
  })

  test("nextDateForCycle — bi-weekly returns correct date", async () => {
    const { nextDateForCycle } = await import("../upcoming.ts")
    const anchor = new Date(2026, 5, 1)   // Jun 1
    const from = new Date(2026, 5, 15)    // Jun 15
    const next = nextDateForCycle(1, anchor, "bi-weekly", from)
    expect(next.getTime()).toBeGreaterThanOrEqual(from.getTime())
    const diffDays = (next.getTime() - from.getTime()) / (24 * 60 * 60 * 1000)
    expect(diffDays).toBeLessThanOrEqual(14)
  })

  test("nextDateForCycle — quarterly returns next quarter", async () => {
    const { nextDateForCycle } = await import("../upcoming.ts")
    const anchor = new Date(2026, 0, 15)  // Jan 15
    const from = new Date(2026, 5, 1)     // Jun 1
    const next = nextDateForCycle(15, anchor, "quarterly", from)
    expect(next.getMonth()).toBe(6)  // July (Q3)
    expect(next.getDate()).toBe(15)
  })
})

describe("calcUpcoming", () => {
  test("returns upcoming billings within period", async () => {
    testDb.exec(
      `INSERT INTO subscriptions (id, name, price, currency, cycle, status, billing_day, created_at)
       VALUES (1, 'Netflix', 1990, 'JPY', 'monthly', 'active', 15, '2026-01-01'),
              (2, 'Spotify', 980, 'JPY', 'monthly', 'active', 1, '2026-01-10'),
              (3, 'GitHub Copilot', 1000, 'USD', 'monthly', 'cancelled', 5, '2026-03-01')`,
    )

    const { calcUpcoming } = await import("../upcoming.ts")
    const result = calcUpcoming(30)
    const names = result.map((e: { sub: { name: string } }) => e.sub.name)
    expect(names).toContain("Netflix")
    expect(names).not.toContain("GitHub Copilot")
  })
})

describe("searchSubscriptions", () => {
  test("searches by name pattern", async () => {
    testDb.exec(
      `INSERT INTO subscriptions (id, name, price, currency, cycle, status, billing_day, created_at, notes)
       VALUES (1, 'Netflix', 1990, 'JPY', 'monthly', 'active', 15, '2026-01-01', 'Family plan'),
              (2, 'Spotify', 980, 'JPY', 'monthly', 'active', 1, '2026-01-10', NULL)`,
    )

    const { searchSubscriptions } = await import("../search.ts")
    const results = searchSubscriptions("net", {})
    expect(results.length).toBeGreaterThanOrEqual(1)
    expect(results.some((r: { name: string }) => r.name === "Netflix")).toBe(true)
  })

  test("returns empty array for no match", async () => {
    testDb.exec(
      `INSERT INTO subscriptions (id, name, price, currency, cycle, status, billing_day, created_at)
       VALUES (1, 'Netflix', 1990, 'JPY', 'monthly', 'active', 15, '2026-01-01')`,
    )

    const { searchSubscriptions } = await import("../search.ts")
    const results = searchSubscriptions("zzzzz", {})
    expect(results.length).toBe(0)
  })
})

describe("startMcpServer", () => {
  test("exports startMcpServer function", async () => {
    const { startMcpServer } = await import("../mcp.ts")
    expect(typeof startMcpServer).toBe("function")
  })
})

describe("MCP input validation", () => {
  test("validateArgs rejects oversized bulk_operations filter_name", async () => {
    const { validateArgs, INPUT_VALIDATIONS } = await import("../mcp/security.ts")
    const err = validateArgs(
      { action: "status", filter_name: "x".repeat(5000) },
      INPUT_VALIDATIONS.bulk_operations!,
    )
    expect(err).toMatch(/too long/)
  })

  test("validateArgs rejects wrong types for previously-uncovered tools", async () => {
    const { validateArgs, INPUT_VALIDATIONS } = await import("../mcp/security.ts")
    // list_subscriptions desc must be boolean
    const err = validateArgs(
      { desc: "yes" },
      INPUT_VALIDATIONS.list_subscriptions!,
    )
    expect(err).toMatch(/boolean/)
    // get_forecast currency must be a short string
    const err2 = validateArgs(
      { currency: "A".repeat(100) },
      INPUT_VALIDATIONS.get_forecast!,
    )
    expect(err2).toMatch(/too long/)
  })

  test("validateArgs rejects unsupported enum and non-boolean search flags", async () => {
    const { validateArgs, INPUT_VALIDATIONS } = await import("../mcp/security.ts")
    expect(validateArgs({ period: "garbage" }, INPUT_VALIDATIONS.compare!)).toMatch(/unsupported/i)
    expect(validateArgs({ names: "false" }, INPUT_VALIDATIONS.search_subscriptions!)).toMatch(/boolean/i)
  })

  test("every registered tool has an input validation schema", async () => {
    const { INPUT_VALIDATIONS } = await import("../mcp/security.ts")
    const { TOOLS } = await import("../mcp/tools.ts")
    for (const tool of TOOLS) {
      expect(INPUT_VALIDATIONS[tool.name], `missing schema for ${tool.name}`).toBeDefined()
    }
  })
})

describe("MCP handlers", () => {
  test("handleAddSubscription validates cycle and status enums", async () => {
    const { handleAddSubscription } = await import("../mcp/handlers.ts")
    const badCycle = await handleAddSubscription({
      name: "X", price: 100, currency: "USD", cycle: "fortnightly",
    })
    expect(badCycle.isError).toBe(true)
    expect(JSON.stringify(badCycle)).toMatch(/Invalid cycle/)

    const badStatus = await handleAddSubscription({
      name: "X", price: 100, currency: "USD", cycle: "monthly", status: "deleted",
    })
    expect(badStatus.isError).toBe(true)
    expect(JSON.stringify(badStatus)).toMatch(/Invalid status/)

    const badCurrency = await handleAddSubscription({
      name: "X", price: 100, currency: "XX", cycle: "monthly",
    })
    expect(badCurrency.isError).toBe(true)
    expect(JSON.stringify(badCurrency)).toMatch(/Invalid currency/)

    const ok = await handleAddSubscription({
      name: "Valid", price: 100, currency: "USD", cycle: "monthly", status: "paused",
    })
    expect(ok.isError).toBeUndefined()
  })

  test("handleEditSubscription validates enums", async () => {
    testDb.exec(
      `INSERT INTO subscriptions (id, name, price, currency, cycle, status, billing_day, created_at)
       VALUES (1, 'Netflix', 1990, 'JPY', 'monthly', 'active', 15, '2026-01-01')`,
    )
    const { handleEditSubscription } = await import("../mcp/handlers.ts")
    const bad = await handleEditSubscription({ id: 1, cycle: "fortnightly" })
    expect(bad.isError).toBe(true)
    expect(JSON.stringify(bad)).toMatch(/Invalid cycle/)
    const badStatus = await handleEditSubscription({ id: 1, status: "deleted" })
    expect(badStatus.isError).toBe(true)
  })

  test("handleGetAnalytics includes statusBreakdown distinct from summary", async () => {
    testDb.exec(
      `INSERT INTO subscriptions (id, name, price, currency, cycle, status, billing_day, created_at)
       VALUES (1, 'Netflix', 1990, 'JPY', 'monthly', 'active', 15, '2026-01-01'),
              (2, 'Spotify', 980, 'JPY', 'monthly', 'paused', 1, '2026-01-10'),
              (3, 'Old', 500, 'JPY', 'monthly', 'cancelled', 5, '2026-03-01'),
              (4, 'Legacy', 300, 'JPY', 'monthly', 'archived', 5, '2026-03-01')`,
    )
    const { handleGetAnalytics } = await import("../mcp/handlers.ts")
    const res = await handleGetAnalytics({})
    const data = JSON.parse(res.content[0].text)
    expect(data.statusBreakdown).toEqual({ active: 1, paused: 1, cancelled: 1, archived: 1 })
    expect(data.totalCount).toBe(2) // cancelled excluded from summary
  })

  test("handleListSubscriptions supports limit and offset", async () => {
    testDb.exec(
      `INSERT INTO subscriptions (id, name, price, currency, cycle, status, billing_day, created_at)
       VALUES (1, 'A', 100, 'USD', 'monthly', 'active', 1, '2026-01-01'),
              (2, 'B', 200, 'USD', 'monthly', 'active', 1, '2026-01-01'),
              (3, 'C', 300, 'USD', 'monthly', 'active', 1, '2026-01-01')`,
    )
    const { handleListSubscriptions } = await import("../mcp/handlers.ts")
    const res = await handleListSubscriptions({ limit: 2 })
    const data = JSON.parse(res.content[0].text)
    expect(data).toHaveLength(2)
    const res2 = await handleListSubscriptions({ limit: 2, offset: 2 })
    const data2 = JSON.parse(res2.content[0].text)
    expect(data2).toHaveLength(1)
    expect(data2[0].name).toBe("C")
  })

  test("handleListTags returns tags with counts", async () => {
    testDb.exec(
      `INSERT INTO subscriptions (id, name, price, currency, cycle, status, billing_day, created_at)
       VALUES (1, 'Netflix', 1990, 'JPY', 'monthly', 'active', 15, '2026-01-01'),
              (2, 'Spotify', 980, 'JPY', 'monthly', 'active', 1, '2026-01-10')`,
    )
    testDb.exec(`INSERT INTO tags (id, name) VALUES (1, 'video'), (2, 'music'), (3, 'work')`)
    testDb.exec(`INSERT INTO subscription_tags (subscription_id, tag_id) VALUES (1, 1), (2, 2), (1, 3)`)

    const { handleListTags } = await import("../mcp/handlers.ts")
    const res = await handleListTags({})
    const data = JSON.parse(res.content[0].text)
    expect(data).toEqual([
      { name: "music", count: 1 },
      { name: "video", count: 1 },
      { name: "work", count: 1 },
    ])
  })

  test("handleGetTagSubscriptions filters by tags", async () => {
    testDb.exec(
      `INSERT INTO subscriptions (id, name, price, currency, cycle, status, billing_day, created_at)
       VALUES (1, 'Netflix', 1990, 'JPY', 'monthly', 'active', 15, '2026-01-01'),
              (2, 'Spotify', 980, 'JPY', 'monthly', 'active', 1, '2026-01-10')`,
    )
    testDb.exec(`INSERT INTO tags (id, name) VALUES (1, 'video'), (2, 'music')`)
    testDb.exec(`INSERT INTO subscription_tags (subscription_id, tag_id) VALUES (1, 1), (2, 2)`)

    const { handleGetTagSubscriptions } = await import("../mcp/handlers.ts")
    const res = await handleGetTagSubscriptions({ tag: "video" })
    const data = JSON.parse(res.content[0].text)
    expect(data).toHaveLength(1)
    expect(data[0].name).toBe("Netflix")

    const noTag = await handleGetTagSubscriptions({})
    expect(noTag.isError).toBe(true)
  })

  test("handleGetUsageTotal aggregates tokens and models", async () => {
    const db = await import("../db.ts")
    testDb.exec(`CREATE TABLE IF NOT EXISTS llm_usage (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      provider TEXT NOT NULL,
      model TEXT NOT NULL,
      input_tokens INTEGER NOT NULL DEFAULT 0,
      output_tokens INTEGER NOT NULL DEFAULT 0,
      cost REAL NOT NULL DEFAULT 0,
      date TEXT NOT NULL,
      description TEXT,
      generation_id TEXT
    )`)
    testDb.exec("DELETE FROM llm_usage")
    db.addLlmUsage({ provider: "openai", model: "gpt-4o", input_tokens: 100, output_tokens: 50, cost: 1.0, date: "2026-08-01", description: null })
    db.addLlmUsage({ provider: "openai", model: "gpt-4o", input_tokens: 200, output_tokens: 100, cost: 2.0, date: "2026-08-02", description: null })

    const { handleGetUsageTotal } = await import("../mcp/handlers.ts")
    const res = await handleGetUsageTotal({ from: "2026-08-01", to: "2026-08-31" })
    const data = JSON.parse(res.content[0].text)
    expect(data.total).toBe(3.0)
    expect(data.tokens).toEqual({ inputTokens: 300, outputTokens: 150 })
    expect(data.byModel).toHaveLength(1)
    expect(data.byModel[0].model).toBe("gpt-4o")
  })

  test("handleListUsage lists entries with filters", async () => {
    const db = await import("../db.ts")
    testDb.exec(`CREATE TABLE IF NOT EXISTS llm_usage (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      provider TEXT NOT NULL,
      model TEXT NOT NULL,
      input_tokens INTEGER NOT NULL DEFAULT 0,
      output_tokens INTEGER NOT NULL DEFAULT 0,
      cost REAL NOT NULL DEFAULT 0,
      date TEXT NOT NULL,
      description TEXT,
      generation_id TEXT
    )`)
    testDb.exec("DELETE FROM llm_usage")
    db.addLlmUsage({ provider: "openai", model: "gpt-4o", input_tokens: 100, output_tokens: 50, cost: 1.0, date: "2026-08-01", description: null })
    db.addLlmUsage({ provider: "anthropic", model: "claude-3", input_tokens: 100, output_tokens: 50, cost: 1.0, date: "2026-08-02", description: null })

    const { handleListUsage } = await import("../mcp/handlers.ts")
    const res = await handleListUsage({ provider: "openai" })
    const data = JSON.parse(res.content[0].text)
    expect(data).toHaveLength(1)
    expect(data[0].provider).toBe("openai")
  })

  test("handleAddSubscription rejects invalid numeric fields", async () => {
    const { handleAddSubscription } = await import("../mcp/handlers.ts")
    const billingDay = await handleAddSubscription({
      name: "X", price: 100, currency: "USD", cycle: "monthly", billingDay: 99,
    })
    expect(billingDay.isError).toBe(true)
    expect(JSON.stringify(billingDay)).toMatch(/billing day/i)

    const price = await handleAddSubscription({
      name: "X", price: -1, currency: "USD", cycle: "monthly",
    })
    expect(price.isError).toBe(true)
    expect(JSON.stringify(price)).toMatch(/price/i)

    const decimal = await handleAddSubscription({
      name: "Decimal", price: 14.99, currency: "USD", cycle: "monthly",
    })
    expect(decimal.isError).toBeUndefined()
  })

  test("handleEditSubscription reports a missing subscription", async () => {
    const { handleEditSubscription } = await import("../mcp/handlers.ts")
    const res = await handleEditSubscription({ id: 999, name: "missing" })
    expect(res.isError).toBe(true)
    expect(res.content[0].text).toMatch(/not found/i)
  })

  test("handleCompare rejects an unsupported period", async () => {
    const { handleCompare } = await import("../mcp/handlers.ts")
    const res = await handleCompare({ period: "garbage" })
    expect(res.isError).toBe(true)
    expect(res.content[0].text).toMatch(/period/i)
  })

  test("handleGetForecast and handleCompare preserve decimal totals", async () => {
    testDb.exec(
      `INSERT INTO subscriptions (name, price, currency, cycle, status) VALUES ('Decimal', 14.99, 'USD', 'monthly', 'active')`,
    )
    const { handleGetForecast, handleCompare } = await import("../mcp/handlers.ts")

    const forecast = JSON.parse((await handleGetForecast({})).content[0].text) as {
      monthlyTotal: number
      entries: { monthly: number }[]
    }
    expect(forecast.monthlyTotal).toBe(14.99)
    expect(forecast.entries[0]?.monthly).toBe(14.99)

    const compare = JSON.parse((await handleCompare({ period: "monthly" })).content[0].text) as {
      rows: { current: number }[]
    }
    expect(compare.rows[0]?.current).toBe(14.99)
  })

  test("handleBulkOperations requires explicit confirmation for deletion", async () => {
    const { handleBulkOperations } = await import("../mcp/handlers.ts")
    const res = await handleBulkOperations({ action: "delete" })
    expect(res.isError).toBe(true)
    expect(res.content[0].text).toMatch(/confirm/i)
  })

  test("handleBulkOperations deletes only after confirmation", async () => {
    testDb.exec(
      `INSERT INTO subscriptions (name, price, currency, cycle, status) VALUES ('A', 100, 'USD', 'monthly', 'active'), ('B', 200, 'USD', 'monthly', 'active')`,
    )
    const { handleBulkOperations } = await import("../mcp/handlers.ts")
    await handleBulkOperations({ action: "delete" })
    expect(testDb.prepare("SELECT COUNT(*) AS count FROM subscriptions").get()).toMatchObject({ count: 2 })

    const confirmed = await handleBulkOperations({ action: "delete", confirm: true })
    expect(confirmed.isError).toBeUndefined()
    expect(testDb.prepare("SELECT COUNT(*) AS count FROM subscriptions").get()).toMatchObject({ count: 0 })
  })

  test("handleBulkOperations reports errors instead of swallowing them", async () => {
    testDb.exec(
      `INSERT INTO subscriptions (id, name, price, currency, cycle, status, billing_day, created_at)
       VALUES (1, 'Netflix', 1990, 'JPY', 'monthly', 'active', 15, '2026-01-01'),
              (2, 'Spotify', 980, 'JPY', 'monthly', 'active', 1, '2026-01-10')`,
    )
    const { handleBulkOperations } = await import("../mcp/handlers.ts")
    const res = await handleBulkOperations({ action: "status", status: "invalid-status" })
    expect(res.isError).toBe(true)
    expect(JSON.stringify(res)).toMatch(/Invalid status/)
  })

  test("handleGetSubscription requires id", async () => {
    const { handleGetSubscription } = await import("../mcp/handlers.ts")
    const res = await handleGetSubscription({})
    expect(res.isError).toBe(true)
    expect(res.content[0].text).toBe("id is required")
  })

  test("handleGetSubscription returns sub for existing id", async () => {
    testDb.exec(
      `INSERT INTO subscriptions (id, name, price, currency, cycle, status, billing_day, created_at)
       VALUES (1, 'Netflix', 1990, 'JPY', 'monthly', 'active', 15, '2026-01-01')`,
    )
    const { handleGetSubscription } = await import("../mcp/handlers.ts")
    const res = await handleGetSubscription({ id: 1 })
    const data = JSON.parse(res.content[0].text)
    expect(data.name).toBe("Netflix")
    expect(data.price).toBe(1990)
    expect(data.currency).toBe("JPY")
  })

  test("handleGetSubscription returns null for missing id", async () => {
    const { handleGetSubscription } = await import("../mcp/handlers.ts")
    const res = await handleGetSubscription({ id: 999 })
    const data = JSON.parse(res.content[0].text)
    expect(data).toBeNull()
  })

  test("handleSearchSubscriptions requires query", async () => {
    const { handleSearchSubscriptions } = await import("../mcp/handlers.ts")
    const res = await handleSearchSubscriptions({})
    expect(res.isError).toBe(true)
    expect(res.content[0].text).toBe("query is required")
  })

  test("handleSearchSubscriptions finds by name", async () => {
    testDb.exec(
      `INSERT INTO subscriptions (id, name, price, currency, cycle, status, billing_day, created_at)
       VALUES (1, 'Netflix', 1990, 'JPY', 'monthly', 'active', 15, '2026-01-01'),
              (2, 'Spotify', 980, 'JPY', 'monthly', 'active', 1, '2026-01-10')`,
    )
    const { handleSearchSubscriptions } = await import("../mcp/handlers.ts")
    const res = await handleSearchSubscriptions({ query: "net" })
    const data = JSON.parse(res.content[0].text)
    expect(data).toHaveLength(1)
    expect(data[0].name).toBe("Netflix")
  })

  test("handleSearchSubscriptions returns empty on no hit", async () => {
    testDb.exec(
      `INSERT INTO subscriptions (id, name, price, currency, cycle, status, billing_day, created_at)
       VALUES (1, 'Netflix', 1990, 'JPY', 'monthly', 'active', 15, '2026-01-01')`,
    )
    const { handleSearchSubscriptions } = await import("../mcp/handlers.ts")
    const res = await handleSearchSubscriptions({ query: "zzzz" })
    const data = JSON.parse(res.content[0].text)
    expect(data).toEqual([])
  })

  test("handleDeleteSubscription requires id", async () => {
    const { handleDeleteSubscription } = await import("../mcp/handlers.ts")
    const res = await handleDeleteSubscription({})
    expect(res.isError).toBe(true)
    expect(res.content[0].text).toBe("id is required")
  })

  test("handleDeleteSubscription deletes an existing sub", async () => {
    testDb.exec(
      `INSERT INTO subscriptions (id, name, price, currency, cycle, status, billing_day, created_at)
       VALUES (1, 'Netflix', 1990, 'JPY', 'monthly', 'active', 15, '2026-01-01')`,
    )
    const { handleDeleteSubscription } = await import("../mcp/handlers.ts")
    const res = await handleDeleteSubscription({ id: 1 })
    const data = JSON.parse(res.content[0].text)
    expect(data.success).toBe(true)
    const db = await import("../db.ts")
    expect(db.getSubscription(1)).toBeUndefined()
  })

  test("handleDeleteSubscription reports false for missing id", async () => {
    const { handleDeleteSubscription } = await import("../mcp/handlers.ts")
    const res = await handleDeleteSubscription({ id: 999 })
    const data = JSON.parse(res.content[0].text)
    expect(data.success).toBe(false)
  })

  test("handleGetSummary returns zero totals on empty db", async () => {
    const { handleGetSummary } = await import("../mcp/handlers.ts")
    const res = await handleGetSummary()
    const data = JSON.parse(res.content[0].text)
    expect(data.totalCount).toBe(0)
    expect(data.monthlyByCurrency).toEqual({})
  })

  test("handleGetSummary aggregates non-cancelled totals by currency", async () => {
    testDb.exec(
      `INSERT INTO subscriptions (id, name, price, currency, cycle, status, billing_day, created_at)
       VALUES (1, 'Netflix', 1990, 'JPY', 'monthly', 'active', 15, '2026-01-01'),
              (2, 'Spotify', 980, 'JPY', 'monthly', 'active', 1, '2026-01-10'),
              (3, 'GitHub', 1000, 'USD', 'monthly', 'cancelled', 5, '2026-03-01'),
              (4, 'Dropbox', 12000, 'JPY', 'yearly', 'active', 1, '2026-01-01'),
              (5, 'Adobe', 1000, 'USD', 'monthly', 'active', 10, '2026-01-01')`,
    )
    const { handleGetSummary } = await import("../mcp/handlers.ts")
    const res = await handleGetSummary()
    const data = JSON.parse(res.content[0].text)
    expect(data.totalCount).toBe(4) // cancelled excluded
    expect(data.monthlyByCurrency.JPY).toBe(1990 + 980 + 1000) // Dropbox 12000/yr = 1000/mo
    expect(data.monthlyByCurrency.USD).toBe(1000)
  })

  test("handleGetUpcoming returns billings within period", async () => {
    const today = new Date().getDate()
    testDb.exec(
      `INSERT INTO subscriptions (id, name, price, currency, cycle, status, billing_day, created_at)
       VALUES (1, 'Netflix', 1990, 'JPY', 'monthly', 'active', ${today}, '2026-01-01'),
              (2, 'Spotify', 980, 'JPY', 'monthly', 'cancelled', 1, '2026-01-10')`,
    )
    const { handleGetUpcoming } = await import("../mcp/handlers.ts")
    const res = await handleGetUpcoming({ days: 30 })
    const data = JSON.parse(res.content[0].text)
    const names = data.map((e: { sub: { name: string } }) => e.sub.name)
    expect(names).toContain("Netflix")
    expect(names).not.toContain("Spotify")
  })

  test("handleGetUpcoming returns empty array on empty db", async () => {
    const { handleGetUpcoming } = await import("../mcp/handlers.ts")
    const res = await handleGetUpcoming({ days: 30 })
    const data = JSON.parse(res.content[0].text)
    expect(data).toEqual([])
  })

  test("handleGetCalendar returns entries for a given month", async () => {
    testDb.exec(
      `INSERT INTO subscriptions (id, name, price, currency, cycle, status, billing_day, created_at)
       VALUES (1, 'Netflix', 1990, 'JPY', 'monthly', 'active', 15, '2026-01-01'),
              (2, 'Spotify', 980, 'JPY', 'monthly', 'cancelled', 1, '2026-01-10')`,
    )
    const { handleGetCalendar } = await import("../mcp/handlers.ts")
    const res = await handleGetCalendar({ month: 3, year: 2026 })
    const data = JSON.parse(res.content[0].text)
    expect(data).toHaveLength(1)
    expect(data[0].day).toBe(15)
    expect(data[0].subs[0].name).toBe("Netflix")
  })

  test("handleExportData exports csv, json and md", async () => {
    testDb.exec(
      `INSERT INTO subscriptions (id, name, price, currency, cycle, status, billing_day, created_at)
       VALUES (1, 'Netflix', 1990, 'JPY', 'monthly', 'active', 15, '2026-01-01')`,
    )
    const { handleExportData } = await import("../mcp/handlers.ts")

    const csv = await handleExportData({ format: "csv" })
    expect(csv.isError).toBeUndefined()
    expect(csv.content[0].text).toContain("Netflix")
    expect(csv.content[0].text).toContain("1990")

    const json = await handleExportData({ format: "json" })
    const parsed = JSON.parse(json.content[0].text)
    expect(parsed).toHaveLength(1)
    expect(parsed[0].name).toBe("Netflix")

    const md = await handleExportData({ format: "md" })
    expect(md.isError).toBeUndefined()
    expect(md.content[0].text).toContain("| Netflix |")
  })

  test("handleExportData rejects unsupported format", async () => {
    const { handleExportData } = await import("../mcp/handlers.ts")
    const res = await handleExportData({ format: "xml" })
    expect(res.isError).toBe(true)
    expect(res.content[0].text).toMatch(/Unsupported format/)
  })

  test("handleGetHistory returns price history for a specific sub", async () => {
    testDb.exec(
      `INSERT INTO subscriptions (id, name, price, currency, cycle, status, billing_day, created_at)
       VALUES (1, 'Netflix', 1990, 'JPY', 'monthly', 'active', 15, '2026-01-01')`,
    )
    testDb.exec(
      `INSERT INTO price_history (subscription_id, old_price, new_price, old_currency, new_currency, changed_at)
       VALUES (1, 1500, 1990, 'JPY', 'JPY', '2026-02-01 10:00:00')`,
    )
    const { handleGetHistory } = await import("../mcp/handlers.ts")
    const res = await handleGetHistory({ id: 1 })
    const data = JSON.parse(res.content[0].text)
    expect(data).toHaveLength(1)
    expect(data[0].subscriptionName).toBe("Netflix")
    expect(data[0].oldPrice).toBe(1500)
    expect(data[0].newPrice).toBe(1990)
  })

  test("handleGetHistory returns all changes without id", async () => {
    testDb.exec(
      `INSERT INTO subscriptions (id, name, price, currency, cycle, status, billing_day, created_at)
       VALUES (1, 'Netflix', 1990, 'JPY', 'monthly', 'active', 15, '2026-01-01'),
              (2, 'Spotify', 980, 'JPY', 'monthly', 'active', 1, '2026-01-10')`,
    )
    testDb.exec(
      `INSERT INTO price_history (subscription_id, old_price, new_price, old_currency, new_currency, changed_at)
       VALUES (1, 1500, 1990, 'JPY', 'JPY', '2026-02-01 10:00:00'),
              (2, 800, 980, 'JPY', 'JPY', '2026-03-01 10:00:00')`,
    )
    const { handleGetHistory } = await import("../mcp/handlers.ts")
    const res = await handleGetHistory({})
    const data = JSON.parse(res.content[0].text)
    expect(data).toHaveLength(2)
    expect(data.map((e: { subscriptionId: number }) => e.subscriptionId).sort()).toEqual([1, 2])
  })

  test("handleGetForecast returns forecast for the requested months", async () => {
    testDb.exec(
      `INSERT INTO subscriptions (id, name, price, currency, cycle, status, billing_day, created_at)
       VALUES (1, 'Netflix', 1990, 'JPY', 'monthly', 'active', 15, '2026-01-01'),
              (2, 'Spotify', 980, 'JPY', 'monthly', 'cancelled', 1, '2026-01-10')`,
    )
    const { handleGetForecast } = await import("../mcp/handlers.ts")
    const res = await handleGetForecast({ months: 3 })
    const data = JSON.parse(res.content[0].text)
    expect(data.months).toBe(3)
    expect(data.totalSubscriptions).toBe(1)
    expect(data.entries[0].name).toBe("Netflix")
    expect(data.monthlyTotal).toBe(1990)
    expect(data.yearlyTotal).toBe(1990 * 12)
  })

  test("handleGetForecast excludes subs via cancel param", async () => {
    testDb.exec(
      `INSERT INTO subscriptions (id, name, price, currency, cycle, status, billing_day, created_at)
       VALUES (1, 'Netflix', 1990, 'JPY', 'monthly', 'active', 15, '2026-01-01'),
              (2, 'Spotify', 980, 'JPY', 'monthly', 'active', 1, '2026-01-10')`,
    )
    const { handleGetForecast } = await import("../mcp/handlers.ts")
    const res = await handleGetForecast({ cancel: "Spotify" })
    const data = JSON.parse(res.content[0].text)
    expect(data.totalSubscriptions).toBe(1)
    expect(data.entries[0].name).toBe("Netflix")
  })

  test("handleGetForecast converts to currency using fx rates", async () => {
    const { fetchFxRates } = await import("../fx.ts")
    vi.mocked(fetchFxRates).mockResolvedValue({ base: "USD", rates: { USD: 1, JPY: 150 } })
    testDb.exec(
      `INSERT INTO subscriptions (id, name, price, currency, cycle, status, billing_day, created_at)
       VALUES (1, 'GitHub', 1000, 'USD', 'monthly', 'active', 5, '2026-01-01')`,
    )
    const { handleGetForecast } = await import("../mcp/handlers.ts")
    const res = await handleGetForecast({ currency: "JPY" })
    const data = JSON.parse(res.content[0].text)
    expect(data.currency).toBe("JPY")
    expect(data.entries[0].monthlyConverted).toBe(150000)
    expect(data.monthlyTotal).toBe(150000)
  })

  test("handleCompare computes current vs previous period totals", async () => {
    testDb.exec(
      `INSERT INTO subscriptions (id, name, price, currency, cycle, status, billing_day, created_at)
       VALUES (1, 'Netflix', 1990, 'JPY', 'monthly', 'active', 15, '2026-01-01')`,
    )
    testDb.exec(
      `INSERT INTO price_history (subscription_id, old_price, new_price, old_currency, new_currency, changed_at)
       VALUES (1, 1500, 1990, 'JPY', 'JPY', '2026-02-01 10:00:00')`,
    )
    const { handleCompare } = await import("../mcp/handlers.ts")
    const res = await handleCompare({ period: "monthly" })
    const data = JSON.parse(res.content[0].text)
    expect(data.period).toBe("monthly")
    const row = data.rows[0]
    expect(row.currency).toBe("JPY")
    expect(row.current).toBe(1990)
    expect(row.previous).toBe(1500)
    expect(data.grandTotal.change).toBe(490)
    expect(data.grandTotal.changePercent).toBe(32.67)
  })

  test("handleCompare honors a non-default period", async () => {
    testDb.exec(
      `INSERT INTO subscriptions (id, name, price, currency, cycle, status, billing_day, created_at)
       VALUES (1, 'Netflix', 1990, 'JPY', 'monthly', 'active', 15, '2026-01-01')`,
    )
    testDb.exec(
      `INSERT INTO price_history (subscription_id, old_price, new_price, old_currency, new_currency, changed_at)
       VALUES (1, 1500, 1990, 'JPY', 'JPY', '2026-02-01 10:00:00')`,
    )
    const { handleCompare } = await import("../mcp/handlers.ts")
    const res = await handleCompare({ period: "yearly" })
    const data = JSON.parse(res.content[0].text)
    expect(data.period).toBe("yearly")
    expect(data.rows[0].current).toBe(1990 * 12)
    expect(data.rows[0].previous).toBe(1500 * 12)
  })

  test("handleGetTrials returns all trials", async () => {
    testDb.exec(
      `INSERT INTO trials (name, expires_at, price, currency, cycle, notes)
       VALUES ('Figma', '2099-12-31', 0, 'USD', 'monthly', NULL),
              ('Linear', '2099-11-30', 800, 'USD', 'monthly', NULL)`,
    )
    const { handleGetTrials } = await import("../mcp/handlers.ts")
    const res = await handleGetTrials({})
    const data = JSON.parse(res.content[0].text)
    expect(data).toHaveLength(2)
    expect(data.map((t: { name: string }) => t.name)).toEqual(["Linear", "Figma"])
  })

  test("handleGetTrials with expiring_soon filters to near-term trials", async () => {
    testDb.exec(
      `INSERT INTO trials (name, expires_at, price, currency, cycle, notes)
       VALUES ('Future', '2099-12-31', 0, 'USD', 'monthly', NULL)`,
    )
    const { handleGetTrials } = await import("../mcp/handlers.ts")
    const res = await handleGetTrials({ expiring_soon: 30 })
    const data = JSON.parse(res.content[0].text)
    expect(data).toEqual([])
  })

  test("handleGetTagSubscriptions filters by multiple tags (AND)", async () => {
    testDb.exec(
      `INSERT INTO subscriptions (id, name, price, currency, cycle, status, billing_day, created_at)
       VALUES (1, 'Netflix', 1990, 'JPY', 'monthly', 'active', 15, '2026-01-01'),
              (2, 'Spotify', 980, 'JPY', 'monthly', 'active', 1, '2026-01-10'),
              (3, 'GitHub', 1000, 'USD', 'monthly', 'active', 5, '2026-01-01')`,
    )
    testDb.exec(`INSERT INTO tags (id, name) VALUES (1, 'video'), (2, 'music'), (3, 'dev')`)
    testDb.exec(`INSERT INTO subscription_tags (subscription_id, tag_id) VALUES (1, 1), (2, 2), (3, 3), (1, 3)`)

    const { handleGetTagSubscriptions } = await import("../mcp/handlers.ts")
    const res = await handleGetTagSubscriptions({ tag: "video,dev" })
    const data = JSON.parse(res.content[0].text)
    expect(data).toHaveLength(1)
    expect(data[0].name).toBe("Netflix")
  })
})
