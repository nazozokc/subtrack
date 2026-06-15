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

// ── Validators ──────────────────────────────────────────

function validateName(v: string): string | true {
  if (!v.trim()) return "Name cannot be empty"
  if (v.length > 100) return "Name too long (max 100 chars)"
  return true
}

function validatePrice(v: string): string | true {
  if (!v.trim()) return "Please enter a valid number"
  if (isNaN(Number(v)) || Number(v) < 0)
    return "Please enter a valid non-negative number"
  if (Number(v) > 99999999) return "Price too high (max 99,999,999)"
  return true
}

function validateTags(v: string): string | true {
  if (!v.trim()) return true
  const tags = v.split(",").map((t) => t.trim()).filter(Boolean)
  if (tags.length > 10) return "Maximum 10 tags allowed"
  for (const tag of tags) {
    if (tag.length > 50) return `Tag too long: "${tag}" (max 50 chars)`
  }
  return true
}

// ── Reusable prompt-or-flag helpers ─────────────────────

async function promptString(
  flag: string | undefined,
  message: string,
  validate: (v: string) => string | true,
): Promise<{ value: string; prompted: boolean } | null> {
  if (flag !== undefined) {
    const result = validate(flag)
    if (result !== true) {
      consola.error(result)
      return null
    }
    return { value: flag, prompted: false }
  }
  return { value: await input({ message, validate }), prompted: true }
}

function validChoices<T>(choices: { value: T }[]): string {
  return choices.map((c) => c.value).join(", ")
}

async function promptSelect<T extends string>(
  flag: string | undefined,
  message: string,
  choices: { name: string; value: T }[],
  isValid: (v: string) => v is T,
): Promise<{ value: T; prompted: boolean } | null> {
  if (flag !== undefined) {
    if (!isValid(flag)) {
      consola.error(`Invalid "${flag}". Valid: ${validChoices(choices)}`)
      return null
    }
    return { value: flag, prompted: false }
  }
  return { value: await select({ message, choices }), prompted: true }
}

// ── Add logic ───────────────────────────────────────────

async function resolveAddOptions(flags: AddFlags) {
  const nameRes = await promptString(flags.name, "subscription name", validateName)
  if (!nameRes) return null

  const priceRes = await promptString(flags.price, "monthly payment amount", validatePrice)
  if (!priceRes) return null

  const currencyRes = await promptSelect(
    flags.currency, "currency", CURRENCY_CHOICES, isValidCurrency,
  )
  if (!currencyRes) return null

  const cycleRes = await promptSelect(
    flags.cycle, "cycle", CYCLE_CHOICES, isValidCycle,
  )
  if (!cycleRes) return null

  // tags: special case — hint from existing tags, no flag-fallback validation needed
  let tagsStr = flags.tags
  let prompted = nameRes.prompted || priceRes.prompted || currencyRes.prompted || cycleRes.prompted

  if (tagsStr === undefined) {
    prompted = true
    const existingTags = getAllTags()
    tagsStr = await input({
      message: "tags",
      hint: existingTags.length > 0 ? `existing: ${existingTags.join(", ")}` : undefined,
      validate: validateTags,
    })
  }

  const tags = tagsStr.split(",").map((t) => t.trim()).filter(Boolean)
  const price = Number(priceRes.value)
  const name = nameRes.value.trim()
  const currency = currencyRes.value
  const cycle = cycleRes.value

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

  return { name, price, currency, cycle, tags }
}

// ── Command handlers ────────────────────────────────────

async function handleList(options: { currency?: string }) {
  await spreadSubscription(undefined, options.currency as Currency | undefined)
}

async function handleAdd(flags: AddFlags) {
  const result = await resolveAddOptions(flags)
  if (!result) return
  writeSubscription(result)
  consola.success(`Added subscription: ${result.name}`)
}

async function handleDelete() {
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
}

async function handleTags(taglist: string[]) {
  const list = tagsSubscription(taglist)
  await spreadSubscription(list)
}

// ── CLI setup ───────────────────────────────────────────

const runCLI = () => {
  const program = new Command()
  program.name("subtrack")

  program
    .command("list")
    .option("-c, --currency <currency>", "convert all prices to currency")
    .action(handleList)

  program
    .command("add")
    .option("--name <name>", "subscription name")
    .option("--price <price>", "monthly payment amount")
    .option("--currency <currency>", "currency")
    .option("--cycle <cycle>", "billing cycle")
    .option("--tags <tags>", "comma-separated tags")
    .action(handleAdd)

  program.command("delete").action(handleDelete)

  program
    .command("tags")
    .argument("<taglist...>")
    .action(handleTags)

  program.parse()
}

runCLI();
