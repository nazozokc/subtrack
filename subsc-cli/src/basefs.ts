import { DatabaseSync } from "node:sqlite";
import { mkdirSync } from "node:fs";
import path from "path";
import { homedir } from "os";

export type SharedArgs = {
  id: number;
  name: string;
  price: number;
  currency: "JPY" | "USD";
  cycle: "monthly" | "yearly";
  tags: string[];
};

//Omit はSharedArgsからidだけ消した型
export type AddSharedArgs = Omit<SharedArgs, "id">;

const dbdir = path.join(homedir(), ".config", "subsc-cli");

mkdirSync(dbdir, {
  recursive: true,
});

const db = new DatabaseSync(path.join(dbdir, "subscriptions.db"));

db.exec(`
CREATE TABLE IF NOT EXISTS subscriptions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  price INTEGER NOT NULL,
  currency TEXT NOT NULL,
  cycle TEXT NOT NULL
  );
`);

db.exec(`
  CREATE TABLE IF NOT EXISTS tags (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL UNIQUE
  );
`);

db.exec(`
  CREATE TABLE IF NOT EXISTS subscription_tags (
  subscription_id INTEGER NOT NULL,
  tag_id INTEGER NOT NULL,

  PRIMARY KEY (subscription_id, tag_id),

  FOREIGN KEY (subscription_id)
    REFERENCES subscriptions(id),

  FOREIGN KEY (tag_id)
    REFERENCES tags(id)
  );
`);

export const getSubscriptions = (): SharedArgs[] => {
  return db.prepare("SELECT * FROM subscriptions").all() as SharedArgs[];
};

export const writeSubscription = (data: AddSharedArgs): void => {
  const int = db.prepare(`
    INSERT INTO subscriptions (name, price, currency, cycle)
    VALUES (?, ?, ?, ?)
  `);

  int.run(data.name, data.price, data.currency, data.cycle);
};

export const deleteSubscription = (id: number): void => {
  db.prepare(`DELETE FROM subscriptions WHERE id = ?`).run(id);
};

export const tagsSubscription = (tag: string[] | string) => {
  const tags = Array.isArray(tag) ? tag : [tag];

  const placeholders = tags.map(() => "?").join(",");

  const stmt = db.prepare(`
    SELECT subscriptions.*
    FROM subscriptions
    JOIN subscription_tags
      ON subscriptions.id = subscription_tags.subscription_id
    JOIN tags
      ON tags.id = subscription_tags.tag_id
    WHERE tags.name IN (${placeholders})
    GROUP BY subscriptions.id
    HAVING COUNT(DISTINCT tags.name) = ?
  `);

  return stmt.all(...tags, tags.length) as SharedArgs[];
};
