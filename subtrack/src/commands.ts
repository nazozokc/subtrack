import { input, confirm, checkbox } from "@inquirer/prompts"
import { consola } from "consola"
import { copyFileSync, statSync, constants } from "node:fs"
import { join } from "node:path"
import type { Currency } from "./db.ts"
import {
  getSubscriptions,
  writeSubscription,
  deleteSubscription,
  getAllTags,
  tagsSubscription,
  getDbPath,
  saveDb,
} from "./db.ts"
import { formatPrice, spreadSubscription } from "./display.ts"
import {
  CURRENCY_CHOICES,
  CYCLE_CHOICES,
  isValidCurrency,
  isValidCycle,
  validateName,
  validatePrice,
  validateTags,
  promptString,
  promptSelect,
} from "./prompts.ts"

export type AddFlags = {
  name?: string
  price?: string
  currency?: string
  cycle?: string
  tags?: string
}

// ── Add workflow ────────────────────────────────────────

async function resolveAddOptions(flags: AddFlags) {
  const nameRes = await promptString(
    flags.name,
    "subscription name",
    validateName,
  )
  if (!nameRes) return null

  const priceRes = await promptString(
    flags.price,
    "monthly payment amount",
    validatePrice,
  )
  if (!priceRes) return null

  const currencyRes = await promptSelect(
    flags.currency,
    "currency",
    CURRENCY_CHOICES,
    isValidCurrency,
  )
  if (!currencyRes) return null

  const cycleRes = await promptSelect(
    flags.cycle,
    "cycle",
    CYCLE_CHOICES,
    isValidCycle,
  )
  if (!cycleRes) return null

  // tags: special case — hint from existing tags, no flag-fallback validation needed
  let tagsStr = flags.tags
  let prompted =
    nameRes.prompted ||
    priceRes.prompted ||
    currencyRes.prompted ||
    cycleRes.prompted

  if (tagsStr === undefined) {
    prompted = true
    const existingTags = getAllTags()
    tagsStr = await input({
      message: "tags",
      hint:
        existingTags.length > 0
          ? `existing: ${existingTags.join(", ")}`
          : undefined,
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

export async function handleList(options: { currency?: string }) {
  await spreadSubscription(undefined, options.currency as Currency | undefined)
}

export async function handleAdd(flags: AddFlags) {
  const result = await resolveAddOptions(flags)
  if (!result) return
  try {
    writeSubscription(result)
    consola.success(`Added subscription: ${result.name}`)
  } catch (error) {
    consola.error("Failed to add subscription:", error)
  }
}

export async function handleDelete() {
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

export async function handleTags(taglist: string[]) {
  const list = tagsSubscription(taglist)
  await spreadSubscription(list)
}

export async function handleBackup(destination: string) {
  // flush in-memory state to disk
  saveDb()

  // validate destination
  let destStat
  try {
    destStat = statSync(destination)
  } catch {
    consola.error(`Backup destination does not exist: ${destination}`)
    process.exit(1)
  }
  if (!destStat.isDirectory()) {
    consola.error(`Backup destination must be a directory: ${destination}`)
    process.exit(1)
  }

  // generate timestamped filename
  const now = new Date()
  const ts = `${now.getFullYear()}${String(now.getMonth() + 1).padStart(2, "0")}${String(now.getDate()).padStart(2, "0")}_${String(now.getHours()).padStart(2, "0")}${String(now.getMinutes()).padStart(2, "0")}${String(now.getSeconds()).padStart(2, "0")}`
  const destPath = join(destination, `subtrack_${ts}.db`)

  // copy with exclusive create to prevent overwrite
  try {
    copyFileSync(getDbPath(), destPath, constants.COPYFILE_EXCL)
    consola.success(`Backup created: ${destPath}`)
  } catch (err) {
    const nodeErr = err as NodeJS.ErrnoException
    if (nodeErr.code === "EEXIST") {
      consola.error(`Backup file already exists: ${destPath}`)
    } else {
      consola.error(`Backup failed: ${nodeErr.message}`)
    }
    process.exit(1)
  }
}
