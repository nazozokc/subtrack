import { test, expect, beforeAll, afterAll, beforeEach, vi } from "vitest"
import initSqlJs from "sql.js"
import type { Database } from "sql.js"
import { mkdtempSync, writeFileSync, existsSync, rmSync } from "node:fs"
import { join } from "node:path"
import { tmpdir } from "node:os"

// Mock consola to capture output
vi.mock("consola", () => {
  const logMessages: string[] = []
  const infoMessages: string[] = []
  const successMessages: string[] = []
  const errorMessages: string[] = []
  const failMessages: string[] = []
  const warnMessages: string[] = []

  // Expose for test assertions
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

// Mock pricing module for LLM usage tests
vi.mock("./pricing.ts", () => ({
  ensurePricingCache: vi.fn().mockResolvedValue({
    "gpt-4o": {
      input_cost_per_token: 2.5e-6,
      output_cost_per_token: 1e-5,
      litellm_provider: "openai",
    },
  }),
  matchModel: vi.fn().mockImplementation(
    (_cache: Record<string, unknown>, _provider: string, model: string) => {
      // Simple lookup for test cache
      const cache: Record<string, unknown> = {
        "gpt-4o": {
          input_cost_per_token: 2.5e-6,
          output_cost_per_token: 1e-5,
          litellm_provider: "openai",
        },
      }
      return cache[model] ?? null
    },
  ),
  calculateCostCents: vi.fn().mockReturnValue(0.75),
  getModelPricingDirect: vi.fn().mockResolvedValue(null),
  refreshPricingCache: vi.fn().mockResolvedValue(null),
}))

// Mock @inquirer/prompts to avoid interactive prompts
vi.mock("@inquirer/prompts", () => ({
  input: vi.fn(),
  confirm: vi.fn(),
  checkbox: vi.fn(),
  select: vi.fn(),
}))

import { input, confirm, checkbox, select } from "@inquirer/prompts"
import { consola, logMessages, infoMessages, successMessages, errorMessages, failMessages, warnMessages } from "consola"

let testDb: Database
let tmpDir: string

let originalFetch: typeof globalThis.fetch
let exitSpy: ReturnType<typeof vi.spyOn>

beforeAll(async () => {
  const SQL = await initSqlJs()
  testDb = new SQL.Database()
  testDb.run("PRAGMA foreign_keys = ON")
  testDb.run(`CREATE TABLE IF NOT EXISTS subscriptions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    price INTEGER NOT NULL,
    currency TEXT NOT NULL,
    cycle TEXT NOT NULL
  )`)
  testDb.run(`CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
  )`)
  testDb.run(`CREATE TABLE IF NOT EXISTS subscription_tags (
    subscription_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,
    PRIMARY KEY (subscription_id, tag_id),
    FOREIGN KEY (subscription_id) REFERENCES subscriptions(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
  )`)
  testDb.run(`CREATE TABLE IF NOT EXISTS llm_usage (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    cost REAL NOT NULL,
    date TEXT NOT NULL,
    description TEXT
  )`)

  const db = await import("./db.ts")
  db.__setDb(testDb)

  originalFetch = globalThis.fetch
  globalThis.fetch = async () =>
    new Response(
      JSON.stringify({ base: "USD", rates: { JPY: 160, USD: 1 } }),
    )

  exitSpy = vi.spyOn(process, "exit").mockImplementation((() => {
    // prevent process.exit from killing the test runner
  }) as () => never)
})

beforeEach(() => {
  testDb.run("DELETE FROM subscription_tags")
  testDb.run("DELETE FROM tags")
  testDb.run("DELETE FROM subscriptions")
  testDb.run("DELETE FROM llm_usage")

  logMessages.length = 0
  infoMessages.length = 0
  successMessages.length = 0
  errorMessages.length = 0
  failMessages.length = 0
  warnMessages.length = 0

  vi.mocked(input).mockReset().mockResolvedValue("")
  vi.mocked(confirm).mockReset().mockResolvedValue(true)
  vi.mocked(checkbox).mockReset().mockResolvedValue([])
  vi.mocked(select).mockReset()
})

afterAll(() => {
  testDb.close()
  globalThis.fetch = originalFetch
  exitSpy.mockRestore()
  if (tmpDir && existsSync(tmpDir)) rmSync(tmpDir, { recursive: true })
})

// ── Helpers ──────────────────────────────────────────────

function createTempDir(): string {
  tmpDir = mkdtempSync(join(tmpdir(), "subtrack-test-"))
  return tmpDir
}

function writeTempFile(filename: string, content: string): string {
  const dir = tmpDir || createTempDir()
  const filePath = join(dir, filename)
  writeFileSync(filePath, content, "utf-8")
  return filePath
}

// ── parseCsvLine ─────────────────────────────────────────

test("parseCsvLine parses simple fields", async () => {
  const { parseCsvLine } = await import("./import-csv.ts")
  const result = parseCsvLine("a,b,c")
  expect(result).toEqual(["a", "b", "c"])
})

test("parseCsvLine handles quoted fields with commas", async () => {
  const { parseCsvLine } = await import("./import-csv.ts")
  const result = parseCsvLine('"a,b",c')
  expect(result).toEqual(["a,b", "c"])
})

test("parseCsvLine handles escaped quotes", async () => {
  const { parseCsvLine } = await import("./import-csv.ts")
  const result = parseCsvLine('"say ""hello""",end')
  expect(result).toEqual(['say "hello"', "end"])
})

test("parseCsvLine handles empty fields", async () => {
  const { parseCsvLine } = await import("./import-csv.ts")
  const result = parseCsvLine("a,,c,")
  expect(result).toEqual(["a", "", "c", ""])
})

// ── handleTagList ────────────────────────────────────────

test("handleTagList shows info when no tags exist", async () => {
  const { handleTagList } = await import("./commands.ts")
  handleTagList()
  expect(infoMessages).toContain("No tags found")
})

test("handleTagList displays tags with counts", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "S1", price: 100, currency: "USD", cycle: "monthly", tags: ["video"] })
  db.writeSubscription({ name: "S2", price: 200, currency: "JPY", cycle: "monthly", tags: ["video"] })
  db.writeSubscription({ name: "S3", price: 300, currency: "JPY", cycle: "monthly", tags: ["storage"] })

  const { handleTagList } = await import("./commands.ts")
  handleTagList()
  const combined = logMessages.join("\n")
  expect(combined).toContain("video")
  expect(combined).toContain("2")
  expect(combined).toContain("storage")
  expect(combined).toContain("1")
})

