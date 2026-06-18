import { test, expect, beforeAll, afterAll, beforeEach } from "vitest"
import initSqlJs from "sql.js"
import type { Database } from "sql.js"

let testDb: Database

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

  const db = await import("./db.ts")
  db.__setDb(testDb)
})

beforeEach(() => {
  testDb.run("DELETE FROM subscription_tags")
  testDb.run("DELETE FROM tags")
  testDb.run("DELETE FROM subscriptions")
})

afterAll(() => {
  testDb.close()
})

test("getSubscriptions returns empty when no data exists", async () => {
  const db = await import("./db.ts")
  expect(db.getSubscriptions()).toEqual([])
})

test("writeSubscription creates a subscription with tags", async () => {
  const db = await import("./db.ts")

  db.writeSubscription({
    name: "Netflix",
    price: 1500,
    currency: "JPY",
    cycle: "monthly",
    tags: ["video", "entertainment"],
  })

  const subs = db.getSubscriptions()
  expect(subs).toHaveLength(1)
  expect(subs[0]).toMatchObject({
    name: "Netflix",
    price: 1500,
    currency: "JPY",
    cycle: "monthly",
    tags: ["video", "entertainment"],
  })
})

test("writeSubscription handles empty tags gracefully", async () => {
  const db = await import("./db.ts")

  db.writeSubscription({
    name: "Dropbox",
    price: 10,
    currency: "USD",
    cycle: "monthly",
    tags: [],
  })

  const subs = db.getSubscriptions()
  expect(subs).toHaveLength(1)
  expect(subs[0].tags).toEqual([])
})

test("writeSubscription supports USD currency", async () => {
  const db = await import("./db.ts")

  db.writeSubscription({
    name: "GitHub Copilot",
    price: 10,
    currency: "USD",
    cycle: "monthly",
    tags: ["dev"],
  })

  const subs = db.getSubscriptions()
  expect(subs[0].currency).toBe("USD")
})

test("writeSubscription supports yearly cycle", async () => {
  const db = await import("./db.ts")

  db.writeSubscription({
    name: "iCloud+",
    price: 12000,
    currency: "JPY",
    cycle: "yearly",
    tags: ["storage"],
  })

  const subs = db.getSubscriptions()
  expect(subs[0].cycle).toBe("yearly")
})

test("getSubscriptions returns all subscriptions ordered by id", async () => {
  const db = await import("./db.ts")

  db.writeSubscription({
    name: "A",
    price: 100,
    currency: "USD",
    cycle: "monthly",
    tags: [],
  })
  db.writeSubscription({
    name: "B",
    price: 200,
    currency: "JPY",
    cycle: "yearly",
    tags: [],
  })
  db.writeSubscription({
    name: "C",
    price: 300,
    currency: "USD",
    cycle: "monthly",
    tags: [],
  })

  const subs = db.getSubscriptions()
  expect(subs).toHaveLength(3)
  expect(subs[0].name).toBe("A")
  expect(subs[1].name).toBe("B")
  expect(subs[2].name).toBe("C")
})

test("deleteSubscription removes a subscription", async () => {
  const db = await import("./db.ts")

  db.writeSubscription({
    name: "ToDelete",
    price: 500,
    currency: "JPY",
    cycle: "monthly",
    tags: [],
  })

  const subsBefore = db.getSubscriptions()
  expect(subsBefore).toHaveLength(1)

  db.deleteSubscription(subsBefore[0].id)
  expect(db.getSubscriptions()).toHaveLength(0)
})

