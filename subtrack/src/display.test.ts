import { test, expect, beforeEach, afterEach } from "vitest"
import { consola } from "consola"
import { spreadSubscription } from "./display"
import type { SharedArgs } from "./db"

const logMessages: string[] = []
const infoMessages: string[] = []
const failMessages: string[] = []
const errorMessages: string[] = []

let originalFetch: typeof globalThis.fetch

beforeEach(() => {
  logMessages.length = 0
  infoMessages.length = 0
  failMessages.length = 0
  errorMessages.length = 0

  consola.mockTypes((_type: string, _defaults: object) => {
    return (...args: unknown[]) => {
      const str = args.map((a) => String(a)).join(" ")
      if (_type === "log") logMessages.push(str)
      if (_type === "info") infoMessages.push(str)
      if (_type === "fail") failMessages.push(str)
      if (_type === "error") errorMessages.push(str)
    }
  })

  originalFetch = globalThis.fetch
  globalThis.fetch = async () =>
    new Response(
      JSON.stringify({
        base: "USD",
        rates: { JPY: 160, USD: 1 },
      }),
    )
})

afterEach(() => {
  consola.mockTypes()
  globalThis.fetch = originalFetch
})

function makeSub(overrides: Partial<SharedArgs> = {}): SharedArgs {
  return {
    id: 1,
    name: "Test Service",
    price: 1000,
    currency: "JPY",
    cycle: "monthly",
    tags: [],
    ...overrides,
  }
}

test("shows info message when no subscriptions", async () => {
  await spreadSubscription([])
  expect(infoMessages).toContain("No subscriptions found")
  expect(logMessages).toHaveLength(0)
})

test("displays table with single subscription", async () => {
  await spreadSubscription([makeSub({ name: "Netflix", price: 1500 })])

  expect(logMessages).toHaveLength(1)
  const table = logMessages[0]
  expect(table).toContain("Netflix")
  expect(table).toContain("¥1,500")
  expect(table).toContain("monthly")
  expect(table).toContain("JPY TOTAL")
})

test("displays table with JPY currency symbol", async () => {
  await spreadSubscription([
    makeSub({ name: "iCloud+", price: 400, currency: "JPY" }),
  ])

  const table = logMessages[0]
  expect(table).toContain("¥400")
  expect(table).toContain("JPY TOTAL")
})

test("displays table with USD currency symbol", async () => {
  await spreadSubscription([
    makeSub({ name: "GitHub", price: 10, currency: "USD" }),
  ])

  const table = logMessages[0]
  expect(table).toContain("$10")
})

test("displays tags column", async () => {
  await spreadSubscription([
    makeSub({ name: "Netflix", tags: ["video", "entertainment"] }),
  ])

  const table = logMessages[0]
  expect(table).toContain("video, entertainment")
})

test("displays dash when no tags", async () => {
  await spreadSubscription([makeSub({ name: "Dropbox", tags: [] })])

  const table = logMessages[0]
  expect(table).toContain("Dropbox")
  expect(table).toContain("-")
})

test("shows correct JPY total", async () => {
  await spreadSubscription([
    makeSub({ name: "A", price: 500, currency: "JPY" }),
    makeSub({ name: "B", price: 1500, currency: "JPY" }),
  ])

  const table = logMessages[0]
  expect(table).toContain("¥2,000")
  expect(table).toContain("JPY TOTAL")
})

test("shows correct USD total", async () => {
  await spreadSubscription([
    makeSub({ name: "A", price: 10, currency: "USD" }),
    makeSub({ name: "B", price: 20, currency: "USD" }),
  ])

  const table = logMessages[0]
  expect(table).toContain("$30")
  expect(table).toContain("USD TOTAL")
})

test("shows mixed currency totals correctly", async () => {
  await spreadSubscription([
    makeSub({ name: "JP", price: 1000, currency: "JPY" }),
    makeSub({ name: "US", price: 15, currency: "USD" }),
  ])

  const table = logMessages.filter(Boolean).join("\n")
  expect(table).toContain("¥1,000")
  expect(table).toContain("JPY TOTAL")
  expect(table).toContain("$15")
  expect(table).toContain("USD TOTAL")
})

test("shows yearly cycle", async () => {
  await spreadSubscription([makeSub({ name: "Annual", cycle: "yearly" })])

  const table = logMessages[0]
  expect(table).toContain("yearly")
})

test("displays multiple subscriptions", async () => {
  const subs = [
    makeSub({ id: 1, name: "A", price: 10, currency: "USD", tags: ["x"] }),
    makeSub({ id: 2, name: "B", price: 20, currency: "USD", tags: ["y"] }),
    makeSub({ id: 3, name: "C", price: 30, currency: "USD", tags: ["z"] }),
  ]

  await spreadSubscription(subs)

  const table = logMessages[0]
  expect(table).toContain("A")
  expect(table).toContain("B")
  expect(table).toContain("C")
  expect(table).toContain("$60")
  expect(table).toContain("USD TOTAL")
})

// --- --currency filter tests ---

