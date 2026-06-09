import Table from "cli-table3";
import { consola } from "consola";
import { type SharedArgs, getSubscriptions } from "./fs.ts";

export const spreadSubscription = (): void => {
  const list = getSubscriptions();

  if (list.length === 0) {
    consola.info("No subscriptions found");
    return;
  }
  const table = new Table({
    head: ["name", "cycle", "tags", "price"],
  });

  for (const sub of list) {
    table.push([
      String(sub.name),
      String(sub.cycle),
      String(sub.tags.join(" | ")),
      sub.currency === "USD"
        ? String(`$${sub.price}`)
        : String(`\\${sub.price}`),
    ]);
  }

  consola.log(table.toString());
};