test("deleteSubscription cascades to subscription_tags", async () => {
  const db = await import("./db.ts")

  db.writeSubscription({
    name: "WithTags",
    price: 999,
    currency: "USD",
    cycle: "monthly",
    tags: ["tag1", "tag2"],
  })

  const subs = db.getSubscriptions()
  expect(subs).toHaveLength(1)
  expect(subs[0].tags).toHaveLength(2)

  const subId = subs[0].id
  const rows = testDb.exec(
    "SELECT COUNT(*) as cnt FROM subscription_tags WHERE subscription_id = ?",
    [subId],
  )
  const relCountBefore = Number(rows[0].values[0][0])
  expect(relCountBefore).toBe(2)

  db.deleteSubscription(subId)
  expect(db.getSubscriptions()).toHaveLength(0)

  const rowsAfter = testDb.exec(
    "SELECT COUNT(*) as cnt FROM subscription_tags WHERE subscription_id = ?",
    [subId],
  )
  const relCountAfter = Number(rowsAfter[0].values[0][0])
  expect(relCountAfter).toBe(0)
})

test("deleteSubscription does not throw when id does not exist", async () => {
  const db = await import("./db.ts")
  expect(() => db.deleteSubscription(99999)).not.toThrow()
})

test("tagsSubscription filters by single tag", async () => {
  const db = await import("./db.ts")

  db.writeSubscription({
    name: "Netflix",
    price: 1500,
    currency: "JPY",
    cycle: "monthly",
    tags: ["video", "entertainment"],
  })
  db.writeSubscription({
    name: "Spotify",
    price: 980,
    currency: "JPY",
    cycle: "monthly",
    tags: ["music"],
  })

  const results = db.tagsSubscription("video")
  expect(results).toHaveLength(1)
  expect(results[0].name).toBe("Netflix")
})

test("tagsSubscription filters by multiple tags with AND logic", async () => {
  const db = await import("./db.ts")

  db.writeSubscription({
    name: "Netflix",
    price: 1500,
    currency: "JPY",
    cycle: "monthly",
    tags: ["video", "entertainment"],
  })
  db.writeSubscription({
    name: "YouTube Premium",
    price: 1280,
    currency: "JPY",
    cycle: "monthly",
    tags: ["video", "entertainment"],
  })
  db.writeSubscription({
    name: "Spotify",
    price: 980,
    currency: "JPY",
    cycle: "monthly",
    tags: ["music"],
  })

  const results = db.tagsSubscription(["video", "entertainment"])
  expect(results).toHaveLength(2)

  const names = results.map((r) => r.name).sort()
  expect(names).toEqual(["Netflix", "YouTube Premium"])
})

test("tagsSubscription returns only subscriptions matching ALL specified tags", async () => {
  const db = await import("./db.ts")

  db.writeSubscription({
    name: "Netflix",
    price: 1500,
    currency: "JPY",
    cycle: "monthly",
    tags: ["video", "entertainment"],
  })
  db.writeSubscription({
    name: "YouTube Premium",
    price: 1280,
    currency: "JPY",
    cycle: "monthly",
    tags: ["video"],
  })

  const results = db.tagsSubscription(["video", "entertainment"])
  expect(results).toHaveLength(1)
  expect(results[0].name).toBe("Netflix")
})

test("tagsSubscription returns empty for non-matching tag", async () => {
  const db = await import("./db.ts")

  db.writeSubscription({
    name: "Netflix",
    price: 1500,
    currency: "JPY",
    cycle: "monthly",
    tags: ["video"],
  })

  expect(db.tagsSubscription("nonexistent")).toEqual([])
})

test("tagsSubscription returns empty array for empty input", async () => {
  const db = await import("./db.ts")
  expect(db.tagsSubscription([])).toEqual([])
  expect(db.tagsSubscription("")).toEqual([])
})

test("works with multiple subscriptions sharing the same tag", async () => {
  const db = await import("./db.ts")

  db.writeSubscription({
    name: "S1",
    price: 100,
    currency: "USD",
    cycle: "monthly",
    tags: ["shared"],
  })
  db.writeSubscription({
    name: "S2",
    price: 200,
    currency: "JPY",
    cycle: "yearly",
    tags: ["shared"],
  })

  const results = db.tagsSubscription("shared")
  expect(results).toHaveLength(2)
})

