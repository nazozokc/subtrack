import consola from "consola";
import { mkdirSync, readFileSync, writeFileSync } from "fs";
import { homedir } from "os";
import path from "path";

export type SharedArgs = {
  id: number;
  name: string;
  price: number;
  currency: "JPY" | "USD";
  cycle: "monthly" | "yearly";
  tags: string[];
};

const dir = path.join(
  homedir(),
  ".config",
  "subscription-cli",
  "subscriptin.json",
);

export const getSubscriptions = (): SharedArgs[] => {
  try {
    const result = readFileSync(dir, "utf-8");
    return JSON.parse(result).filter(Boolean);
  } catch (err) {
    return [];
  }
};

export const writeSubscription = (SharedArgs: SharedArgs[]): void => {
  mkdirSync(path.dirname(dir), { recursive: true });
  const json = JSON.stringify(SharedArgs, null, 2);
  try {
    writeFileSync(dir, json, "utf-8");
    consola.success("done add subscription");
  } catch (error) {
    consola.error("ファイルを作成することができませんでした");
  }
};

export const deleteSubscription = (id: number) => {
  const get = getSubscriptions();
  const filter = get.filter((n) => n.id !== id);
  writeSubscription(filter);
};
