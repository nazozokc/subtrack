import { test, expect, beforeEach, afterEach } from "vitest"
import { consola } from "consola"
import { spreadSubscription } from "./table"
import type { SharedArgs } from "./basefs"

const logMessages: string[] = [];
const infoMessages: string[] = [];

beforeEach(() => {
  logMessages.length = 0;
  infoMessages.length = 0;

  consola.mockTypes((_type: string, _defaults: object) => {
    return (...args: unknown[]) => {
      const str = args.map((a) => String(a)).join(" ");
      if (_type === "log") logMessages.push(str);
      if (_type === "info") infoMessages.push(str);
    };
  });
});

afterEach(() => {
  consola.mockTypes();
});

function makeSub(overrides: Partial<SharedArgs> = {}): SharedArgs {
  return {
    id: 1,
    name: "Test Service",
    price: 1000,
    currency: "JPY",
    cycle: "monthly",
    tags: [],
    ...overrides,
  }
}

test("shows info message when no subscriptions", () => {
  spreadSubscription([]);
  expect(infoMessages).toContain("No subscriptions found");
  expect(logMessages).toHaveLength(0);
});

test("displays table with single subscription", () => {
  spreadSubscription([makeSub({ name: "Netflix", price: 1500 })]);

  expect(logMessages).toHaveLength(1);
  const table = logMessages[0];
  expect(table).toContain("Netflix");
  expect(table).toContain("¥1500");
  expect(table).toContain("monthly");
  expect(table).toContain("JPY TOTAL");
  expect(table).toContain("USD TOTAL");
});

test("displays table with JPY currency symbol", () => {
  spreadSubscription([
    makeSub({ name: "iCloud+", price: 400, currency: "JPY" }),
  ]);

  const table = logMessages[0];
  expect(table).toContain("¥400");
});

test("displays table with USD currency symbol", () => {
  spreadSubscription([
    makeSub({ name: "GitHub", price: 10, currency: "USD" }),
  ]);

  const table = logMessages[0];
  expect(table).toContain("$10");
});

test("displays tags column", () => {
  spreadSubscription([
    makeSub({ name: "Netflix", tags: ["video", "entertainment"] }),
  ]);

  const table = logMessages[0];
  expect(table).toContain("video, entertainment");
});

test("displays dash when no tags", () => {
  spreadSubscription([makeSub({ name: "Dropbox", tags: [] })]);

  const table = logMessages[0];
  expect(table).toContain("Dropbox");
  expect(table).toContain("-");
});

test("shows correct JPY total", () => {
  spreadSubscription([
    makeSub({ name: "A", price: 500, currency: "JPY" }),
    makeSub({ name: "B", price: 1500, currency: "JPY" }),
  ]);

  const table = logMessages[0];
  expect(table).toContain("¥2000");
  expect(table).toContain("$0");
});

test("shows correct USD total", () => {
  spreadSubscription([
    makeSub({ name: "A", price: 10, currency: "USD" }),
    makeSub({ name: "B", price: 20, currency: "USD" }),
  ]);

  const table = logMessages[0];
  expect(table).toContain("$30");
  expect(table).toContain("¥0");
});

test("shows mixed currency totals correctly", () => {
  spreadSubscription([
    makeSub({ name: "JP", price: 1000, currency: "JPY" }),
    makeSub({ name: "US", price: 15, currency: "USD" }),
  ]);

  const table = logMessages[0];
  expect(table).toContain("¥1000");
  expect(table).toContain("$15");
});

test("shows yearly cycle", () => {
  spreadSubscription([makeSub({ name: "Annual", cycle: "yearly" })]);

  const table = logMessages[0];
  expect(table).toContain("yearly");
});

test("displays multiple subscriptions", () => {
  const subs = [
    makeSub({ id: 1, name: "A", price: 10, currency: "USD", tags: ["x"] }),
    makeSub({ id: 2, name: "B", price: 20, currency: "USD", tags: ["y"] }),
    makeSub({ id: 3, name: "C", price: 30, currency: "USD", tags: ["z"] }),
  ];

  spreadSubscription(subs);

  const table = logMessages[0];
  expect(table).toContain("A");
  expect(table).toContain("B");
  expect(table).toContain("C");
  expect(table).toContain("$60");
});
