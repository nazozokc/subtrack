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
    FOREIGN KEY (tag_id) REFERENCES tags(id)
  )`)

  const db = await import("./db")
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
  const db = await import("./db")
  expect(db.getSubscriptions()).toEqual([])
})

test("writeSubscription creates a subscription with tags", async () => {
  const db = await import("./db")

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
  const db = await import("./db")

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
  const db = await import("./db")

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
  const db = await import("./db")

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
  const db = await import("./db")

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
  const db = await import("./db")

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
  const db = await import("./db")

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
  const db = await import("./db")
  expect(() => db.deleteSubscription(99999)).not.toThrow()
})

test("tagsSubscription filters by single tag", async () => {
  const db = await import("./db")

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
  const db = await import("./db")

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
  const db = await import("./db")

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
  const db = await import("./db")

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
  const db = await import("./db")
  expect(db.tagsSubscription([])).toEqual([])
  expect(db.tagsSubscription("")).toEqual([])
})

test("works with multiple subscriptions sharing the same tag", async () => {
  const db = await import("./db")

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

test("getSubscriptions returns correct data types", async () => {
  const db = await import("./db")

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
  const db = await import("./db")

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
