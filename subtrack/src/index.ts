#!/usr/bin/env node
import { Command } from "commander";
import { input, select, confirm, checkbox } from "@inquirer/prompts";
import { consola } from "consola";
import type { Currency, Cycle } from "./basefs.ts";
import { spreadSubscription, formatPrice } from "./table.ts";
import {
  getSubscriptions,
  deleteSubscription,
  writeSubscription,
  tagsSubscription,
  getAllTags,
} from "./basefs.ts";

const CURRENCY_CHOICES: { name: string; value: Currency }[] = [
  { name: "JPY (日本円)", value: "JPY" },
  { name: "USD (US Dollar)", value: "USD" },
  { name: "EUR (Euro)", value: "EUR" },
  { name: "GBP (British Pound)", value: "GBP" },
  { name: "AUD (Australian Dollar)", value: "AUD" },
  { name: "CAD (Canadian Dollar)", value: "CAD" },
  { name: "KRW (South Korean Won)", value: "KRW" },
  { name: "CNY (Chinese Yuan)", value: "CNY" },
  { name: "SGD (Singapore Dollar)", value: "SGD" },
  { name: "HKD (Hong Kong Dollar)", value: "HKD" },
]

const CYCLE_CHOICES: { name: string; value: Cycle }[] = [
  { name: "weekly", value: "weekly" },
  { name: "bi-weekly", value: "bi-weekly" },
  { name: "monthly", value: "monthly" },
  { name: "quarterly", value: "quarterly" },
  { name: "semi-annual", value: "semi-annual" },
  { name: "yearly", value: "yearly" },
]

type AddFlags = {
  name?: string
  price?: string
  currency?: string
  cycle?: string
  tags?: string
}

function isValidCurrency(v: string): v is Currency {
  return CURRENCY_CHOICES.some((c) => c.value === v)
}

function isValidCycle(v: string): v is Cycle {
  return CYCLE_CHOICES.some((c) => c.value === v)
}

async function resolveAddOptions(flags: AddFlags) {
  let prompted = false

  // --- name ---
  let name = flags.name
  if (name === undefined) {
    prompted = true
    name = await input({
      message: "subscription name",
      validate: (v) => {
        if (!v.trim()) return "Name cannot be empty"
        if (v.length > 100) return "Name too long (max 100 chars)"
        return true
      },
    })
  } else {
    if (!name.trim()) {
      consola.error("Name cannot be empty")
      return null
    }
    if (name.length > 100) {
      consola.error("Name too long (max 100 chars)")
      return null
    }
  }

  // --- price ---
  let priceStr = flags.price
  if (priceStr === undefined) {
    prompted = true
    priceStr = await input({
      message: "monthly payment amount",
      validate: (v) => {
        if (!v.trim()) return "Please enter a valid number"
        if (isNaN(Number(v)) || Number(v) < 0)
          return "Please enter a valid non-negative number"
        if (Number(v) > 99999999) return "Price too high (max 99,999,999)"
        return true
      },
    })
  } else {
    if (isNaN(Number(priceStr)) || Number(priceStr) < 0) {
      consola.error("Price must be a valid non-negative number")
      return null
    }
    if (Number(priceStr) > 99999999) {
      consola.error("Price too high (max 99,999,999)")
      return null
    }
  }

  // --- currency ---
  let currency: Currency
  if (flags.currency === undefined) {
    prompted = true
    currency = await select({
      message: "currency",
      choices: CURRENCY_CHOICES,
    })
  } else {
    if (!isValidCurrency(flags.currency)) {
      consola.error(
        `Invalid currency "${flags.currency}". Valid: ${CURRENCY_CHOICES.map((c) => c.value).join(", ")}`,
      )
      return null
    }
    currency = flags.currency
  }

  // --- cycle ---
  let cycle: Cycle
  if (flags.cycle === undefined) {
    prompted = true
    cycle = await select({
      message: "cycle",
      choices: CYCLE_CHOICES,
    })
  } else {
    if (!isValidCycle(flags.cycle)) {
      consola.error(
        `Invalid cycle "${flags.cycle}". Valid: ${CYCLE_CHOICES.map((c) => c.value).join(", ")}`,
      )
      return null
    }
    cycle = flags.cycle
  }

  // --- tags ---
  let tagsStr = flags.tags
  if (tagsStr === undefined) {
    prompted = true
    const existingTags = getAllTags()
    const hint =
      existingTags.length > 0
        ? `existing: ${existingTags.join(", ")}`
        : undefined

    tagsStr = await input({
      message: "tags",
      hint,
      validate: (v) => {
        if (!v.trim()) return true
        const tags = v.split(",").map((t) => t.trim()).filter(Boolean)
        if (tags.length > 10) return "Maximum 10 tags allowed"
        for (const tag of tags) {
          if (tag.length > 50) return `Tag too long: "${tag}" (max 50 chars)`
        }
        return true
      },
    })
  }

  const tags = tagsStr
    .split(",")
    .map((t) => t.trim())
    .filter(Boolean)

  const price = Number(priceStr)

  // --- confirm in interactive mode ---
  if (prompted) {
    const ok = await confirm({
      message: `Save "${name}" (${formatPrice(price, currency)}, ${cycle})?`,
      default: true,
    })
    if (!ok) {
      consola.info("Cancelled")
      return null
    }
  }

  return { name: name.trim(), price, currency, cycle, tags }
}

const runCLI = () => {
  const program = new Command();
  program.name("subtrack");

  program
    .command("list")
    .option("-c, --currency <currency>", "convert all prices to currency")
    .action(async (options) => {
      await spreadSubscription(
        undefined,
        options.currency as Currency | undefined,
      );
    });

  program
    .command("add")
    .option("--name <name>", "subscription name")
    .option("--price <price>", "monthly payment amount")
    .option("--currency <currency>", "currency")
    .option("--cycle <cycle>", "billing cycle")
    .option("--tags <tags>", "comma-separated tags")
    .action(async (flags) => {
      const result = await resolveAddOptions(flags)
      if (!result) return
      writeSubscription(result)
      consola.success(`Added subscription: ${result.name}`)
    })

  program
    .command("delete")
    .action(async () => {
      const all = getSubscriptions()

      if (all.length === 0) {
        consola.info("No subscriptions found")
        return
      }

      const selected = await checkbox({
        message: "select subscriptions to delete",
        choices: all.map((sub) => ({
          name: `${sub.name} — ${formatPrice(sub.price, sub.currency)}/${sub.cycle}${sub.tags.length > 0 ? ` [${sub.tags.join(", ")}]` : ""}`,
          value: sub,
        })),
      })

      if (selected.length === 0) {
        consola.info("Cancelled")
        return
      }

      const names = selected.map((s) => s.name).join(", ")
      const ok = await confirm({
        message: `Delete ${selected.length} subscription${selected.length > 1 ? "s" : ""}? (${names})`,
        default: false,
      })

      if (!ok) {
        consola.info("Cancelled")
        return
      }

      for (const sub of selected) {
        deleteSubscription(sub.id)
        consola.success(`Deleted: ${sub.name}`)
      }
    })

  program
    .command("tags")
    .argument("<taglist...>")
    .action(async (taglist) => {
      const list = tagsSubscription(taglist);
      await spreadSubscription(list);
    });

  program.parse();
};

runCLI();
