import { test, expect, beforeAll, afterAll, beforeEach, vi } from "vitest"
import { DatabaseSync } from "node:sqlite"
import { mkdtempSync, rmSync, readFileSync, existsSync } from "node:fs"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { isEncrypted } from "@subtrack/lib/crypto"

vi.mock("@subtrack/lib/logger", () => {
  const noop = () => {}
  return {
    default: { log: noop, info: noop, success: noop, error: noop, fail: noop, warn: noop },
    consola: { log: noop, info: noop, success: noop, error: noop, fail: noop, warn: noop },
  }
})

// Counting flushes has to happen at the filesystem, not by spying on a module:
// ESM namespace objects are frozen, and `db.ts` re-exports `saveDb`, so a spy
// on `db/connection.ts` would never be observed by the repository adapter.
// Every flush writes a `.subtrack.db.<uuid>.tmp` before renaming it into place,
// which gives an unambiguous, implementation-independent counter.
const flushes = { count: 0 }
vi.mock("node:fs", async (importOriginal) => {
  const actual = await importOriginal<typeof import("node:fs")>()
  return {
    ...actual,
    default: actual,
    writeFileSync: (file: never, ...rest: never[]) => {
      if (typeof file === "string" && file.includes(".subtrack.db.") && file.endsWith(".tmp")) {
        flushes.count++
      }
      return (actual.writeFileSync as (...a: never[]) => void)(file, ...rest)
    },
  }
})

// The flush is a whole-file rewrite of an encrypted database, so proving that a
// bulk edit writes once — not once per row — needs a real file on disk. An
// in-memory database would accept every code path and prove nothing.
vi.setConfig({ testTimeout: 60_000 })

const conn = await import("../db/connection.ts")
const repos = await import("../application/repositories.ts")

let dir: string

beforeAll(() => {
  dir = mkdtempSync(join(tmpdir(), "subtrack-batch-"))
  process.env.SUBSC_CLI_DB_DIR = dir
  conn.getDb()
})

afterAll(() => {
  conn.closeDb()
  rmSync(dir, { recursive: true, force: true })
})

beforeEach(() => {
  flushes.count = 0
  const db = conn.getDb()
  db.exec("DELETE FROM subscriptions")
  db.exec("DELETE FROM tags")
  db.exec("DELETE FROM subscription_tags")
  conn.saveDb()
  flushes.count = 0
})

function seed(name: string, price = 1000): number {
  return repos.subscriptionRepository.add({
    name,
    price,
    currency: "JPY",
    cycle: "monthly",
    status: "active",
  } as never)
}

test("withBatch flushes the encrypted database exactly once for many writes", () => {
  seed("A")
  seed("B")
  seed("C")
  flushes.count = 0

  repos.withBatch(() => {
    for (const sub of repos.subscriptionRepository.list()) {
      repos.subscriptionRepository.update(sub.id, { status: "paused" })
    }
  })

  expect(flushes.count).toBe(1)
})

test("withBatch leaves the database untouched when nothing is written", () => {
  seed("Solo")
  flushes.count = 0

  repos.withBatch(() => {
    // Filter matches nothing — no writes, so no rewrite of the whole file.
  })

  expect(flushes.count).toBe(0)
})

test("writes issued outside a batch still flush immediately", () => {
  const id = seed("D")
  flushes.count = 0

  expect(repos.subscriptionRepository.update(id, { status: "cancelled" })).toBe(true)

  expect(flushes.count).toBe(1)
})

test("a failed write inside a batch still flushes the rows that succeeded", () => {
  const id = seed("E")
  seed("F")
  flushes.count = 0

  repos.withBatch(() => {
    expect(repos.subscriptionRepository.update(id, { status: "paused" })).toBe(true)
    // A missing id reports false rather than throwing.
    expect(repos.subscriptionRepository.update(999_999, { status: "paused" })).toBe(false)
  })

  expect(flushes.count).toBe(1)
  // The successful write is durable, not lost with the failed one.
  expect(repos.subscriptionRepository.get(id)?.status).toBe("paused")
})

test("nested batches flush once, at the outermost close", () => {
  const first = seed("G")
  const second = seed("H")
  flushes.count = 0

  repos.withBatch(() => {
    repos.subscriptionRepository.update(first, { status: "paused" })
    repos.withBatch(() => {
      repos.subscriptionRepository.update(second, { status: "cancelled" })
    })
    // Still inside the outer batch: nothing has been flushed yet.
    expect(flushes.count).toBe(0)
  })

  expect(flushes.count).toBe(1)
})

