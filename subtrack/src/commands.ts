import { input, confirm, checkbox, select } from "@inquirer/prompts"
import { consola } from "consola"
import { copyFileSync, statSync, constants, readFileSync, existsSync } from "node:fs"
import { join } from "node:path"
import type { Currency, Cycle, SharedArgs, AddSharedArgs } from "./db.ts"
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
  showPayment,
  showSummary,
  exportCsv,
  exportMd,
  exportJson,
  fetchFxRates,
  convertPrice,
} from "./display.ts"
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

export async function handleList(options: { currency?: string; sort?: string; desc?: boolean }) {
  const list = getSubscriptions(options.sort, options.desc)
  await spreadSubscription(list, options.currency as Currency | undefined)
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

export async function handleExport(
  format: string,
  options: { currency?: string; tags?: string },
) {
  if (format !== "csv" && format !== "json" && format !== "md") {
    consola.error(`Unsupported export format: "${format}". Supported: csv, json, md`)
    process.exit(1)
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
  options: { currency?: string },
) {
  await showPayment(period, options.currency as Currency | undefined)
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

// ── Import CSV ────────────────────────────────────────────

export function parseCsvLine(line: string): string[] {
  const fields: string[] = []
  let current = ""
  let inQuotes = false
  for (let i = 0; i < line.length; i++) {
    const ch = line[i]
    if (inQuotes) {
      if (ch === '"') {
        if (i + 1 < line.length && line[i + 1] === '"') {
          current += '"'
          i++
        } else {
          inQuotes = false
        }
      } else {
        current += ch
      }
    } else if (ch === ",") {
      fields.push(current)
      current = ""
    } else if (ch === '"') {
      inQuotes = true
    } else {
      current += ch
    }
  }
  fields.push(current)
  return fields
}

export async function handleImport(
  file: string,
  options: { dryRun?: boolean },
) {
  if (!file) {
    consola.error("Usage: subtrack import <file> [--dry-run]")
    return
  }

  if (!existsSync(file)) {
    consola.error(`File not found: ${file}`)
    return
  }

  const content = readFileSync(file, "utf-8")
  // Strip BOM
  const clean = content.charCodeAt(0) === 0xfeff ? content.slice(1) : content
  const lines = clean.split("\n").map((l) => l.trim()).filter(Boolean)

  if (lines.length < 2) {
    consola.error("CSV file must have a header row and at least one data row")
    return
  }

  // Validate header
  const header = parseCsvLine(lines[0]).map((h) => h.toLowerCase().trim())
  if (header.join(",") !== "name,cycle,tags,price,currency") {
    consola.error(
      `Invalid CSV header. Expected: name,cycle,tags,price,currency`,
    )
    return
  }

  let success = 0
  let failed = 0

  for (let i = 1; i < lines.length; i++) {
    const fields = parseCsvLine(lines[i])
    if (fields.length < 5) {
      consola.warn(`Line ${i + 1}: skipping (expected 5 fields, got ${fields.length})`)
      failed++
      continue
    }

    const [name, cycle, tagsStr, priceStr, currency] = fields

    // Validate
    const nameErr = validateName(name)
    if (nameErr !== true) { consola.warn(`Line ${i + 1}: ${nameErr}`); failed++; continue }

    const priceErr = validatePrice(priceStr)
    if (priceErr !== true) { consola.warn(`Line ${i + 1}: ${priceErr}`); failed++; continue }

    if (!isValidCurrency(currency)) {
      consola.warn(`Line ${i + 1}: invalid currency "${currency}"`)
      failed++
      continue
    }
    if (!isValidCycle(cycle)) {
      consola.warn(`Line ${i + 1}: invalid cycle "${cycle}"`)
      failed++
      continue
    }

    const tags = tagsStr.split(";").map((t) => t.trim()).filter(Boolean)
    const tagsErr = validateTags(tags.join(","))
    if (tagsErr !== true) { consola.warn(`Line ${i + 1}: ${tagsErr}`); failed++; continue }

    if (options.dryRun) {
      consola.info(`[dry-run] Would import: ${name} (${priceStr} ${currency}, ${cycle})`)
      success++
    } else {
      try {
        writeSubscription({
          name: name.trim(),
          price: Number(priceStr),
          currency,
          cycle,
          tags,
        })
        success++
      } catch (e) {
        consola.warn(`Line ${i + 1}: failed to import: ${e}`)
        failed++
      }
    }
  }

  if (options.dryRun) {
    consola.success(`Dry-run complete: ${success} valid, ${failed} invalid`)
  } else {
    consola.success(`Import complete: ${success} imported, ${failed} failed`)
  }
}

// ── Summary ───────────────────────────────────────────────

export async function handleSummary() {
  await showSummary()
}
