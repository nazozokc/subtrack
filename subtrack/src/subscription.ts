import { input, confirm, checkbox, select } from "@inquirer/prompts"
import { consola } from "consola"
import type { Currency, Cycle, Status, SharedArgs, AddSharedArgs, AddFlags } from "./types.ts"
import {
  getSubscriptions,
  getSubscription,
  writeSubscription,
  updateSubscription,
  deleteSubscription,
  getAllTags,
  tagsSubscription,
  getLlmUsageTotal,
  getLlmUsageTotalByProvider,
} from "./db.ts"
import {
  formatPrice,
  spreadSubscription,
  showApiUsage,
} from "./display.ts"
import {
  CURRENCY_CHOICES,
  CYCLE_CHOICES,
  STATUS_CHOICES,
  isValidCurrency,
  isValidCycle,
  isValidStatus,
  validateName,
  validatePrice,
  validateTags,
  validateBillingDay,
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
    if (prompted) {
      const existingTags = getAllTags()
      tagsStr = await input({
        message:
          "tags" +
          (existingTags.length > 0
            ? ` (existing: ${existingTags.join(", ")})`
            : ""),
        validate: validateTags,
      })
    } else {
      tagsStr = ""
    }
  }

  // billingDay: optional, 1-31
  let billingDay: number | null = null
  const billingDayStr = flags.billingDay
  if (billingDayStr !== undefined) {
    const trimmed = billingDayStr.trim()
    if (trimmed) {
      const valid = validateBillingDay(trimmed)
      if (valid !== true) { consola.error(valid); return null }
      billingDay = Number(trimmed)
    }
  } else if (prompted) {
    const dayStr = await input({
      message: "billing day (1-31, optional)",
      validate: validateBillingDay,
    })
    if (dayStr.trim()) billingDay = Number(dayStr)
  }

  // status
  const statusRes = await promptSelect(
    flags.status,
    "status",
    STATUS_CHOICES,
    isValidStatus,
  )
  if (!statusRes) return null

  const tags = tagsStr.split(",").map((t) => t.trim()).filter(Boolean)
  const price = Number(priceRes.value)
  const name = nameRes.value.trim()
  const currency = currencyRes.value
  const cycle = cycleRes.value
  const status = statusRes.value
  prompted = prompted || statusRes.prompted

  if (prompted) {
    const extra = status !== "active" ? `, status: ${status}` : ""
    const ok = await confirm({
      message: `Save "${name}" (${formatPrice(price, currency)}, ${cycle}${extra})?`,
      default: true,
    })
    if (!ok) {
      consola.info("Cancelled")
      return null
    }
  }

  return { name, price, currency, cycle, tags, status, billingDay }
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
    consola.error(`Failed to add subscription: ${String(error)}`)
  }
}

export async function handleDelete(ids?: number[]) {
  if (ids && ids.length > 0) {
    for (const id of ids) {
      const sub = getSubscription(id)
      if (!sub) {
        consola.error(`Subscription with id ${id} not found`)
        continue
      }
      deleteSubscription(id)
      consola.success(`Deleted: ${sub.name}`)
    }
    return
  }

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
    flags.tags !== undefined || flags.status !== undefined ||
    flags.billingDay !== undefined

  if (hasFlags) {
    // Non-interactive: update only flagged fields
    const newData: Partial<AddSharedArgs> = {}
    if (flags.name !== undefined) newData.name = flags.name
    if (flags.price !== undefined) newData.price = Number(flags.price)
    if (flags.currency !== undefined) newData.currency = flags.currency
    if (flags.cycle !== undefined) newData.cycle = flags.cycle as Cycle
    if (flags.status !== undefined) newData.status = flags.status as Status
    if (flags.billingDay !== undefined) {
      const trimmed = flags.billingDay.trim()
      newData.billingDay = trimmed ? Number(trimmed) : null
    }
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
      { name: `status (${sub.status})`, value: "status" },
      { name: `billing day (${sub.billingDay ?? "not set"})`, value: "billingDay" },
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
  if (fields.includes("status")) {
    const status = await select({
      message: "New status:",
      choices: STATUS_CHOICES,
    })
    newData.status = status
  }
  if (fields.includes("billingDay")) {
    const day = await input({
      message: "New billing day (1-31, empty to clear):",
      default: sub.billingDay ? String(sub.billingDay) : "",
      validate: validateBillingDay,
    })
    newData.billingDay = day.trim() ? Number(day) : null
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
