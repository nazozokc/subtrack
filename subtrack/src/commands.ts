import { input, confirm, checkbox, select } from "@inquirer/prompts"
import { consola } from "consola"
import { copyFileSync, statSync, constants } from "node:fs"
import { join } from "node:path"
import type { Currency, Cycle, SharedArgs, AddSharedArgs, AddFlags } from "./types.ts"
import {
  getSubscriptions,
  getSubscription,
  writeSubscription,
  updateSubscription,
  deleteSubscription,
  getAllTags,
  getTagsWithCount,
  tagsSubscription,
  renameTag,
  deleteTag,
  pruneTags,
  getDbPath,
  saveDb,
} from "./db.ts"
import {
  formatPrice,
  spreadSubscription,
  showApiUsage,
} from "./display.ts"
import { showPayment, showSummary } from "./payment.ts"
import { getLlmUsageTotal, getLlmUsageTotalByProvider } from "./db.ts"
import { exportCsv, exportMd, exportJson } from "./export.ts"
import { fetchFxRates, convertPrice } from "./fx.ts"
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
      message:
        "tags" +
        (existingTags.length > 0
          ? ` (existing: ${existingTags.join(", ")})`
          : ""),
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

export async function handleList(options: { currency?: string; sort?: string; desc?: boolean; api?: boolean }) {
  const list = getSubscriptions(options.sort, options.desc)
  await spreadSubscription(list, options.currency as Currency | undefined)

  if (options.api) {
    const now = new Date()
    const y = now.getFullYear()
    const m = now.getMonth() + 1
    const from = `${y}-${String(m).padStart(2, "0")}-01`
    const to = `${y}-${String(m).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`
    const monthLabel = `${now.toLocaleString("en-US", { month: "long" })} ${y}`

    const total = getLlmUsageTotal(from, to)
    const byProvider = getLlmUsageTotalByProvider(from, to)
    showApiUsage(total, byProvider, monthLabel)
  }
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
    return
  }
  if (!destStat.isDirectory()) {
    consola.error(`Backup destination must be a directory: ${destination}`)
    return
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
  }
}

export async function handleExport(
  format: string,
  options: { currency?: string; tags?: string },
) {
  if (format !== "csv" && format !== "json" && format !== "md") {
    consola.error(`Unsupported export format: "${format}". Supported: csv, json, md`)
    return
  }

  let list = options.tags
    ? tagsSubscription(options.tags.split(",").map((t) => t.trim()))
    : getSubscriptions()

  if (list.length === 0) {
    consola.info("No subscriptions found")
    return
  }

  if (options.currency) {
    try {
      const rates = await fetchFxRates()
      const targetCurrency = options.currency as Currency
      list = list.map((sub) => ({
        ...sub,
        price: Math.round(
          convertPrice(sub.price, sub.currency, targetCurrency, rates.rates),
        ),
        currency: targetCurrency,
      }))
    } catch (e) {
      consola.fail(
        `Failed to fetch exchange rates; exporting in original currencies: ${e}`,
      )
    }
  }

  const output = format === "csv" ? exportCsv(list) : format === "json" ? exportJson(list) : exportMd(list)
  consola.log(output)
}

export async function handlePayment(
  period: Cycle,
  options: { currency?: string; api?: boolean },
) {
  await showPayment(period, options.currency as Currency | undefined, undefined, options.api)
}

// ── Edit workflow ────────────────────────────────────────

