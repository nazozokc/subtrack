import { describe, expect, test, vi } from "vitest"
import { daysUntil } from "@subtrack/lib/date"
import { calculateMonthlyTotal, calculateTotals } from "../domain/billing.ts"
import type { SharedArgs } from "../types.ts"

const sub = (overrides: Partial<SharedArgs> = {}): SharedArgs => ({
  id: 1,
  name: "Example",
  price: 1200,
  currency: "JPY",
  cycle: "monthly",
  tags: [],
  status: "active",
  billingDay: null,
  createdAt: "2026-01-01",
  notes: null,
  paymentMethod: null,
  contractStart: null,
  contractEnd: null,
  autoRenewal: true,
  vendorName: null,
  vendorUrl: null,
  planTier: null,
  discountAmount: null,
  discountType: null,
  ...overrides,
})

describe("billing domain", () => {
  test("calculates totals without presentation or database dependencies", () => {
    expect(calculateTotals([
      sub(),
      sub({ id: 2, price: 12000, cycle: "yearly" }),
      sub({ id: 3, currency: "USD", price: 10 }),
    ], "monthly")).toEqual({ JPY: 2200, USD: 10 })
  })

  test("excludes cancelled subscriptions", () => {
    expect(calculateTotals([sub({ status: "cancelled" })], "monthly")).toEqual({})
  })

  test("normalizes a subscription to monthly cost", () => {
    expect(calculateMonthlyTotal(sub({ price: 12000, cycle: "yearly" }))).toBe(1000)
  })

  test("daysUntil counts calendar days across a DST transition", () => {
    const originalTz = process.env.TZ
    process.env.TZ = "America/New_York"
    vi.useFakeTimers()
    try {
      vi.setSystemTime(new Date(2026, 2, 8, 12))
      expect(daysUntil("2026-03-09")).toBe(1)
    } finally {
      vi.useRealTimers()
      if (originalTz === undefined) delete process.env.TZ
      else process.env.TZ = originalTz
    }
  })
})

