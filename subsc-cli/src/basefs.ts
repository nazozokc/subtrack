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