export async function handleEdit(
  id?: number,
  flags: Partial<AddFlags> = {},
) {
  const all = getSubscriptions()
  if (all.length === 0) {
    consola.info("No subscriptions found")
    return
  }

  // Resolve subscription
  let sub: SharedArgs | undefined
  if (id !== undefined) {
    sub = getSubscription(id)
    if (!sub) {
      consola.error(`Subscription with id ${id} not found`)
      return
    }
  } else {
    const picked = await select({
      message: "select subscription to edit",
      loop: false,
      pageSize: 10,
      choices: all.map((s) => ({
        name: `${s.name} — ${formatPrice(s.price, s.currency)}/${s.cycle}${s.tags.length > 0 ? ` [${s.tags.join(", ")}]` : ""}`,
        value: s,
      })),
    })
    sub = picked
  }

  const hasFlags =
    flags.name !== undefined || flags.price !== undefined ||
    flags.currency !== undefined || flags.cycle !== undefined ||
    flags.tags !== undefined

  if (hasFlags) {
    // Non-interactive: update only flagged fields
    const newData: Partial<AddSharedArgs> = {}
    if (flags.name !== undefined) newData.name = flags.name
    if (flags.price !== undefined) newData.price = Number(flags.price)
    if (flags.currency !== undefined) newData.currency = flags.currency
    if (flags.cycle !== undefined) newData.cycle = flags.cycle
    if (flags.tags !== undefined) {
      newData.tags = flags.tags.split(",").map((t) => t.trim()).filter(Boolean)
    }
    updateSubscription(sub.id, newData)
    const updated = getSubscription(sub.id)!
    consola.success(
      `Updated: ${updated.name} — ${formatPrice(updated.price, updated.currency)}/${updated.cycle}`,
    )
    return
  }

  consola.info(
    `Editing: ${sub.name} — ${formatPrice(sub.price, sub.currency)}/${sub.cycle} [${sub.tags.join(", ") || "no tags"}]`,
  )

  // Interactive: pick fields to change
  const fields = await checkbox({
    message: "Select fields to edit:",
    loop: false,
    choices: [
      { name: `name (${sub.name})`, value: "name" },
      { name: `price (${formatPrice(sub.price, sub.currency)})`, value: "price" },
      { name: `currency (${sub.currency})`, value: "currency" },
      { name: `cycle (${sub.cycle})`, value: "cycle" },
      { name: `tags (${sub.tags.join(", ") || "none"})`, value: "tags" },
    ],
  })

  if (fields.length === 0) {
    consola.info("Cancelled")
    return
  }

  const newData: Partial<AddSharedArgs> = {}

  if (fields.includes("name")) {
    const name = await input({
      message: "New name:",
      default: sub.name,
      validate: validateName,
    })
    newData.name = name
  }
  if (fields.includes("price")) {
    const price = await input({
      message: "New price:",
      default: String(sub.price),
      validate: validatePrice,
    })
    newData.price = Number(price)
  }
  if (fields.includes("currency")) {
    const currency = await select({
      message: "New currency:",
      choices: CURRENCY_CHOICES,
    })
    newData.currency = currency
  }
  if (fields.includes("cycle")) {
    const cycle = await select({
      message: "New cycle:",
      choices: CYCLE_CHOICES,
    })
    newData.cycle = cycle
  }
  if (fields.includes("tags")) {
    const existingTags = getAllTags()
    const tags = await input({
      message:
        "New tags (comma-separated)" +
        (existingTags.length > 0 ? ` (existing: ${existingTags.join(", ")})` : ""),
      default: sub.tags.join(", "),
      validate: validateTags,
    })
    newData.tags = tags.split(",").map((t) => t.trim()).filter(Boolean)
  }

  const ok = await confirm({ message: "Save changes?", default: true })
  if (!ok) {
    consola.info("Cancelled")
    return
  }

  updateSubscription(sub.id, newData)
  const updated = getSubscription(sub.id)
  if (!updated) {
    consola.error("Failed to retrieve updated subscription")
    return
  }
  consola.success(
    `Updated: ${updated.name} — ${formatPrice(updated.price, updated.currency)}/${updated.cycle}`,
  )
}

// ── Tag management ────────────────────────────────────────

export function handleTagList() {
  const tags = getTagsWithCount()
  if (tags.length === 0) {
    consola.info("No tags found")
    return
  }
  const maxNameLen = Math.max(...tags.map((t) => t.name.length), 4)
  consola.log(`${"Name".padEnd(maxNameLen)}  Subscriptions`)
  consola.log("─".repeat(maxNameLen + 14))
  for (const t of tags) {
    consola.log(`${t.name.padEnd(maxNameLen)}  ${t.count}`)
  }
}

export function handleTagRename(oldName: string, newName: string) {
  if (!oldName || !newName) {
    consola.error("Usage: subtrack tag rename <old> <new>")
    return
  }
  try {
    if (renameTag(oldName, newName)) {
      consola.success(`Renamed tag: "${oldName}" → "${newName}"`)
    } else {
      consola.error(`Tag "${oldName}" not found`)
    }
  } catch (e) {
    consola.error(`Failed to rename tag: ${e}`)
  }
}

export function handleTagDelete(name: string) {
  if (!name) {
    consola.error("Usage: subtrack tag delete <name>")
    return
  }
  if (deleteTag(name)) {
    consola.success(`Deleted tag: "${name}"`)
  } else {
    consola.error(`Tag "${name}" not found`)
  }
}

export function handleTagPrune() {
  const count = pruneTags()
  if (count > 0) {
    consola.success(`Removed ${count} orphaned tag${count > 1 ? "s" : ""}`)
  } else {
    consola.info("No orphaned tags found")
  }
}

// ── Summary ───────────────────────────────────────────────

export async function handleSummary() {
  await showSummary()
}