test("currency JPY converts USD prices and shows single total", async () => {
  await spreadSubscription(
    [
      makeSub({ name: "Local", price: 1000, currency: "JPY" }),
      makeSub({ name: "Foreign", price: 10, currency: "USD" }),
    ],
    "JPY",
  )

  const table = logMessages[0]
  // JPY 1000 stays 1000
  expect(table).toContain("￥1,000")
  // USD 10 × 160 = 1600
  expect(table).toContain("￥1,600")
  // Single JPY TOTAL
  expect(table).toContain("JPY TOTAL")
  expect(table).toContain("￥2,600")
  // No USD TOTAL
  expect(table).not.toContain("USD TOTAL")
})

test("currency USD converts JPY prices and shows single total", async () => {
  await spreadSubscription(
    [
      makeSub({ name: "Local", price: 10, currency: "USD" }),
      makeSub({ name: "Foreign", price: 1600, currency: "JPY" }),
    ],
    "USD",
  )

  const table = logMessages[0]
  // USD 10 stays 10
  expect(table).toContain("$10.00")
  // JPY 1600 → 1600 / 160 = 10
  expect(table).toContain("$10.00")
  // Single USD TOTAL
  expect(table).toContain("USD TOTAL")
  // No JPY TOTAL
  expect(table).not.toContain("JPY TOTAL")
})

test("currency JPY with all JPY items shows correctly", async () => {
  await spreadSubscription(
    [
      makeSub({ name: "A", price: 500, currency: "JPY" }),
      makeSub({ name: "B", price: 1500, currency: "JPY" }),
    ],
    "JPY",
  )

  const table = logMessages[0]
  expect(table).toContain("￥500")
  expect(table).toContain("￥1,500")
  expect(table).toContain("￥2,000")
  expect(table).not.toContain("USD TOTAL")
})

test("currency falls back when fetch fails", async () => {
  globalThis.fetch = async () => {
    throw new Error("Network error")
  }

  await spreadSubscription(
    [
      makeSub({ name: "JP", price: 1000, currency: "JPY" }),
      makeSub({ name: "US", price: 15, currency: "USD" }),
    ],
    "JPY",
  )

  // Should log fail message
  expect(failMessages.length).toBeGreaterThan(0)
  expect(failMessages[0]).toContain("Failed to fetch exchange rates")

  // Should fall back to grouped-by-currency display
  const table = logMessages.filter(Boolean).join("\n")
  expect(table).toContain("¥1,000")
  expect(table).toContain("$15")
  expect(table).toContain("JPY TOTAL")
  expect(table).toContain("USD TOTAL")
})

test("currency with empty list shows info", async () => {
  await spreadSubscription([], "JPY")
  expect(infoMessages).toContain("No subscriptions found")
})

// ── exportCsv tests ──────────────────────────────────────

test("exportCsv returns header-only when no subscriptions", async () => {
  const { exportCsv } = await import("./display")
  const csv = exportCsv([])
  expect(csv).toBe("\uFEFFname,cycle,tags,price,currency")
})

test("exportCsv produces correct csv for single subscription", async () => {
  const { exportCsv } = await import("./display")
  const csv = exportCsv([makeSub({ name: "Netflix", price: 1500, currency: "JPY" })])
  const lines = csv.split("\n")
  expect(lines).toHaveLength(2)
  expect(lines[0]).toBe("\uFEFFname,cycle,tags,price,currency")
  expect(lines[1]).toBe("Netflix,monthly,,1500,JPY")
})

test("exportCsv handles tags correctly", async () => {
  const { exportCsv } = await import("./display")
  const csv = exportCsv([
    makeSub({ name: "Netflix", tags: ["video", "entertainment"] }),
  ])
  const lines = csv.split("\n")
  expect(lines[1]).toBe("Netflix,monthly,video;entertainment,1000,JPY")
})

test("exportCsv escapes commas in name", async () => {
  const { exportCsv } = await import("./display")
  const csv = exportCsv([
    makeSub({ name: "Service, Inc.", price: 100 }),
  ])
  const lines = csv.split("\n")
  expect(lines[1]).toBe('"Service, Inc.",monthly,,100,JPY')
})

test("exportCsv handles multiple subscriptions", async () => {
  const { exportCsv } = await import("./display")
  const csv = exportCsv([
    makeSub({ id: 1, name: "A", price: 10, currency: "USD", tags: ["x"] }),
    makeSub({ id: 2, name: "B", price: 20, currency: "USD", tags: ["y"] }),
  ])
  const lines = csv.split("\n")
  expect(lines).toHaveLength(3)
  expect(lines[1]).toBe("A,monthly,x,10,USD")
  expect(lines[2]).toBe("B,monthly,y,20,USD")
})

test("exportCsv begins with BOM", async () => {
  const { exportCsv } = await import("./display")
  const csv = exportCsv([makeSub()])
  expect(csv.charCodeAt(0)).toBe(0xfeff)
})

// ── exportMd tests ───────────────────────────────────────

test("exportMd returns header and separator when no subscriptions", async () => {
  const { exportMd } = await import("./display")
  const md = exportMd([])
  expect(md).toBe("| name | cycle | tags | price | currency |\n| --- | --- | --- | --- | --- |")
})