test("periodFactor returns correct factor for monthly to monthly", async () => {
  const { periodFactor } = await import("./db.ts")
  expect(periodFactor("monthly", "monthly")).toBe(1)
})

test("periodFactor returns correct factor for yearly to monthly", async () => {
  const { periodFactor } = await import("./db.ts")
  expect(periodFactor("yearly", "monthly")).toBe(1 / 12)
})

test("periodFactor returns correct factor for monthly to yearly", async () => {
  const { periodFactor } = await import("./db.ts")
  expect(periodFactor("monthly", "yearly")).toBe(12)
})

test("periodFactor returns correct factor for weekly to monthly", async () => {
  const { periodFactor } = await import("./db.ts")
  expect(periodFactor("weekly", "monthly")).toBe(52 / 12)
})

test("periodFactor returns correct factor for bi-weekly to monthly", async () => {
  const { periodFactor } = await import("./db.ts")
  expect(periodFactor("bi-weekly", "monthly")).toBe(26 / 12)
})

test("periodFactor returns correct factor for quarterly to monthly", async () => {
  const { periodFactor } = await import("./db.ts")
  expect(periodFactor("quarterly", "monthly")).toBe(4 / 12)
})

test("periodFactor returns correct factor for semi-annual to monthly", async () => {
  const { periodFactor } = await import("./db.ts")
  expect(periodFactor("semi-annual", "monthly")).toBe(2 / 12)
})

test("periodFactor defaults to monthly when to is omitted", async () => {
  const { periodFactor } = await import("./db.ts")
  expect(periodFactor("yearly")).toBe(1 / 12)
  expect(periodFactor("monthly")).toBe(1)
})

test("periodFactor returns correct factor for quarterly to yearly", async () => {
  const { periodFactor } = await import("./db.ts")
  expect(periodFactor("quarterly", "yearly")).toBe(4)
})

test("periodFactor handles all cycle-to-cycle combinations without throwing", async () => {
  const { periodFactor, OCCURRENCES_PER_YEAR } = await import("./db.ts")
  const cycles = Object.keys(OCCURRENCES_PER_YEAR) as Array<keyof typeof OCCURRENCES_PER_YEAR>
  for (const from of cycles) {
    for (const to of cycles) {
      const factor = periodFactor(from, to)
      expect(typeof factor).toBe("number")
      expect(factor).not.toBeNaN()
      expect(factor).toBeGreaterThan(0)
    }
  }
})

test("getSubscriptions returns correct data types", async () => {
  const db = await import("./db.ts")

  db.writeSubscription({
    name: "Test",
    price: 1000,
    currency: "JPY",
    cycle: "monthly",
    tags: ["test"],
  })

  const [sub] = db.getSubscriptions()
  expect(typeof sub.id).toBe("number")
  expect(typeof sub.name).toBe("string")
  expect(typeof sub.price).toBe("number")
  expect(["JPY", "USD"]).toContain(sub.currency)
  expect(["monthly", "yearly"]).toContain(sub.cycle)
  expect(Array.isArray(sub.tags)).toBe(true)
})

test("does not share tags between different subscriptions", async () => {
  const db = await import("./db.ts")

  db.writeSubscription({
    name: "Netflix",
    price: 1500,
    currency: "JPY",
    cycle: "monthly",
    tags: ["video"],
  })
  db.writeSubscription({
    name: "Dropbox",
    price: 10,
    currency: "USD",
    cycle: "monthly",
    tags: ["storage"],
  })

  const subs = db.getSubscriptions()
  const netflix = subs.find((s) => s.name === "Netflix")
  const dropbox = subs.find((s) => s.name === "Dropbox")
  expect(netflix?.tags).toEqual(["video"])
  expect(dropbox?.tags).toEqual(["storage"])
})

// ── sort ──────────────────────────────────────────────────

