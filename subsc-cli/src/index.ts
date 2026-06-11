#!/usr/bin/env node
import { Command } from "commander";
import { input, select } from "@inquirer/prompts";
import { spreadSubscription } from "./table.ts";
import {
  deleteSubscription,
  getSubscriptions,
  writeSubscription,
} from "./basefs.ts";

const runCLI = () => {
  const program = new Command();
  program.name("subsc-cli");

  program.command("list").action(() => {
    spreadSubscription();
  });

  program.command("add").action(async () => {
    const name = await input({
      message: "subscription name",
    });

    const price = await input({
      message: "payment subscribe service",
    });

    const currency = await select({
      message: "currency",
      choices: [
        { label: "JPY", value: "JPY" },
        { label: "USD", value: "USD" },
      ],
    });

    const cycle = await select({
      message: "cycle",
      choices: [
        { label: "monthly", value: "monthly" },
        { label: "yearly", value: "yearly" },
      ],
    });

    writeSubscription({
      name,
      price: Number(price),
      currency,
      cycle,
    });
  });

  program
    .command("delete")
    .argument("<number>")
    .action((number) => {
      deleteSubscription(Number(number));
    });

  program.parse();
};

runCLI();