test("exportMd produces correct markdown for single subscription", async () => {
  const { exportMd } = await import("./display")
  const md = exportMd([makeSub({ name: "Netflix", price: 1500, currency: "JPY" })])
  const lines = md.split("\n")
  expect(lines).toHaveLength(3)
  expect(lines[2]).toBe("| Netflix | monthly | - | ¥1,500 | JPY |")
})

test("exportMd handles tags correctly", async () => {
  const { exportMd } = await import("./display")
  const md = exportMd([
    makeSub({ name: "Netflix", tags: ["video", "entertainment"] }),
  ])
  const lines = md.split("\n")
  expect(lines[2]).toBe("| Netflix | monthly | video, entertainment | ¥1,000 | JPY |")
})

test("exportMd handles multiple subscriptions", async () => {
  const { exportMd } = await import("./display")
  const md = exportMd([
    makeSub({ id: 1, name: "A", price: 10, currency: "USD", tags: ["x"] }),
    makeSub({ id: 2, name: "B", price: 20, currency: "USD", tags: [] }),
  ])
  const lines = md.split("\n")
  expect(lines).toHaveLength(4)
  expect(lines[2]).toBe("| A | monthly | x | $10 | USD |")
  expect(lines[3]).toBe("| B | monthly | - | $20 | USD |")
})

// ── showPayment tests ────────────────────────────────────

test("showPayment shows info when no subscriptions", async () => {
  const { showPayment } = await import("./display")
  await showPayment("monthly", undefined, [])
  expect(infoMessages).toContain("No subscriptions found")
})

test("showPayment shows monthly total for single currency", async () => {
  const { showPayment } = await import("./display")
  await showPayment("monthly", undefined, [
    makeSub({ name: "Netflix", price: 1500, currency: "JPY" }),
  ])
  expect(logMessages).toHaveLength(1)
  expect(logMessages[0]).toContain("¥1,500")
  expect(logMessages[0]).toContain("/month")
})

test("showPayment shows yearly total for single currency", async () => {
  const { showPayment } = await import("./display")
  await showPayment("yearly", undefined, [
    makeSub({ name: "Netflix", price: 1500, currency: "JPY" }),
  ])
  expect(logMessages).toHaveLength(1)
  expect(logMessages[0]).toContain("¥18,000")
  expect(logMessages[0]).toContain("/year")
})

test("showPayment shows per-currency totals for mixed currencies", async () => {
  const { showPayment } = await import("./display")
  await showPayment("monthly", undefined, [
    makeSub({ name: "Netflix", price: 1500, currency: "JPY" }),
    makeSub({ name: "GitHub", price: 10, currency: "USD" }),
  ])
  expect(logMessages).toHaveLength(2)
  const combined = logMessages.join("\n")
  expect(combined).toContain("JPY")
  expect(combined).toContain("¥1,500")
  expect(combined).toContain("USD")
  expect(combined).toContain("$10")
})

test("showPayment yearly converts all cycles correctly", async () => {
  const { showPayment } = await import("./display")
  await showPayment("yearly", undefined, [
    makeSub({ name: "Monthly", price: 1000, currency: "JPY", cycle: "monthly" }),
    makeSub({ name: "Yearly", price: 12000, currency: "JPY", cycle: "yearly" }),
  ])
  expect(logMessages).toHaveLength(1)
  // monthly 1000 => yearly 12000, yearly 12000 => yearly 12000 => total 24000
  expect(logMessages[0]).toContain("¥24,000")
})

test("showPayment weekly converts correctly", async () => {
  const { showPayment } = await import("./display")
  await showPayment("weekly", undefined, [
    makeSub({ name: "WeeklySub", price: 100, currency: "USD", cycle: "weekly" }),
  ])
  expect(logMessages[0]).toContain("$100")
  expect(logMessages[0]).toContain("/week")
})

test("showPayment with --currency converts all to target currency", async () => {
  const { showPayment } = await import("./display")
  await showPayment("monthly", "JPY", [
    makeSub({ name: "Local", price: 1000, currency: "JPY" }),
    makeSub({ name: "Foreign", price: 10, currency: "USD" }),
  ])
  // JPY 1000 stays 1000, USD 10 * 160 = 1600 => total 2600
  expect(logMessages).toHaveLength(1)
  expect(logMessages[0]).toContain("￥2,600")
})

test("showPayment --currency falls back when fetch fails", async () => {
  globalThis.fetch = async () => {
    throw new Error("Network error")
  }

  const { showPayment } = await import("./display")
  await showPayment("monthly", "JPY", [
    makeSub({ name: "JP", price: 1000, currency: "JPY" }),
    makeSub({ name: "US", price: 10, currency: "USD" }),
  ])

  // Should log fail message
  expect(failMessages.length).toBeGreaterThan(0)
  expect(failMessages[0]).toContain("Failed to fetch exchange rates")

  // Should fall back to per-currency display
  const combined = logMessages.join("\n")
  expect(combined).toContain("JPY")
  expect(combined).toContain("¥1,000")
  expect(combined).toContain("USD")
  expect(combined).toContain("$10")
})