test("getSubscriptions sorts by name ascending", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "C", price: 100, currency: "USD", cycle: "monthly", tags: [] })
  db.writeSubscription({ name: "A", price: 200, currency: "USD", cycle: "monthly", tags: [] })
  db.writeSubscription({ name: "B", price: 300, currency: "USD", cycle: "monthly", tags: [] })

  const subs = db.getSubscriptions("name", false)
  expect(subs[0].name).toBe("A")
  expect(subs[1].name).toBe("B")
  expect(subs[2].name).toBe("C")
})

test("getSubscriptions sorts by name descending", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "A", price: 100, currency: "USD", cycle: "monthly", tags: [] })
  db.writeSubscription({ name: "B", price: 200, currency: "USD", cycle: "monthly", tags: [] })
  db.writeSubscription({ name: "C", price: 300, currency: "USD", cycle: "monthly", tags: [] })

  const subs = db.getSubscriptions("name", true)
  expect(subs[0].name).toBe("C")
  expect(subs[1].name).toBe("B")
  expect(subs[2].name).toBe("A")
})

test("getSubscriptions sorts by price ascending", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "A", price: 300, currency: "USD", cycle: "monthly", tags: [] })
  db.writeSubscription({ name: "B", price: 100, currency: "USD", cycle: "monthly", tags: [] })
  db.writeSubscription({ name: "C", price: 200, currency: "USD", cycle: "monthly", tags: [] })

  const subs = db.getSubscriptions("price", false)
  expect(subs[0].price).toBe(100)
  expect(subs[1].price).toBe(200)
  expect(subs[2].price).toBe(300)
})

test("getSubscriptions falls back to id order for invalid sort field", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "B", price: 100, currency: "USD", cycle: "monthly", tags: [] })
  db.writeSubscription({ name: "A", price: 200, currency: "USD", cycle: "monthly", tags: [] })

  const subs = db.getSubscriptions("invalid_field", false)
  expect(subs[0].name).toBe("B")
  expect(subs[1].name).toBe("A")
})

// ── getSubscription ───────────────────────────────────────

test("getSubscription returns a single subscription by id", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "Target", price: 500, currency: "JPY", cycle: "monthly", tags: ["test"] })

  const [all] = db.getSubscriptions()
  const found = db.getSubscription(all.id)
  expect(found).toBeDefined()
  expect(found?.name).toBe("Target")
  expect(found?.tags).toEqual(["test"])
})

test("getSubscription returns undefined for non-existent id", async () => {
  const db = await import("./db.ts")
  expect(db.getSubscription(99999)).toBeUndefined()
})

// ── updateSubscription ────────────────────────────────────

test("updateSubscription updates a single field", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "Old Name", price: 1000, currency: "JPY", cycle: "monthly", tags: [] })

  const [sub] = db.getSubscriptions()
  db.updateSubscription(sub.id, { name: "New Name" })

  const updated = db.getSubscription(sub.id)
  expect(updated?.name).toBe("New Name")
  expect(updated?.price).toBe(1000)
})

test("updateSubscription updates all fields", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "Old", price: 1000, currency: "JPY", cycle: "monthly", tags: ["old"] })

  const [sub] = db.getSubscriptions()
  db.updateSubscription(sub.id, {
    name: "New",
    price: 2000,
    currency: "USD",
    cycle: "yearly",
    tags: ["new"],
  })

  const updated = db.getSubscription(sub.id)
  expect(updated?.name).toBe("New")
  expect(updated?.price).toBe(2000)
  expect(updated?.currency).toBe("USD")
  expect(updated?.cycle).toBe("yearly")
  expect(updated?.tags).toEqual(["new"])
})

test("updateSubscription replaces tags when specified", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "Test", price: 500, currency: "JPY", cycle: "monthly", tags: ["old1", "old2"] })

  const [sub] = db.getSubscriptions()
  db.updateSubscription(sub.id, { tags: ["new1"] })

  const updated = db.getSubscription(sub.id)
  expect(updated?.tags).toEqual(["new1"])
})

