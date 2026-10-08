#!/usr/bin/env node
// Builds legacy-schema fixtures for golden capture:
//
//   dbs/legacy/         pre-v1 schema (LEGACY_COLUMNS only, user_version=0),
//                       encrypted with the same AES-256-GCM format as lib/src/crypto.ts
//   dbs/legacy-plain/   same schema, plain (unencrypted) SQLite file
//   inputs/legacy-backup.db  copy of the plain DB (restore golden — no .sha256
//                       sidecar, like real pre-hash backups)
//
// Run with tsx (imports TypeScript directly):
//   pnpm exec tsx rust/fixtures/capture/make-legacy.mjs

import { DatabaseSync } from "node:sqlite"
import { createHash } from "node:crypto"
import { mkdirSync, writeFileSync, copyFileSync, rmSync } from "node:fs"
import path from "node:path"
import { fileURLToPath } from "node:url"
import { encryptBuffer } from "../../../lib/src/crypto.ts"

const here = path.dirname(fileURLToPath(import.meta.url))
const fixturesDir = path.resolve(here, "..")

const LEGACY_DDL = `
CREATE TABLE subscriptions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  price REAL NOT NULL,
  currency TEXT NOT NULL,
  cycle TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'active',
  billing_day INTEGER,
  created_at TEXT NOT NULL DEFAULT (date('now'))
);
CREATE TABLE tags (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL UNIQUE
);
CREATE TABLE subscription_tags (
  subscription_id INTEGER NOT NULL,
  tag_id INTEGER NOT NULL,
  PRIMARY KEY (subscription_id, tag_id),
  FOREIGN KEY (subscription_id) REFERENCES subscriptions(id) ON DELETE CASCADE,
  FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
);
`

// Build the plain legacy database in a temp location, then encrypt a copy.
const tmpPlain = path.join(fixturesDir, "capture", ".legacy-tmp.db")
const db = new DatabaseSync(tmpPlain)
db.exec("PRAGMA foreign_keys = ON")
db.exec(LEGACY_DDL)

const subs = [
  // id, name, price, currency, cycle, status, billing_day, created_at
  ["Netflix", 15.99, "USD", "monthly", "active", 15, "2025-11-20"],
  ["Spotify", 5.99, "USD", "monthly", "active", 1, "2025-12-01"],
  ["Local Gym", 12000, "JPY", "monthly", "paused", 5, "2026-01-10"],
  ["Old Magazine", 9.99, "EUR", "quarterly", "cancelled", 3, "2026-02-14"],
]
const insertSub = db.prepare(
  "INSERT INTO subscriptions (name, price, currency, cycle, status, billing_day, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
)
for (const s of subs) insertSub.run(...s)

const insertTag = db.prepare("INSERT INTO tags (name) VALUES (?)")
const tagIds = new Map()
for (const name of ["video", "music", "health", "reading"]) {
  const r = insertTag.run(name)
  tagIds.set(name, Number(r.lastInsertRowid))
}
const linkTag = db.prepare(
  "INSERT INTO subscription_tags (subscription_id, tag_id) VALUES (?, ?)",
)
linkTag.run(1, tagIds.get("video")) // Netflix → video
linkTag.run(2, tagIds.get("music")) // Spotify → music
linkTag.run(3, tagIds.get("health")) // Gym → health
linkTag.run(4, tagIds.get("reading")) // Magazine → reading
db.close()

// 1) Plain legacy fixture (+ copy for restore input).
const plainDir = path.join(fixturesDir, "dbs", "legacy-plain")
mkdirSync(plainDir, { recursive: true, mode: 0o700 })
copyFileSync(tmpPlain, path.join(plainDir, "subtrack.db"))
copyFileSync(tmpPlain, path.join(fixturesDir, "inputs", "legacy-backup.db"))
// A backup of a database that predates hash sidecars ships without one on
// purpose — restore must warn and continue.

// 2) Encrypted legacy fixture (auto-creates .key + .key.sha256 via crypto.ts).
const legacyDir = path.join(fixturesDir, "dbs", "legacy")
mkdirSync(legacyDir, { recursive: true, mode: 0o700 })
process.env.SUBSC_CLI_DB_DIR = legacyDir
delete process.env.SUBSC_CLI_DB_PASSPHRASE
const plain = await import("node:fs").then((fs) => fs.readFileSync(tmpPlain))
const encrypted = encryptBuffer(plain)
writeFileSync(path.join(legacyDir, "subtrack.db"), encrypted, { mode: 0o600 })
// No subtrack.db.sha256 / .subtrack.integrity — mimics a database copied from
// an older install, exercising first-open hash/integrity bootstrap.

// Sanity: encrypted fixture must not start with the SQLite magic header.
const magic = Buffer.from("SQLite format 3\u0000", "binary")
if (encrypted.subarray(0, 16).equals(magic)) {
  throw new Error("legacy fixture was not encrypted")
}

const plainHash = createHash("sha256").update(plain).digest("hex")
rmSync(tmpPlain, { force: true }) // keep the repo clean — the temp copy is done
console.log(
  `legacy fixtures written:\n` +
    `  dbs/legacy/          (encrypted, ${encrypted.length} bytes)\n` +
    `  dbs/legacy-plain/    (plain, ${plain.length} bytes)\n` +
    `  inputs/legacy-backup.db (sha256 ${plainHash.slice(0, 16)}…)\n` +
    `  subscriptions: ${subs.length}, tags: ${tagIds.size}`,
)
