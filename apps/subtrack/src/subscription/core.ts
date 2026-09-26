/**
 * Core subscription command handlers: list, clone, archive, unarchive, tags.
 * Separated from the more complex add/edit workflows and the interactive
 * delete flow (subscription/delete.ts).
 */

import { subscriptionRepository, usageRepository } from "../application/index.ts"
import { isValidCurrency, isValidCycle, validatePrice } from "../validation.ts"
import { consola } from "@subtrack/lib/logger"
import { fail } from "../error.ts"
import { loadConfig } from "../config.ts"
import type { Currency, Cycle, AddFlags } from "../types.ts"
import { spreadSubscription, showApiUsage } from "../display.ts"
import { fetchConvertedSubs } from "../fx.ts"
import { logAudit } from "../audit-log.ts"
import { runPreCommandHooks } from "../pre-command.ts"

export async function handleList(options: {
  currency?: string
  sort?: string
  desc?: boolean
  api?: boolean
  notes?: boolean
  method?: boolean
  tags?: string
  json?: boolean
  limit?: number
  offset?: number
  includeArchived?: boolean
  showContract?: boolean
  showVendor?: boolean
  status?: string
  minPrice?: number
  maxPrice?: number
}) {
  // Auto-scan for new suggestions (non-blocking on failure)
  await runPreCommandHooks(options)

  const list = options.tags
    ? subscriptionRepository.list({
        tags: options.tags.split(",").map((t) => t.trim()),
        includeArchived: true,
      })
    : subscriptionRepository.list({
        sort: options.sort,
        desc: options.desc,
        limit: options.limit,
        offset: options.offset,
        includeArchived: options.includeArchived,
        status: options.status,
        minPrice: options.minPrice,
        maxPrice: options.maxPrice,
      })

  if (options.json) {
    process.stdout.write(JSON.stringify(list, null, 2) + "\n")
    return
  }
  // Flag > config > default (off)
  const showNotes = options.notes ?? loadConfig().listShowNotes === "on"
  const showMethod = options.method ?? loadConfig().listShowMethod === "on"

  // Currency conversion is the handler's job: fetch rates, convert, and pass
  // the converted list to the pure rendering layer. Skip the rate fetch when
  // nothing needs conversion (empty list, or every entry already uses the
  // target currency).
  let displayList = list
  let displayCurrency = options.currency as Currency | undefined
  if (displayCurrency && list.some((s) => s.currency !== displayCurrency)) {
    consola.info("Fetching the latest exchange rates...")
    const converted = await fetchConvertedSubs(list, displayCurrency)
    if (converted) {
      consola.success("Exchange rates updated")
      displayList = converted.list
      if (converted.hasMissing) {
        consola.warn(
          "Some prices could not be converted (missing rate). They are shown in original currency.",
        )
      }
    } else {
      consola.warn("Failed to fetch exchange rates; showing in original currencies")
      displayCurrency = undefined
    }
  }
  spreadSubscription(displayList, displayCurrency, showNotes, showMethod, options.showContract, options.showVendor)

  if (options.api) {
    const now = new Date()
    const y = now.getFullYear()
    const m = now.getMonth() + 1
    const from = `${y}-${String(m).padStart(2, "0")}-01`
    const to = `${y}-${String(m).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`
    const monthLabel = `${now.toLocaleString("en-US", { month: "long" })} ${y}`

    const total = usageRepository.totalCost(from, to)
    const byProvider = usageRepository.totalCostByProvider(from, to)
    showApiUsage(total, byProvider, monthLabel)
  }
}

export async function handleTags(taglist: string[]) {
  const list = subscriptionRepository.list({ tags: taglist, includeArchived: true })
  await spreadSubscription(list)
}

// ── Clone ──────────────────────────────────────────────

export async function handleClone(id: number, flags: Partial<AddFlags> = {}): Promise<void> {
  const sub = subscriptionRepository.get(id)
  if (!sub) {
    fail(`Subscription with id ${id} not found`)
    return
  }

  const newName = flags.name ?? `${sub.name} (copy)`
  let price = sub.price
  if (flags.price !== undefined) {
    const err = validatePrice(flags.price)
    if (err !== true) {
      fail(`Invalid price: ${err}`)
      return
    }
    price = Number(flags.price)
  }
  let currency = sub.currency
  if (flags.currency !== undefined) {
    if (!isValidCurrency(flags.currency)) {
      fail(`Invalid currency: "${flags.currency}"`)
      return
    }
    currency = flags.currency
  }
  let cycle: Cycle = sub.cycle
  if (flags.cycle !== undefined) {
    if (!isValidCycle(flags.cycle)) {
      fail(`Invalid cycle: "${flags.cycle}"`)
      return
    }
    cycle = flags.cycle
  }
  const newData = {
    name: newName,
    price,
    currency,
    cycle,
    tags: flags.tags ? flags.tags.split(",").map((t) => t.trim()).filter(Boolean) : [...sub.tags],
    status: sub.status,
    billingDay: sub.billingDay,
    notes: sub.notes,
    paymentMethod: sub.paymentMethod,
  }

  try {
    const newId = subscriptionRepository.add(newData)
    logAudit("subscription.clone", {
      targetType: "subscription",
      targetId: newId,
      details: `Cloned from #${id} "${sub.name}" → "${newName}"`,
    })
    consola.success(`Cloned: "${sub.name}" → "${newName}" (id=${newId})`)
  } catch (error) {
    fail(`Failed to clone subscription: ${String(error)}`)
  }
}

// ── Archive / Unarchive ──────────────────────────────────

export function handleArchive(id: number) {
  const sub = subscriptionRepository.get(id)
  if (!sub) {
    fail(`Subscription with id ${id} not found`)
    return
  }
  if (sub.status === "archived") {
    consola.info(`"${sub.name}" is already archived`)
    return
  }
  if (subscriptionRepository.archive(id)) {
    logAudit("subscription.archive", {
      targetType: "subscription",
      targetId: id,
      details: sub.name,
    })
    consola.success(`Archived: "${sub.name}"`)
  }
}

export function handleUnarchive(id: number) {
  const sub = subscriptionRepository.get(id)
  if (!sub) {
    fail(`Subscription with id ${id} not found`)
    return
  }
  if (sub.status !== "archived") {
    consola.info(`"${sub.name}" is not archived (status: ${sub.status})`)
    return
  }
  if (subscriptionRepository.unarchive(id)) {
    logAudit("subscription.unarchive", {
      targetType: "subscription",
      targetId: id,
      details: sub.name,
    })
    consola.success(`Unarchived: "${sub.name}"`)
  }
}