// ── handleTagRename ──────────────────────────────────────

test("handleTagRename shows error when names are empty", async () => {
  const { handleTagRename } = await import("./commands.ts")
  handleTagRename("", "")
  expect(errorMessages.length).toBeGreaterThan(0)
})

test("handleTagRename shows error for non-existent tag", async () => {
  const { handleTagRename } = await import("./commands.ts")
  handleTagRename("nonexistent", "new")
  expect(errorMessages.some((m) => m.includes("not found"))).toBe(true)
})

test("handleTagRename renames a tag successfully", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "S1", price: 100, currency: "USD", cycle: "monthly", tags: ["old"] })

  const { handleTagRename } = await import("./commands.ts")
  handleTagRename("old", "new")
  expect(successMessages.some((m) => m.includes("old") && m.includes("new"))).toBe(true)

  const tags = db.getTagsWithCount()
  expect(tags.find((t) => t.name === "old")).toBeUndefined()
  expect(tags.find((t) => t.name === "new")).toBeDefined()
})

// ── handleTagDelete ──────────────────────────────────────

test("handleTagDelete shows error when name is empty", async () => {
  const { handleTagDelete } = await import("./commands.ts")
  handleTagDelete("")
  expect(errorMessages.length).toBeGreaterThan(0)
})

test("handleTagDelete shows error for non-existent tag", async () => {
  const { handleTagDelete } = await import("./commands.ts")
  handleTagDelete("nonexistent")
  expect(errorMessages.some((m) => m.includes("not found"))).toBe(true)
})

test("handleTagDelete deletes a tag successfully", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "S1", price: 100, currency: "USD", cycle: "monthly", tags: ["delete-me"] })

  const { handleTagDelete } = await import("./commands.ts")
  handleTagDelete("delete-me")
  expect(successMessages.some((m) => m.includes("delete-me"))).toBe(true)
  expect(db.getTagsWithCount()).toHaveLength(0)
})

// ── handleTagPrune ───────────────────────────────────────

test("handleTagPrune shows info when no orphaned tags", async () => {
  const { handleTagPrune } = await import("./commands.ts")
  handleTagPrune()
  expect(infoMessages.some((m) => m.includes("No orphaned"))).toBe(true)
})