test("a throwing batch still flushes completed writes", () => {
  const id = seed("I")
  flushes.count = 0

  expect(() =>
    repos.withBatch(() => {
      repos.subscriptionRepository.update(id, { status: "paused" })
      throw new Error("boom")
    }),
  ).toThrow("boom")

  expect(flushes.count).toBe(1)
  expect(repos.subscriptionRepository.get(id)?.status).toBe("paused")
})

test("batched updates survive a close and reopen of the database", () => {
  const id = seed("Persisted")
  repos.withBatch(() => {
    repos.subscriptionRepository.update(id, { status: "cancelled" })
  })

  // Prove the encrypted file on disk actually carries the change.
  const dbPath = conn.getDbPath()
  expect(existsSync(dbPath)).toBe(true)
  expect(isEncrypted(readFileSync(dbPath))).toBe(true)

  conn.closeDb()
  expect(repos.subscriptionRepository.get(id)?.status).toBe("cancelled")
})

test("batch depth does not leak across cases", () => {
  // If a previous test threw inside withBatch without unwinding, the depth
  // counter would stay above zero and silently suppress every later flush.
  const id = seed("J")
  flushes.count = 0

  repos.subscriptionRepository.update(id, { status: "paused" })

  expect(flushes.count).toBe(1)
  expect(conn.getDb()).toBeInstanceOf(DatabaseSync)
})

// Every write adapter must honour the batch. A method that takes part in the
// bookkeeping but never passes `persist: false` would still rewrite the whole
// file per row, so the batch would look like it works while doing N flushes.
// These cases pin the contract for the repositories beyond subscriptions, and
// fail loudly if a new adapter is added without joining the protocol.
test("every write repository participates in the batch", () => {
  const id = seed("Batch")
  flushes.count = 0

  repos.withBatch(() => {
    repos.subscriptionRepository.archive(id)
    repos.subscriptionRepository.unarchive(id)
    repos.trialRepository.add({ name: "Trial", expiresAt: "2027-01-01" } as never)
    repos.trialRepository.remove(1)
    repos.usageRepository.add({
      provider: "openai",
      model: "gpt-5",
      input_tokens: 1,
      output_tokens: 1,
      cost: 1,
      date: "2026-01-01",
      description: null,
    })
    repos.priceHistoryRepository.record(id, 1000, 1200, "JPY", "JPY")
    repos.auditRepository.record({ action: "subscription.edit" })
  })

  expect(flushes.count).toBe(1)
})

test("writes outside a batch flush exactly once each", () => {
  const id = seed("Solo")
  flushes.count = 0

  repos.subscriptionRepository.archive(id)
  repos.trialRepository.add({ name: "Solo", expiresAt: "2027-01-01" } as never)
  repos.auditRepository.record({ action: "subscription.edit" })

  expect(flushes.count).toBe(3)
})

test("a no-op write inside a batch does not force a rewrite", () => {
  seed("Noop")
  flushes.count = 0

  repos.withBatch(() => {
    // Both target a row that is not there, so neither changes anything and
    // the file should not be rewritten.
    expect(repos.subscriptionRepository.update(999_999, { status: "paused" })).toBe(false)
    expect(repos.subscriptionRepository.remove(999_999)).toBe(false)
    expect(repos.subscriptionRepository.archive(999_999)).toBe(false)
  })

  expect(flushes.count).toBe(0)
})

// `write()` decides "does this batch owe a flush" from the db function's
// return value: `false` and a zero count mean nothing changed, `void` means the
// write happened. A db function that starts returning one of those shapes
// without its adapter saying so would rewrite the file for a no-op, so the
// three shapes are pinned here.
test("a skipped write inside a batch does not force a rewrite", () => {
  seed("Dedup")
  flushes.count = 0

  repos.withBatch(() => {
    // addFromLog deduplicates on generation_id, so the second one writes nothing.
    const entry = {
      provider: "openai",
      model: "gpt-5",
      input_tokens: 1,
      output_tokens: 1,
      cost: 1,
      date: "2026-01-01",
      description: null,
      generation_id: "gen-dedup",
    }
    expect(repos.usageRepository.addFromLog(entry)).toBe(true)
    expect(repos.usageRepository.addFromLog(entry)).toBe(false)
  })

  expect(flushes.count).toBe(1)
})

test("an empty batch does not rewrite the file", () => {
  seed("Untouched")
  flushes.count = 0

  repos.withBatch(() => {
    // Reads only: nothing here owes a flush.
    repos.subscriptionRepository.list()
    repos.subscriptionRepository.get(1)
    repos.tagRepository.list()
    repos.trialRepository.list()
  })

  expect(flushes.count).toBe(0)
})
