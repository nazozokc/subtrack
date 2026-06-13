#!/usr/bin/env node
import { Command } from "commander";
import { input, select } from "@inquirer/prompts"
import { spreadSubscription } from "./table.ts";
import {
  deleteSubscription,
  writeSubscription,
  tagsSubscription,
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
      message: "monthly payment amount",
      validate: (value) => {
        if (value.trim() === "") {
          return "Please enter a valid number";
        }
        if (isNaN(Number(value)) || Number(value) < 0) {
          return "Please enter a valid non-negative number";
        }
        return true;
      },
    });

    const currency = await select({
      message: "currency",
      choices: [
        { name: "JPY", value: "JPY" },
        { name: "USD", value: "USD" },
      ],
    });

    const cycle = await select({
      message: "cycle",
      choices: [
        { name: "monthly", value: "monthly" },
        { name: "yearly", value: "yearly" },
      ],
    });

    const tagsInput = await input({
      message: "tags",
    });

    const tag = tagsInput
      .split(",")
      .map((tag) => tag.trim())
      .filter(Boolean);

    writeSubscription({
      name,
      price: Number(price),
      currency,
      cycle,
      tags: tag,
    });
  });

  program
    .command("delete")
    .argument("<number>")
    .action((number) => {
      deleteSubscription(Number(number));
    });

  program
    .command("tags")
    .argument("<taglist...>")
    .action((taglist) => {
      const list = tagsSubscription(taglist);
      spreadSubscription(list);
    });

  program.parse();
};

runCLI();
