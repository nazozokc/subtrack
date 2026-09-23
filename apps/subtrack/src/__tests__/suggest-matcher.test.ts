import { test, expect, describe, beforeAll, beforeEach } from "vitest"
import { DatabaseSync } from "node:sqlite"
import { findMatches, hasPriceConflict } from "../suggest/matcher.ts"
import type { SharedArgs } from "../types.ts"

let testDb: DatabaseSync

function makeSub(overrides: Partial<SharedArgs> = {}): SharedArgs {
  return {
    id: 0,
    name: "Existing",
    price: 100,
    currency: "USD",
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
  testDb.exec("DELETE FROM subscription_tags")
  testDb.exec("DELETE FROM tags")
  testDb.exec("DELETE FROM subscriptions")
  testDb.exec("DELETE FROM suggestions")
})

describe("suggest matcher: findMatches", () => {
  test("exact case-insensitive name match returns exactMatch", async () => {
    const db = await import("../db.ts")
    db.writeSubscription({ name: "Netflix", price: 1490, currency: "JPY", cycle: "monthly", tags: [] })

    const res = findMatches({ name: "netflix", price: 1490 })
    expect(res.exactMatch).toBe(true)
    expect(res.matches).toHaveLength(1)
    expect(res.matches[0].name).toBe("Netflix")
  })

  test("fuzzy match on significant word overlap", async () => {
    const db = await import("../db.ts")
    db.writeSubscription({ name: "Netflix Premium", price: 1490, currency: "JPY", cycle: "monthly", tags: [] })

    const res = findMatches({ name: "Netflix Basic", price: 900 })
    expect(res.exactMatch).toBe(false)
    expect(res.matches.length).toBeGreaterThan(0)
    expect(res.matches[0].name).toBe("Netflix Premium")
  })

  test("matches when the suggestion name contains an existing service name", async () => {
    const db = await import("../db.ts")
    db.writeSubscription({ name: "Spotify", price: 980, currency: "JPY", cycle: "monthly", tags: [] })

    const res = findMatches({ name: "Spotify Premium", price: 1280 })
    expect(res.exactMatch).toBe(false)
    expect(res.matches.length).toBeGreaterThan(0)
    expect(res.matches[0].name).toBe("Spotify")
  })

  test("matches when the existing name contains the suggestion name", async () => {
    const db = await import("../db.ts")
    db.writeSubscription({ name: "GitHub Copilot Pro", price: 1000, currency: "USD", cycle: "monthly", tags: [] })

    const res = findMatches({ name: "GitHub Copilot", price: 1000 })
    expect(res.exactMatch).toBe(false)
    expect(res.matches.length).toBeGreaterThan(0)
  })

  test("returns no matches when nothing overlaps", async () => {
    const db = await import("../db.ts")
    db.writeSubscription({ name: "Netflix Premium", price: 1490, currency: "JPY", cycle: "monthly", tags: [] })

    const res = findMatches({ name: "Adobe Creative Cloud", price: 6500 })
    expect(res.exactMatch).toBe(false)
    expect(res.matches).toHaveLength(0)
  })
})

describe("suggest matcher: hasPriceConflict", () => {
  test("returns true when prices differ by more than 30%", () => {
    expect(hasPriceConflict({ price: 100, currency: "USD" }, makeSub({ price: 150 }))).toBe(true)
  })

  test("returns false when prices differ by exactly 30% or less", () => {
    // diff 30 / existing 100 = 30% — pins the strict `> 0.3` boundary
    expect(hasPriceConflict({ price: 130, currency: "USD" }, makeSub({ price: 100 }))).toBe(false)
  })

  test("returns false when suggestion has no price", () => {
    expect(hasPriceConflict({ price: null, currency: "USD" }, makeSub({ price: 150 }))).toBe(false)
  })

  test("returns false when suggestion has no currency", () => {
    expect(hasPriceConflict({ price: 100, currency: null }, makeSub({ price: 150 }))).toBe(false)
  })

  test("returns false for different currencies", () => {
    expect(hasPriceConflict({ price: 100, currency: "USD" }, makeSub({ price: 150, currency: "JPY" }))).toBe(false)
  })
})