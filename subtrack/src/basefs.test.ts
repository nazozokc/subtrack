import { test, expect, beforeAll, afterAll, beforeEach } from "vitest";
import Database from "better-sqlite3";

// Use an in-memory database for full test isolation
const testDb = new Database(":memory:");
testDb.pragma("journal_mode = WAL");
testDb.pragma("foreign_keys = ON");

// In-memory DB doesn't support CREATE TABLE with the same singleton pattern,
// so we create the tables here and inject the DB instance.
testDb.exec(`
  CREATE TABLE IF NOT EXISTS subscriptions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    price INTEGER NOT NULL,
    currency TEXT NOT NULL,
    cycle TEXT NOT NULL
  );
`);

testDb.exec(`
  CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
  );
`);

testDb.exec(`
  CREATE TABLE IF NOT EXISTS subscription_tags (
    subscription_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,
    PRIMARY KEY (subscription_id, tag_id),
    FOREIGN KEY (subscription_id) REFERENCES subscriptions(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id)
  );
`);

// Import after setting up in-memory schema, then inject it
const basefs = await import("./basefs");
basefs.__setDb(testDb);

beforeEach(() => {
  testDb.exec("DELETE FROM subscription_tags");
  testDb.exec("DELETE FROM tags");
  testDb.exec("DELETE FROM subscriptions");
});

afterAll(() => {
  testDb.close();
});

test("getSubscriptions returns empty when no data exists", () => {
  expect(basefs.getSubscriptions()).toEqual([]);
});

test("writeSubscription creates a subscription with tags", () => {
  basefs.writeSubscription({
    name: "Netflix",
    price: 1500,
    currency: "JPY",
    cycle: "monthly",
    tags: ["video", "entertainment"],
  });

  const subs = basefs.getSubscriptions();
  expect(subs).toHaveLength(1);
  expect(subs[0]).toMatchObject({
    name: "Netflix",
    price: 1500,
    currency: "JPY",
    cycle: "monthly",
    tags: ["video", "entertainment"],
  });
});

test("writeSubscription handles empty tags gracefully", () => {
  basefs.writeSubscription({
    name: "Dropbox",
    price: 10,
    currency: "USD",
    cycle: "monthly",
    tags: [],
  });

  const subs = basefs.getSubscriptions();
  expect(subs).toHaveLength(1);
  expect(subs[0].tags).toEqual([]);
});

test("writeSubscription supports USD currency", () => {
  basefs.writeSubscription({
    name: "GitHub Copilot",
    price: 10,
    currency: "USD",
    cycle: "monthly",
    tags: ["dev"],
  });

  const subs = basefs.getSubscriptions();
  expect(subs[0].currency).toBe("USD");
});

test("writeSubscription supports yearly cycle", () => {
  basefs.writeSubscription({
    name: "iCloud+",
    price: 12000,
    currency: "JPY",
    cycle: "yearly",
    tags: ["storage"],
  });

  const subs = basefs.getSubscriptions();
  expect(subs[0].cycle).toBe("yearly");
});

test("getSubscriptions returns all subscriptions ordered by id", () => {
  basefs.writeSubscription({
    name: "A",
    price: 100,
    currency: "USD",
    cycle: "monthly",
    tags: [],
  });
  basefs.writeSubscription({
    name: "B",
    price: 200,
    currency: "JPY",
    cycle: "yearly",
    tags: [],
  });
  basefs.writeSubscription({
    name: "C",
    price: 300,
    currency: "USD",
    cycle: "monthly",
    tags: [],
  });

  const subs = basefs.getSubscriptions();
  expect(subs).toHaveLength(3);
  expect(subs[0].name).toBe("A");
  expect(subs[1].name).toBe("B");
  expect(subs[2].name).toBe("C");
});

test("deleteSubscription removes a subscription", () => {
  basefs.writeSubscription({
    name: "ToDelete",
    price: 500,
    currency: "JPY",
    cycle: "monthly",
    tags: [],
  });

  const subsBefore = basefs.getSubscriptions();
  expect(subsBefore).toHaveLength(1);

  basefs.deleteSubscription(subsBefore[0].id);
  expect(basefs.getSubscriptions()).toHaveLength(0);
});

test("deleteSubscription cascades to subscription_tags", () => {
  basefs.writeSubscription({
    name: "WithTags",
    price: 999,
    currency: "USD",
    cycle: "monthly",
    tags: ["tag1", "tag2"],
  });

  const subs = basefs.getSubscriptions();
  expect(subs).toHaveLength(1);
  expect(subs[0].tags).toHaveLength(2);

  const subId = subs[0].id;
  const relCountBefore = (
    testDb.prepare(
      "SELECT COUNT(*) as cnt FROM subscription_tags WHERE subscription_id = ?",
    ).get(subId) as { cnt: number }
  ).cnt;
  expect(relCountBefore).toBe(2);

  basefs.deleteSubscription(subId);
  expect(basefs.getSubscriptions()).toHaveLength(0);

  const relCountAfter = (
    testDb.prepare(
      "SELECT COUNT(*) as cnt FROM subscription_tags WHERE subscription_id = ?",
    ).get(subId) as { cnt: number }
  ).cnt;
  expect(relCountAfter).toBe(0);
});