test("updateSubscription does not clear tags when not specified", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "Test", price: 500, currency: "JPY", cycle: "monthly", tags: ["keep"] })

  const [sub] = db.getSubscriptions()
  db.updateSubscription(sub.id, { name: "Renamed" })

  const updated = db.getSubscription(sub.id)
  expect(updated?.tags).toEqual(["keep"])
})

// ── getTagsWithCount ──────────────────────────────────────

test("getTagsWithCount returns tag usage counts", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "S1", price: 100, currency: "USD", cycle: "monthly", tags: ["shared"] })
  db.writeSubscription({ name: "S2", price: 200, currency: "JPY", cycle: "monthly", tags: ["shared", "unique"] })

  const tags = db.getTagsWithCount()
  const shared = tags.find((t) => t.name === "shared")
  const unique = tags.find((t) => t.name === "unique")
  expect(shared?.count).toBe(2)
  expect(unique?.count).toBe(1)
})

test("getTagsWithCount returns empty array when no tags exist", async () => {
  const db = await import("./db.ts")
  expect(db.getTagsWithCount()).toEqual([])
})

// ── renameTag ─────────────────────────────────────────────

test("renameTag renames a tag", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "S1", price: 100, currency: "USD", cycle: "monthly", tags: ["old"] })

  const result = db.renameTag("old", "new")
  expect(result).toBe(true)

  const tags = db.getTagsWithCount()
  expect(tags.find((t) => t.name === "old")).toBeUndefined()
  expect(tags.find((t) => t.name === "new")?.count).toBe(1)
})

test("renameTag merges when target name already exists", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "S1", price: 100, currency: "USD", cycle: "monthly", tags: ["a"] })
  db.writeSubscription({ name: "S2", price: 200, currency: "JPY", cycle: "monthly", tags: ["b"] })

  db.renameTag("a", "b")
  const tags = db.getTagsWithCount()
  const merged = tags.find((t) => t.name === "b")
  expect(merged?.count).toBe(2)
  expect(tags.find((t) => t.name === "a")).toBeUndefined()
})

test("renameTag returns false for non-existent tag", async () => {
  const db = await import("./db.ts")
  expect(db.renameTag("nonexistent", "new")).toBe(false)
})

// ── deleteTag ─────────────────────────────────────────────

test("deleteTag removes a tag", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "S1", price: 100, currency: "USD", cycle: "monthly", tags: ["remove"] })

  const result = db.deleteTag("remove")
  expect(result).toBe(true)
  expect(db.getTagsWithCount()).toHaveLength(0)
})

test("deleteTag returns false for non-existent tag", async () => {
  const db = await import("./db.ts")
  expect(db.deleteTag("nonexistent")).toBe(false)
})

// ── pruneTags ─────────────────────────────────────────────

test("pruneTags removes orphaned tags", async () => {
  const db = await import("./db.ts")
  // Create tags via subscription
  db.writeSubscription({ name: "S1", price: 100, currency: "USD", cycle: "monthly", tags: ["keep"] })
  // Orphan tag by deleting subscription (CASCADE removes subscription_tags)
  const [sub] = db.getSubscriptions()
  db.deleteSubscription(sub.id)

  // Re-create the orphan tags directly
  testDb.run("INSERT INTO tags (name) VALUES ('orphan1'), ('orphan2')")

  // keep + orphan1 + orphan2 = 3 orphaned tags
  const count = db.pruneTags()
  expect(count).toBe(3)

  const remaining = db.getTagsWithCount()
  expect(remaining).toHaveLength(0)
})

test("pruneTags does not remove tags still in use", async () => {
  const db = await import("./db.ts")
  db.writeSubscription({ name: "S1", price: 100, currency: "USD", cycle: "monthly", tags: ["active"] })

  const count = db.pruneTags()
  expect(count).toBe(0)

  const tags = db.getTagsWithCount()
  expect(tags).toHaveLength(1)
  expect(tags[0].name).toBe("active")
})