test("handleTagPrune removes orphaned tags", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "S1", price: 100, currency: "USD", cycle: "monthly", tags: ["keep"] })
  const [sub] = db.getSubscriptions()
  db.deleteSubscription(sub.id)
  testDb.run("INSERT INTO tags (name) VALUES ('orphan1'), ('orphan2')")

  const { handleTagPrune } = await import("./commands.ts")
  handleTagPrune()
  expect(successMessages.some((m) => m.includes("3 orphaned"))).toBe(true)
})

// ── handleExport ─────────────────────────────────────────

test("handleExport shows error for unsupported format", async () => {
  const { handleExport } = await import("./commands.ts")
  await handleExport("pdf", {})
  expect(errorMessages.length).toBeGreaterThan(0)
  expect(errorMessages[0].toLowerCase()).toContain("unsupported")
})

test("handleExport shows info when no subscriptions", async () => {
  const { handleExport } = await import("./commands.ts")
  await handleExport("csv", {})
  expect(infoMessages).toContain("No subscriptions found")
})

test("handleExport outputs JSON for json format", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "Netflix", price: 1500, currency: "JPY", cycle: "monthly", tags: ["video"] })

  const { handleExport } = await import("./commands.ts")
  await handleExport("json", {})
  const combined = logMessages.join("\n")
  const parsed = JSON.parse(combined)
  expect(parsed).toHaveLength(1)
  expect(parsed[0].name).toBe("Netflix")
})

test("handleExport outputs Markdown for md format", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "Netflix", price: 1500, currency: "JPY", cycle: "monthly", tags: [] })

  const { handleExport } = await import("./commands.ts")
  await handleExport("md", {})
  const combined = logMessages.join("\n")
  expect(combined).toContain("Netflix")
  expect(combined).toContain("|")
  expect(combined).toContain("¥1,500")
})

test("handleExport filters by tags", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "A", price: 100, currency: "USD", cycle: "monthly", tags: ["video"] })
  db.writeSubscription({ name: "B", price: 200, currency: "USD", cycle: "monthly", tags: ["audio"] })

  const { handleExport } = await import("./commands.ts")
  await handleExport("csv", { tags: "video" })
  const combined = logMessages.join("\n")
  expect(combined).toContain("A")
  expect(combined).not.toContain("B")
})

test("handleExport with currency converts prices", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "JP", price: 1600, currency: "JPY", cycle: "monthly", tags: [] })

  const { handleExport } = await import("./commands.ts")
  await handleExport("csv", { currency: "USD" })
  const combined = logMessages.join("\n")
  expect(combined).toContain("10")
  expect(combined).toContain("USD")
})

test("handleExport with currency falls back when fetch fails", async () => {
  globalThis.fetch = async () => { throw new Error("Network error") }
  const db = await import("./db.ts")
  db.writeSubscription({ name: "JP", price: 1000, currency: "JPY", cycle: "monthly", tags: [] })

  const { handleExport } = await import("./commands.ts")
  await handleExport("csv", { currency: "USD" })
  expect(failMessages.length).toBeGreaterThan(0)
  expect(failMessages[0]).toContain("Failed to fetch exchange rates")

  globalThis.fetch = async () =>
    new Response(JSON.stringify({ base: "USD", rates: { JPY: 160, USD: 1 } }))
})

// ── handleList ───────────────────────────────────────────

test("handleList delegates to spreadSubscription", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "Netflix", price: 1500, currency: "JPY", cycle: "monthly", tags: [] })

  const { handleList } = await import("./commands.ts")
  await handleList({})
  const combined = logMessages.join("\n")
  expect(combined).toContain("Netflix")
  expect(combined).toContain("JPY TOTAL")
})

test("handleList passes sort and desc to getSubscriptions", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "B", price: 200, currency: "USD", cycle: "monthly", tags: [] })
  db.writeSubscription({ name: "A", price: 100, currency: "USD", cycle: "monthly", tags: [] })

  const { handleList } = await import("./commands.ts")
  await handleList({ sort: "name", desc: false })
  const combined = logMessages.join("\n")
  const aIdx = combined.indexOf("A")
  const bIdx = combined.indexOf("B")
  expect(aIdx).toBeLessThan(bIdx)
})

// ── handleEdit (non-interactive, with flags) ────────────

