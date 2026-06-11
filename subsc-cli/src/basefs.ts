import Database from "better-sqlite3";
import path from "path";
import { homedir } from "os";

export type SharedArgs = {
  name: string;
  price: number;
  currency: "JPY" | "USD";
  cycle: "monthly" | "yearly";
  tags: string[];
};

const dbdir = path.join(homedir(), ".config", "subsc-cli", "subscriptions.db");
const db = new Database(dbdir);

db.exec(`
CREATE TABLE IF NOT EXISTS subscriptions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  price INTEGER NOT NULL,
  currency TEXT NOT NULL,
  cycle TEXT NOT NULL,
  );
`);

export const getSubscriptions = (): SharedArgs[] => {
  return db.prepare("SELECT * FROM subscriptions").all()
}

export writeSubscription = (SharedArgs: SharedArgs[]): void => {
  const int = db.prepare(`
    INSERT INTO subscriptions (name, price currency, cycle)
    VALUES (?, ?, ?, ?)
  `);

  int.run(SharedArgs.name, SharedArgs.price, SharedArgs.currency, SharedArgs.cycle);
};

export const deleteSubscription = (id: number): void => {
  db.prepare(`DELETE FROM subscriptions WHERE id = ?`).run(id);
}

export const tagsSubscription = (tag: string,): SharedArgs[] => {
  const stmt = db.prepare(`
    SELECT subscriptions.*
    FROM subscriptions
    JOIN subscription_tags
      ON subscriptions.id = subscription_tags.subscription_id
    JOIN tags
      ON tags.id = subscription_tags.tag_id
    WHERE tags.name = ?
  `);

  return stmt.all(tag) as SharedArgs[];
};