test("deleteSubscription does not throw when id does not exist", () => {
  expect(() => basefs.deleteSubscription(99999)).not.toThrow();
});

test("tagsSubscription filters by single tag", () => {
  basefs.writeSubscription({
    name: "Netflix",
    price: 1500,
    currency: "JPY",
    cycle: "monthly",
    tags: ["video", "entertainment"],
  });
  basefs.writeSubscription({
    name: "Spotify",
    price: 980,
    currency: "JPY",
    cycle: "monthly",
    tags: ["music"],
  });

  const results = basefs.tagsSubscription("video");
  expect(results).toHaveLength(1);
  expect(results[0].name).toBe("Netflix");
});

test("tagsSubscription filters by multiple tags with AND logic", () => {
  basefs.writeSubscription({
    name: "Netflix",
    price: 1500,
    currency: "JPY",
    cycle: "monthly",
    tags: ["video", "entertainment"],
  });
  basefs.writeSubscription({
    name: "YouTube Premium",
    price: 1280,
    currency: "JPY",
    cycle: "monthly",
    tags: ["video", "entertainment"],
  });
  basefs.writeSubscription({
    name: "Spotify",
    price: 980,
    currency: "JPY",
    cycle: "monthly",
    tags: ["music"],
  });

  // AND logic: must match ALL tags
  const results = basefs.tagsSubscription(["video", "entertainment"]);
  expect(results).toHaveLength(2);

  const names = results.map((r) => r.name).sort();
  expect(names).toEqual(["Netflix", "YouTube Premium"]);
});

test("tagsSubscription returns only subscriptions matching ALL specified tags", () => {
  basefs.writeSubscription({
    name: "Netflix",
    price: 1500,
    currency: "JPY",
    cycle: "monthly",
    tags: ["video", "entertainment"],
  });
  basefs.writeSubscription({
    name: "YouTube Premium",
    price: 1280,
    currency: "JPY",
    cycle: "monthly",
    tags: ["video"],
  });

  // Only Netflix has BOTH tags
  const results = basefs.tagsSubscription(["video", "entertainment"]);
  expect(results).toHaveLength(1);
  expect(results[0].name).toBe("Netflix");
});

test("tagsSubscription returns empty for non-matching tag", () => {
  basefs.writeSubscription({
    name: "Netflix",
    price: 1500,
    currency: "JPY",
    cycle: "monthly",
    tags: ["video"],
  });

  expect(basefs.tagsSubscription("nonexistent")).toEqual([]);
});

test("tagsSubscription returns empty array for empty input", () => {
  expect(basefs.tagsSubscription([])).toEqual([]);
  expect(basefs.tagsSubscription("")).toEqual([]);
});

test("works with multiple subscriptions sharing the same tag", () => {
  basefs.writeSubscription({
    name: "S1",
    price: 100,
    currency: "USD",
    cycle: "monthly",
    tags: ["shared"],
  });
  basefs.writeSubscription({
    name: "S2",
    price: 200,
    currency: "JPY",
    cycle: "yearly",
    tags: ["shared"],
  });

  const results = basefs.tagsSubscription("shared");
  expect(results).toHaveLength(2);
});

test("getSubscriptions returns correct data types", () => {
  basefs.writeSubscription({
    name: "Test",
    price: 1000,
    currency: "JPY",
    cycle: "monthly",
    tags: ["test"],
  });

  const [sub] = basefs.getSubscriptions();
  expect(typeof sub.id).toBe("number");
  expect(typeof sub.name).toBe("string");
  expect(typeof sub.price).toBe("number");
  expect(["JPY", "USD"]).toContain(sub.currency);
  expect(["monthly", "yearly"]).toContain(sub.cycle);
  expect(Array.isArray(sub.tags)).toBe(true);
});

test("does not share tags between different subscriptions", () => {
  basefs.writeSubscription({
    name: "Netflix",
    price: 1500,
    currency: "JPY",
    cycle: "monthly",
    tags: ["video"],
  });
  basefs.writeSubscription({
    name: "Dropbox",
    price: 10,
    currency: "USD",
    cycle: "monthly",
    tags: ["storage"],
  });

  const subs = basefs.getSubscriptions();
  const netflix = subs.find((s) => s.name === "Netflix");
  const dropbox = subs.find((s) => s.name === "Dropbox");
  expect(netflix?.tags).toEqual(["video"]);
  expect(dropbox?.tags).toEqual(["storage"]);
});