test("handleEdit shows info when no subscriptions", async () => {
  const { handleEdit } = await import("./commands.ts")
  await handleEdit(1, { name: "New Name" })
  expect(infoMessages).toContain("No subscriptions found")
})

test("handleEdit shows error for non-existent id", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "S1", price: 100, currency: "USD", cycle: "monthly", tags: [] })

  const { handleEdit } = await import("./commands.ts")
  await handleEdit(999, { name: "New Name" })
  expect(errorMessages.some((m) => m.includes("not found"))).toBe(true)
})

test("handleEdit updates name with --name flag", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "OldName", price: 1000, currency: "JPY", cycle: "monthly", tags: [] })
  const [sub] = db.getSubscriptions()

  const { handleEdit } = await import("./commands.ts")
  await handleEdit(sub.id, { name: "NewName" })
  expect(successMessages.some((m) => m.includes("NewName"))).toBe(true)

  const updated = db.getSubscription(sub.id)
  expect(updated?.name).toBe("NewName")
})

test("handleEdit updates price with --price flag", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "S1", price: 500, currency: "JPY", cycle: "monthly", tags: [] })
  const [sub] = db.getSubscriptions()

  const { handleEdit } = await import("./commands.ts")
  await handleEdit(sub.id, { price: "999" })
  expect(successMessages.length).toBeGreaterThan(0)

  const updated = db.getSubscription(sub.id)
  expect(updated?.price).toBe(999)
})

test("handleEdit updates tags with --tags flag", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "S1", price: 500, currency: "JPY", cycle: "monthly", tags: ["old"] })
  const [sub] = db.getSubscriptions()

  const { handleEdit } = await import("./commands.ts")
  await handleEdit(sub.id, { tags: "new1, new2" })
  expect(successMessages.length).toBeGreaterThan(0)

  const updated = db.getSubscription(sub.id)
  expect(updated?.tags).toEqual(["new1", "new2"])
})

// ── handleEdit (interactive, with mocked prompts) ───────

test("handleEdit interactive: select picks the subscription", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "S1", price: 1000, currency: "JPY", cycle: "monthly", tags: ["x"] })
  const [sub] = db.getSubscriptions()

  vi.mocked(select).mockResolvedValue(sub)
  vi.mocked(checkbox).mockResolvedValue(["name"])
  vi.mocked(input).mockResolvedValue("Renamed")
  vi.mocked(confirm).mockResolvedValue(true)

  const { handleEdit } = await import("./commands.ts")
  await handleEdit()
  expect(successMessages.some((m) => m.includes("Renamed"))).toBe(true)

  const updated = db.getSubscription(sub.id)
  expect(updated?.name).toBe("Renamed")
})

test("handleEdit interactive: cancels when no fields selected", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "S1", price: 1000, currency: "JPY", cycle: "monthly", tags: [] })
  const [sub] = db.getSubscriptions()

  vi.mocked(select).mockResolvedValue(sub)
  vi.mocked(checkbox).mockResolvedValue([])

  const { handleEdit } = await import("./commands.ts")
  await handleEdit()
  expect(infoMessages.some((m) => m.includes("Cancelled"))).toBe(true)
})

test("handleEdit interactive: cancels when confirm is declined", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "S1", price: 1000, currency: "JPY", cycle: "monthly", tags: [] })
  const [sub] = db.getSubscriptions()

  vi.mocked(select).mockResolvedValue(sub)
  vi.mocked(checkbox).mockResolvedValue(["name"])
  vi.mocked(input).mockResolvedValue("Renamed")
  vi.mocked(confirm).mockResolvedValue(false)

  const { handleEdit } = await import("./commands.ts")
  await handleEdit()
  expect(infoMessages.some((m) => m.includes("Cancelled"))).toBe(true)
  const updated = db.getSubscription(sub.id)
  expect(updated?.name).toBe("S1")
})

// ── handleImport ─────────────────────────────────────────

test("handleImport shows error when no file argument", async () => {
  const { handleImport } = await import("./import-csv.ts")
  await handleImport("", {})
  expect(errorMessages.length).toBeGreaterThan(0)
  expect(errorMessages[0].toLowerCase()).toContain("usage")
})

test("handleImport shows error for non-existent file", async () => {
  const { handleImport } = await import("./import-csv.ts")
  await handleImport("/nonexistent/file.csv", {})
  expect(errorMessages.some((m) => m.includes("not found"))).toBe(true)
})

