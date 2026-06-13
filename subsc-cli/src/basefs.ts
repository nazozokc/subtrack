import Database from "better-sqlite3";
import { mkdirSync, existsSync } from "node:fs";
import path from "node:path";
import { homedir } from "node:os";
import { consola } from "consola";

export type SharedArgs = {
  id: number;
  name: string;
  price: number;
  currency: "JPY" | "USD";
  cycle: "monthly" | "yearly";
  tags: string[];
};

export type AddSharedArgs = Omit<SharedArgs, "id">;

let _db: Database | null = null;

function getDbDir(): string {
  return process.env.SUBSC_CLI_DB_DIR ?? path.join(homedir(), ".config", "subsc-cli");
}

function getDb(): Database {
  if (_db) return _db;

  const dbdir = getDbDir();
  mkdirSync(dbdir, { recursive: true });

  _db = new Database(path.join(dbdir, "subscriptions.db"));
  _db.pragma("journal_mode = WAL");
  _db.pragma("foreign_keys = ON");

  _db.exec(`
    CREATE TABLE IF NOT EXISTS subscriptions (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      name TEXT NOT NULL,
      price INTEGER NOT NULL,
      currency TEXT NOT NULL,
      cycle TEXT NOT NULL
    );
  `);

  _db.exec(`
    CREATE TABLE IF NOT EXISTS tags (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      name TEXT NOT NULL UNIQUE
    );
  `);

  _db.exec(`
    CREATE TABLE IF NOT EXISTS subscription_tags (
      subscription_id INTEGER NOT NULL,
      tag_id INTEGER NOT NULL,
      PRIMARY KEY (subscription_id, tag_id),
      FOREIGN KEY (subscription_id) REFERENCES subscriptions(id) ON DELETE CASCADE,
      FOREIGN KEY (tag_id) REFERENCES tags(id)
    );
  `);

  return _db;
}

// For testing: replace the DB instance (e.g. with in-memory)
export function __setDb(db: Database): void {
  _db = db;
}

// For testing: reset to use file DB
export function __resetDb(): void {
  if (_db) {
    _db.close();
    _db = null;
  }
}

function mapTags(subs: SharedArgs[]): SharedArgs[] {
  if (subs.length === 0) return subs;

  const db = getDb();
  const getTags = db.prepare(`
    SELECT tags.name FROM tags
    JOIN subscription_tags ON subscription_tags.tag_id = tags.id
    WHERE subscription_tags.subscription_id = ?
  `);

  for (const sub of subs) {
    const tagRows = getTags.all(sub.id) as { name: string }[];
    sub.tags = tagRows.map((r) => r.name);
  }

  return subs;
}

export const getSubscriptions = (): SharedArgs[] => {
  try {
    const db = getDb();
    const subs = db
      .prepare(
        "SELECT id, name, price, currency, cycle FROM subscriptions ORDER BY id",
      )
      .all() as SharedArgs[];
    return mapTags(subs);
  } catch (error) {
    consola.error("Failed to fetch subscriptions:", error);
    throw error;
  }
};

export const writeSubscription = (data: AddSharedArgs): void => {
  try {
    const db = getDb();
    const uniqueTags = Array.from(new Set(data.tags))

    const writeTx = db.transaction(() => {
      const insertSub = db.prepare(`
        INSERT INTO subscriptions (name, price, currency, cycle)
        VALUES (?, ?, ?, ?)
      `);

      const result = insertSub.run(
        data.name,
        data.price,
        data.currency,
        data.cycle,
      );

      const subscriptionId = Number(result.lastInsertRowid);

      const insertTag = db.prepare(`
        INSERT OR IGNORE INTO tags (name)
        VALUES (?)
      `);

      const getTagId = db.prepare(`
        SELECT id FROM tags WHERE name = ?
      `);

      const insertRel = db.prepare(`
        INSERT INTO subscription_tags (subscription_id, tag_id)
        VALUES (?, ?)
      `);

      for (const t of uniqueTags) {
        insertTag.run(t);
        const tagRow = getTagId.get(t) as { id: number } | undefined;
        if (tagRow) {
          insertRel.run(subscriptionId, tagRow.id);
        }
      }
    });

    writeTx();
  } catch (error) {
    consola.error("Failed to add subscription:", error);
    throw error;
  }
};

export const deleteSubscription = (id: number): void => {
  try {
    const db = getDb();
    const result = db
      .prepare("DELETE FROM subscriptions WHERE id = ?")
      .run(id);

    if (result.changes === 0) {
      consola.warn(`No subscription found with id ${id}`);
    }
  } catch (error) {
    consola.error("Failed to delete subscription:", error);
    throw error;
  }
};

export const tagsSubscription = (tag: string[] | string) => {
  try {
    const db = getDb();
    const tags = Array.from(new Set(Array.isArray(tag) ? tag : [tag]));
    if (tags.length === 0) return [];

    const placeholders = tags.map(() => "?").join(",");

    const rows = db
      .prepare(
        `
        SELECT subscription_tags.subscription_id
        FROM subscription_tags
        JOIN tags ON tags.id = subscription_tags.tag_id
        WHERE tags.name IN (${placeholders})
        GROUP BY subscription_tags.subscription_id
        HAVING COUNT(DISTINCT tags.name) = ?
      `,
      )
      .all(...tags, tags.length) as { subscription_id: number }[];

    const ids = rows.map((r) => r.subscription_id);
    if (ids.length === 0) return [];

    const stmt = db.prepare(`
      SELECT id, name, price, currency, cycle FROM subscriptions
      WHERE id IN (${ids.map(() => "?").join(",")})
    `);

    const subs = stmt.all(...ids) as SharedArgs[];
    return mapTags(subs);
  } catch (error) {
    consola.error("Failed to filter by tags:", error);
    throw error;
  }
};
