import { test, expect, describe, beforeEach, afterEach } from "vitest"
import type { SharedArgs } from "../types.ts"
import { fetchConvertedSubs, tryConvertSubs, convertAmounts } from "../fx.ts"
import type { FxRates } from "../fx.ts"

let originalFetch: typeof globalThis.fetch

beforeEach(() => {
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
    status: "active",
    billingDay: null,
    createdAt: "2026-01-01",
    notes: null,
    paymentMethod: null,
    ...overrides,
  }
}

const rates: FxRates = { base: "USD", rates: { JPY: 160, USD: 1 } }

describe("fetchConvertedSubs", () => {
  test("converts the list to the target currency", async () => {
    const { list, hasMissing } = await fetchConvertedSubs(
      [makeSub({ name: "US", price: 10, currency: "USD" })],
      "JPY",
    )
    expect(hasMissing).toBe(false)
    expect(list[0]!.currency).toBe("JPY")
    expect(list[0]!.price).toBe(1600)
  })

  test("keeps entries with a missing rate in their original currency", async () => {
    const { list, hasMissing } = await fetchConvertedSubs(
      [makeSub({ name: "EUR", price: 55, currency: "EUR" })],
      "JPY",
    )
    expect(hasMissing).toBe(true)
    expect(list[0]!.currency).toBe("EUR")
    expect(list[0]!.price).toBe(55)
  })

  test("returns null when the rate fetch fails", async () => {
    globalThis.fetch = async () => {
      throw new Error("Network error")
    }
    expect(await fetchConvertedSubs([makeSub()], "JPY")).toBeNull()
  })
})

describe("tryConvertSubs", () => {
  test("rounds converted prices to currency precision", () => {
    const { list } = tryConvertSubs(
      [makeSub({ name: "US", price: 995, currency: "USD" })],
      "JPY",
      rates,
    )
    expect(list[0]!.price).toBe(159_200) // 995 × 160
    expect(list[0]!.currency).toBe("JPY")
  })

  test("preserves converted decimal prices", () => {
    const { list } = tryConvertSubs(
      [makeSub({ name: "Decimal", price: 9.99, currency: "USD" })],
      "JPY",
      rates,
    )
    expect(list[0]!.price).toBe(1598.4)
  })

  test("flags entries whose rate is missing", () => {
    const { list, hasMissing } = tryConvertSubs(
      [
        makeSub({ name: "JP", price: 1000, currency: "JPY" }),
        makeSub({ name: "KRW", price: 1000, currency: "KRW" }),
      ],
      "JPY",
      rates,
    )
    expect(hasMissing).toBe(true)
    expect(list[0]!.currency).toBe("JPY")
    expect(list[1]!.currency).toBe("KRW")
  })
})

describe("convertAmounts", () => {
  test("returns per-entry results and flags missing rates", () => {
    const { converted, hasMissing } = convertAmounts(
      [
        { amount: 1000, currency: "JPY" },
        { amount: 10, currency: "USD" },
        { amount: 5, currency: "EUR" },
      ],
      "JPY",
      rates,
    )
    expect(converted).toEqual([1000, 1600, null])
    expect(hasMissing).toBe(true)
  })

  test("returns all converted values when no rate is missing", () => {
    const { converted, hasMissing } = convertAmounts(
      [
        { amount: 1000, currency: "JPY" },
        { amount: 1600, currency: "JPY" },
      ],
      "JPY",
      rates,
    )
    expect(converted).toEqual([1000, 1600])
    expect(hasMissing).toBe(false)
  })
})