test("handleImport shows error for invalid CSV header", async () => {
  const filePath = writeTempFile("bad-header.csv", "name,price\na,100")
  const { handleImport } = await import("./import-csv.ts")
  await handleImport(filePath, {})
  expect(errorMessages.length).toBeGreaterThan(0)
  expect(errorMessages[0].toLowerCase()).toContain("invalid csv header")
})

test("handleImport imports valid CSV data", async () => {
  const csv = "\uFEFFname,cycle,tags,price,currency\nNetflix,monthly,video;entertainment,1500,JPY\nDropbox,monthly,storage,10,USD"
  const filePath = writeTempFile("valid.csv", csv)

  const { handleImport } = await import("./import-csv.ts")
  await handleImport(filePath, {})

  const db = await import("./db.ts")
  const subs = db.getSubscriptions()
  expect(subs).toHaveLength(2)
  expect(subs[0].name).toBe("Netflix")
  expect(subs[0].tags).toEqual(["video", "entertainment"])
  expect(subs[1].name).toBe("Dropbox")
  expect(subs[1].price).toBe(10)
  expect(subs[1].currency).toBe("USD")
  expect(successMessages.some((m) => m.includes("2 imported"))).toBe(true)
})

test("handleImport supports dry-run mode", async () => {
  const csv = "name,cycle,tags,price,currency\nNetflix,monthly,,1500,JPY"
  const filePath = writeTempFile("dryrun.csv", csv)

  const { handleImport } = await import("./import-csv.ts")
  await handleImport(filePath, { dryRun: true })

  const db = await import("./db.ts")
  expect(db.getSubscriptions()).toHaveLength(0)
  expect(successMessages.some((m) => m.includes("Dry-run"))).toBe(true)
})

test("handleImport skips invalid rows", async () => {
  const csv = "name,cycle,tags,price,currency\nValid,monthly,,100,JPY\n,monthly,,abc,JPY\nBadPrice,monthly,,notanumber,JPY\nBadCurrency,monthly,,100,ZZ" // ZZ fails regex /^[A-Z]{3}$/
  const filePath = writeTempFile("partial.csv", csv)

  const { handleImport } = await import("./import-csv.ts")
  await handleImport(filePath, {})

  const db = await import("./db.ts")
  expect(db.getSubscriptions()).toHaveLength(1)
  expect(db.getSubscriptions()[0].name).toBe("Valid")
  expect(successMessages.some((m) => m.includes("1 imported"))).toBe(true)
  expect(warnMessages.length).toBeGreaterThan(0)
})

// ── handleSummary ────────────────────────────────────────

test("handleSummary shows info when no subscriptions", async () => {
  const { handleSummary } = await import("./commands.ts")
  await handleSummary()
  expect(infoMessages).toContain("No subscriptions found")
})

test("handleSummary displays summary data", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "Netflix", price: 1500, currency: "JPY", cycle: "monthly", tags: ["video"] })

  const { handleSummary } = await import("./commands.ts")
  await handleSummary()
  const combined = logMessages.join("\n")
  expect(combined).toContain("Total subscriptions:")
  expect(combined).toContain("Netflix")
  expect(combined).toContain("Monthly by currency:")
})

// ── handleTags ────────────────────────────────────────────

test("handleTags delegates to spreadSubscription with tag filter", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "A", price: 100, currency: "USD", cycle: "monthly", tags: ["video"] })
  db.writeSubscription({ name: "B", price: 200, currency: "USD", cycle: "monthly", tags: ["audio"] })

  const { handleTags } = await import("./commands.ts")
  await handleTags(["video"])
  const combined = logMessages.join("\n")
  expect(combined).toContain("A")
  expect(combined).not.toContain("B")
})

// ── handlePayment ─────────────────────────────────────────

test("handlePayment shows monthly total", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "Netflix", price: 1500, currency: "JPY", cycle: "monthly", tags: [] })

  const { handlePayment } = await import("./commands.ts")
  await handlePayment("monthly", {})
  const combined = logMessages.join("\n")
  expect(combined).toContain("¥1,500")
  expect(combined).toContain("/month")
})

test("handlePayment with --currency converts to target", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "Netflix", price: 1000, currency: "JPY", cycle: "monthly", tags: [] })

  const { handlePayment } = await import("./commands.ts")
  await handlePayment("monthly", { currency: "JPY" })
  const combined = logMessages.join("\n")
  expect(combined).toContain("¥1,000")
  expect(combined).toContain("/month")
})

// ── handleAdd ─────────────────────────────────────────────

test("handleAdd shows info when cancelled at confirm", async () => {
  // Mock prompts for resolveAddOptions
  vi.mocked(input)
    .mockResolvedValueOnce("New Service")   // name
    .mockResolvedValueOnce("2500")           // price
    .mockResolvedValueOnce("my-tag")         // tags
  vi.mocked(select)
    .mockResolvedValueOnce("JPY")            // currency
    .mockResolvedValueOnce("monthly")        // cycle
  vi.mocked(confirm).mockResolvedValueOnce(false) // decline

  const { handleAdd } = await import("./commands.ts")
  await handleAdd({})
  expect(infoMessages.some((m) => m.includes("Cancelled"))).toBe(true)
})

test("handleAdd creates subscription with prompted fields", async () => {
  vi.mocked(input)
    .mockResolvedValueOnce("Spotify")
    .mockResolvedValueOnce("980")
    .mockResolvedValueOnce("music")
  vi.mocked(select)
    .mockResolvedValueOnce("JPY")
    .mockResolvedValueOnce("monthly")
  vi.mocked(confirm).mockResolvedValueOnce(true)

  const { handleAdd } = await import("./commands.ts")
  await handleAdd({})
  expect(successMessages.some((m) => m.includes("Spotify"))).toBe(true)

  const db = await import("./db.ts")
  const subs = db.getSubscriptions()
  expect(subs).toHaveLength(1)
  expect(subs[0].name).toBe("Spotify")
  expect(subs[0].price).toBe(980)
})

test("handleAdd uses flags when provided (non-interactive)", async () => {
  const { handleAdd } = await import("./commands.ts")
  await handleAdd({
    name: "FlagService",
    price: "500",
    currency: "USD",
    cycle: "yearly",
    tags: "flag-test",
  })
  expect(successMessages.some((m) => m.includes("FlagService"))).toBe(true)

  const db = await import("./db.ts")
  const subs = db.getSubscriptions()
  expect(subs).toHaveLength(1)
  expect(subs[0].name).toBe("FlagService")
  expect(subs[0].price).toBe(500)
  expect(subs[0].currency).toBe("USD")
  expect(subs[0].cycle).toBe("yearly")
  expect(subs[0].tags).toEqual(["flag-test"])
})

// ── handleDelete ──────────────────────────────────────────

test("handleDelete shows info when no subscriptions", async () => {
  const { handleDelete } = await import("./commands.ts")
  await handleDelete()
  expect(infoMessages).toContain("No subscriptions found")
})

test("handleDelete deletes selected subscriptions", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "S1", price: 100, currency: "USD", cycle: "monthly", tags: [] })
  db.writeSubscription({ name: "S2", price: 200, currency: "USD", cycle: "monthly", tags: [] })
  const [s1, s2] = db.getSubscriptions()

  // Mock checkbox to return s1 (the first sub)
  vi.mocked(checkbox).mockResolvedValue([s1])
  vi.mocked(confirm).mockResolvedValue(true)

  const { handleDelete } = await import("./commands.ts")
  await handleDelete()

  const remaining = db.getSubscriptions()
  expect(remaining).toHaveLength(1)
  expect(remaining[0].name).toBe("S2")
  expect(successMessages.some((m) => m.includes("S1"))).toBe(true)
})

test("handleDelete cancels when no selection", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "S1", price: 100, currency: "USD", cycle: "monthly", tags: [] })

  vi.mocked(checkbox).mockResolvedValue([])

  const { handleDelete } = await import("./commands.ts")
  await handleDelete()

  expect(infoMessages.some((m) => m.includes("Cancelled"))).toBe(true)
  expect(db.getSubscriptions()).toHaveLength(1) // unchanged
})

test("handleDelete cancels when confirm is declined", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "S1", price: 100, currency: "USD", cycle: "monthly", tags: [] })
  const [s1] = db.getSubscriptions()

  vi.mocked(checkbox).mockResolvedValue([s1])
  vi.mocked(confirm).mockResolvedValue(false)

  const { handleDelete } = await import("./commands.ts")
  await handleDelete()

  expect(infoMessages.some((m) => m.includes("Cancelled"))).toBe(true)
  expect(db.getSubscriptions()).toHaveLength(1) // unchanged
})

// ── handleUsageAdd (non-interactive) ─────────────────────

test("handleUsageAdd creates entry with all flags", async () => {
  const db = await import("./db.ts")
  const { handleUsageAdd } = await import("./usage.ts")

  await handleUsageAdd({
    provider: "openai",
    model: "gpt-4o",
    inputTokens: "1000",
    outputTokens: "500",
    date: "2026-06-19",
    description: "test run",
  })

  const entries = db.getLlmUsage()
  expect(entries).toHaveLength(1)
  expect(entries[0].provider).toBe("openai")
  expect(entries[0].model).toBe("gpt-4o")
  expect(entries[0].input_tokens).toBe(1000)
  expect(entries[0].output_tokens).toBe(500)
  expect(entries[0].date).toBe("2026-06-19")
  expect(entries[0].description).toBe("test run")
  expect(entries[0].cost).toBeGreaterThan(0) // auto-calculated
})

test("handleUsageAdd with invalid provider shows error", async () => {
  const { handleUsageAdd } = await import("./usage.ts")
  await handleUsageAdd({
    provider: "nonexistent",
    model: "gpt-4o",
    inputTokens: "100",
    outputTokens: "50",
  })
  expect(errorMessages.length).toBeGreaterThan(0)
  expect(errorMessages[0].toLowerCase()).toContain("invalid provider")
})

test("handleUsageAdd with invalid tokens shows error", async () => {
  const { handleUsageAdd } = await import("./usage.ts")
  await handleUsageAdd({
    provider: "openai",
    model: "gpt-4o",
    inputTokens: "abc",
    outputTokens: "50",
  })
  expect(errorMessages.length).toBeGreaterThan(0)
})

test("handleUsageAdd with invalid date shows error", async () => {
  const { handleUsageAdd } = await import("./usage.ts")
  await handleUsageAdd({
    provider: "openai",
    model: "gpt-4o",
    inputTokens: "100",
    outputTokens: "50",
    date: "not-a-date",
  })
  expect(errorMessages.length).toBeGreaterThan(0)
})

// ── handleUsageList ──────────────────────────────────────

test("handleUsageList shows info when no entries", async () => {
  const { handleUsageList } = await import("./usage.ts")
  await handleUsageList({})
  expect(infoMessages.some((m) => m.includes("No usage entries"))).toBe(true)
})

test("handleUsageList displays entries", async () => {
  const db = await import("./db.ts")
  db.addLlmUsage({
    provider: "openai",
    model: "gpt-4o",
    input_tokens: 1000,
    output_tokens: 500,
    cost: 0.5,
    date: "2026-06-19",
    description: null,
  })

  const { handleUsageList } = await import("./usage.ts")
  await handleUsageList({})
  const combined = logMessages.join("\n")
  expect(combined).toContain("openai")
  expect(combined).toContain("gpt-4o")
  expect(combined).toContain("Total:")
})

// ── handleUsageDelete ─────────────────────────────────────

test("handleUsageDelete shows info when no entries", async () => {
  const { handleUsageDelete } = await import("./usage.ts")
  await handleUsageDelete()
  expect(infoMessages.some((m) => m.includes("No usage entries"))).toBe(true)
})

test("handleUsageDelete deletes selected entries", async () => {
  const db = await import("./db.ts")
  db.addLlmUsage({
    provider: "openai",
    model: "gpt-4o",
    input_tokens: 100,
    output_tokens: 50,
    cost: 0.1,
    date: "2026-06-19",
    description: null,
  })
  const entries = db.getLlmUsage()

  vi.mocked(checkbox).mockResolvedValue(entries)
  vi.mocked(confirm).mockResolvedValue(true)

  const { handleUsageDelete } = await import("./usage.ts")
  await handleUsageDelete()

  expect(db.getLlmUsage()).toHaveLength(0)
  expect(successMessages.some((m) => m.includes("openai"))).toBe(true)
})

// ── handleUsageRefresh ───────────────────────────────────

test("handleUsageRefresh shows failure when fetch fails", async () => {
  globalThis.fetch = async () => { throw new Error("Network error") }

  const { handleUsageRefresh } = await import("./usage.ts")
  await handleUsageRefresh()

  expect(failMessages.some((m) => m.includes("Failed to fetch"))).toBe(true)

  globalThis.fetch = async () =>
    new Response(JSON.stringify({ base: "USD", rates: { JPY: 160, USD: 1 } }))
})
