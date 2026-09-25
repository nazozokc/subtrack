/**
 * MCP tool handler implementations.
 * Each exported function handles one tool and returns an McpResponse.
 */

import type { AddSharedArgs, Cycle, Status, Currency } from "../types.ts"
import type { FxRates } from "../fx.ts"
import type { McpResponse } from "./types.ts"
import {
  getSubscriptions,
  saveDb,
  getNonCancelledSubscriptions,
  getSubscription,
  writeSubscription,
  deleteSubscription,
  updateSubscription,
  getPriceHistory,
  getAllPriceChanges,
  getTrials,
  getTrialsExpiringSoon,
  getTagsWithCount,
  tagsSubscription,
  getLlmUsage,
  getLlmUsageTotal,
  getLlmUsageTokenTotal,
  getLlmUsageTotalByProvider,
  getLlmUsageTotalByModel,
} from "../db.ts"
import { calcSummary, calcSubTotal, calcPreviousTotals } from "../payment.ts"
import { getPeriodDateRange, periodFactor } from "@subtrack/lib/date"
import type { NamedCycle } from "@subtrack/lib/date"
import { calcCalendarEntries } from "../calendar.ts"
import { exportCsv, exportJson, exportMd } from "../export.ts"
import { fetchFxRates, convertPrice } from "../fx.ts"
import { searchSubscriptions } from "../search.ts"
import { roundCurrency } from "../price.ts"
import { calcUpcoming } from "../upcoming.ts"
import {
  isValidCycle, isValidStatus, isValidCurrency,
  validateName, validatePrice, validateBillingDay, validateNotes,
  validatePaymentMethod, validateTags, isValidNamedCycle,
} from "../validation.ts"
import { MAX_TAG_COUNT, rateLimiter, validateToolCall } from "./security.ts"

const MAX_BULK_AFFECTED = 1_000
const MAX_NEW_TAGS = 10

const errorResponse = (message: string): McpResponse => ({
  content: [{ type: "text", text: message }],
  isError: true,
})

export async function handleListSubscriptions(args?: Record<string, unknown>): Promise<McpResponse> {
  const subs = getSubscriptions({
    sort: args?.sort as string | undefined,
    desc: args?.desc as boolean | undefined,
    limit: args?.limit as number | undefined,
    offset: args?.offset as number | undefined,
  })
  return { content: [{ type: "text", text: JSON.stringify(subs) }] }
}

export async function handleGetSubscription(args?: Record<string, unknown>): Promise<McpResponse> {
  if (args?.id === undefined) return errorResponse("id is required")
  const id = Number(args.id)
  if (!Number.isInteger(id) || id < 1) return errorResponse("id must be a positive integer")
  const sub = getSubscription(id)
  return { content: [{ type: "text", text: JSON.stringify(sub ?? null) }] }
}

export async function handleSearchSubscriptions(args?: Record<string, unknown>): Promise<McpResponse> {
  if (!args?.query) return errorResponse("query is required")
  for (const key of ["names", "notes", "tags"] as const) {
    if (args[key] !== undefined && typeof args[key] !== "boolean") {
      return errorResponse(`${key} must be a boolean`)
    }
  }
  const results = searchSubscriptions(String(args.query), {
    names: args.names as boolean | undefined,
    notes: args.notes as boolean | undefined,
    tags: args.tags as boolean | undefined,
  })
  return { content: [{ type: "text", text: JSON.stringify(results) }] }
}

export async function handleAddSubscription(args?: Record<string, unknown>): Promise<McpResponse> {
  if (!args?.name || args?.price === undefined || !args?.currency || !args?.cycle) {
    return errorResponse("name, price, currency, and cycle are required")
  }

  const name = String(args.name)
  const nameError = validateName(name)
  if (nameError !== true) return errorResponse(`Invalid name: ${nameError}`)

  const priceError = validatePrice(String(args.price))
  if (priceError !== true) return errorResponse(`Invalid price: ${priceError}`)

  const currency = String(args.currency)
  if (!isValidCurrency(currency)) {
    return errorResponse(`Invalid currency "${currency}". Use a supported 3-letter ISO code (e.g. USD, JPY)`)
  }
  const cycle = String(args.cycle)
  if (!isValidCycle(cycle)) {
    return errorResponse(`Invalid cycle "${cycle}". Use: weekly, bi-weekly, monthly, quarterly, semi-annual, yearly`)
  }
  const status = (args.status as Status | undefined) ?? "active"
  if (!isValidStatus(status)) {
    return errorResponse(`Invalid status "${status}". Use: active, paused, cancelled, archived`)
  }

  const tags = args.tags
    ? String(args.tags).split(",").map((t: string) => t.trim()).filter(Boolean)
    : []
  if (tags.length > MAX_NEW_TAGS) return errorResponse(`Maximum ${MAX_NEW_TAGS} tags allowed`)
  const tagsError = validateTags(tags.join(","))
  if (tagsError !== true) return errorResponse(`Invalid tags: ${tagsError}`)

  if (args.billingDay !== undefined) {
    const billingDayError = validateBillingDay(String(args.billingDay))
    if (billingDayError !== true) return errorResponse(`Invalid billing day: ${billingDayError}`)
  }
  if (args.paymentMethod !== undefined) {
    const paymentMethodError = validatePaymentMethod(String(args.paymentMethod))
    if (paymentMethodError !== true) return errorResponse(`Invalid payment method: ${paymentMethodError}`)
  }
  if (args.notes !== undefined) {
    const notesError = validateNotes(String(args.notes))
    if (notesError !== true) return errorResponse(`Invalid notes: ${notesError}`)
  }

  const addArgs: AddSharedArgs = {
    name,
    price: Number(args.price),
    currency,
    cycle: cycle as Cycle,
    tags,
    status,
    billingDay: args.billingDay !== undefined ? Number(args.billingDay) : null,
    paymentMethod: args.paymentMethod as string | undefined,
    notes: args.notes as string | undefined,
  }
  const id = writeSubscription(addArgs)
  return { content: [{ type: "text", text: JSON.stringify({ id }) }] }
}

export async function handleDeleteSubscription(args?: Record<string, unknown>): Promise<McpResponse> {
  if (args?.id === undefined) return errorResponse("id is required")
  const id = Number(args.id)
  if (!Number.isInteger(id) || id < 1) return errorResponse("id must be a positive integer")
  const success = deleteSubscription(id)
  return { content: [{ type: "text", text: JSON.stringify({ success }) }] }
}

export async function handleGetSummary(_args?: Record<string, unknown>): Promise<McpResponse> {
  const subs = getNonCancelledSubscriptions()
  const summary = calcSummary(subs)
  return { content: [{ type: "text", text: JSON.stringify(summary) }] }
}

export async function handleGetUpcoming(args?: Record<string, unknown>): Promise<McpResponse> {
  const days = (args?.days as number | undefined) ?? 7
  const entries = calcUpcoming(days)
  return { content: [{ type: "text", text: JSON.stringify(entries) }] }
}

export async function handleGetCalendar(args?: Record<string, unknown>): Promise<McpResponse> {
  const now = new Date()
  const month = (args?.month as number | undefined) ?? now.getMonth() + 1
  const year = (args?.year as number | undefined) ?? now.getFullYear()
  const entries = calcCalendarEntries(month, year)
  return { content: [{ type: "text", text: JSON.stringify(entries) }] }
}

export async function handleExportData(args?: Record<string, unknown>): Promise<McpResponse> {
  const format = String(args?.format ?? "json")
  const subs = getSubscriptions()
  let output: string
  switch (format) {
    case "csv":
      output = exportCsv(subs)
      break
    case "json":
      output = exportJson(subs)
      break
    case "md":
      output = exportMd(subs)
      break
    default:
      return {
        content: [{ type: "text", text: `Unsupported format: ${format}. Supported formats: csv, json, md` }],
        isError: true,
      }
  }
  return { content: [{ type: "text", text: output }] }
}

export async function handleEditSubscription(args?: Record<string, unknown>): Promise<McpResponse> {
  if (args?.id === undefined) return errorResponse("id is required")
  const id = Number(args.id)
  if (!Number.isInteger(id) || id < 1) return errorResponse("id must be a positive integer")
  if (args.name !== undefined) {
    const nameError = validateName(String(args.name))
    if (nameError !== true) return errorResponse(`Invalid name: ${nameError}`)
  }
  if (args.price !== undefined) {
    const priceError = validatePrice(String(args.price))
    if (priceError !== true) return errorResponse(`Invalid price: ${priceError}`)
  }
  if (args.currency !== undefined && !isValidCurrency(String(args.currency))) {
    return errorResponse(`Invalid currency "${String(args.currency)}". Use a supported 3-letter ISO code (e.g. USD, JPY)`)
  }
  if (args.cycle !== undefined && !isValidCycle(String(args.cycle))) {
    return errorResponse(`Invalid cycle "${String(args.cycle)}". Use: weekly, bi-weekly, monthly, quarterly, semi-annual, yearly`)
  }
  if (args.status !== undefined && !isValidStatus(String(args.status))) {
    return errorResponse(`Invalid status "${String(args.status)}". Use: active, paused, cancelled, archived`)
  }
  if (args.billingDay !== undefined) {
    const billingDayError = validateBillingDay(String(args.billingDay))
    if (billingDayError !== true) return errorResponse(`Invalid billing day: ${billingDayError}`)
  }
  if (args.paymentMethod !== undefined) {
    const paymentMethodError = validatePaymentMethod(String(args.paymentMethod))
    if (paymentMethodError !== true) return errorResponse(`Invalid payment method: ${paymentMethodError}`)
  }
  if (args.notes !== undefined) {
    const notesError = validateNotes(String(args.notes))
    if (notesError !== true) return errorResponse(`Invalid notes: ${notesError}`)
  }

  const editFields: Partial<AddSharedArgs> = {}
  if (args.name !== undefined) editFields.name = String(args.name)
  if (args.price !== undefined) editFields.price = Number(args.price)
  if (args.currency !== undefined) editFields.currency = String(args.currency)
  if (args.cycle !== undefined) editFields.cycle = String(args.cycle) as Cycle
  if (args.status !== undefined) editFields.status = String(args.status) as Status
  if (args.billingDay !== undefined) editFields.billingDay = Number(args.billingDay)
  if (args.paymentMethod !== undefined) editFields.paymentMethod = String(args.paymentMethod)
  if (args.notes !== undefined) editFields.notes = String(args.notes)
  if (args.tags !== undefined) {
    const tags = String(args.tags).split(",").map((t: string) => t.trim()).filter(Boolean)
    if (tags.length > MAX_NEW_TAGS) return errorResponse(`Maximum ${MAX_NEW_TAGS} tags allowed`)
    const tagsError = validateTags(tags.join(","))
    if (tagsError !== true) return errorResponse(`Invalid tags: ${tagsError}`)
    editFields.tags = tags
  }
  const success = updateSubscription(id, editFields)
  if (!success) return errorResponse(`Subscription with id ${id} not found`)
  return { content: [{ type: "text", text: JSON.stringify({ success }) }] }
}

export async function handleGetHistory(args?: Record<string, unknown>): Promise<McpResponse> {
  const id = args?.id as number | undefined
  const days = args?.days as number | undefined
  let entries
  if (id !== undefined) {
    entries = getPriceHistory(id)
  } else {
    entries = getAllPriceChanges(days)
  }
  return { content: [{ type: "text", text: JSON.stringify(entries) }] }
}

export async function handleGetAnalytics(_args?: Record<string, unknown>): Promise<McpResponse> {
  const all = getSubscriptions({ includeArchived: true })
  const active = all.filter((s) => s.status !== "cancelled" && s.status !== "archived")
  const summary = calcSummary(active)
  const statusBreakdown = {
    active: all.filter((s) => s.status === "active").length,
    paused: all.filter((s) => s.status === "paused").length,
    cancelled: all.filter((s) => s.status === "cancelled").length,
    archived: all.filter((s) => s.status === "archived").length,
  }
  return {
    content: [{
      type: "text",
      text: JSON.stringify({
        ...summary,
        statusBreakdown,
      }),
    }],
  }
}

export async function handleGetForecast(args?: Record<string, unknown>): Promise<McpResponse> {
  const months = (args?.months as number | undefined) ?? 12
  const targetCurrency = args?.currency as string | undefined
  const cancelNames = args?.cancel
    ? String(args.cancel).split(",").map((n: string) => n.trim()).filter(Boolean)
    : []

  let rates: FxRates | null = null
  if (targetCurrency) {
    try { rates = await fetchFxRates() } catch { /* fall through */ }
  }

  const subs = getSubscriptions()
  const activeSubs = subs.filter(
    (s) => s.status !== "cancelled" && !cancelNames.includes(s.name),
  )

  const entries: { name: string; price: number; currency: string; cycle: string; monthly: number; monthlyConverted?: number }[] = []
  let rawMonthlyTotal = 0

  for (const sub of activeSubs) {
    const monthly = sub.price * periodFactor(sub.cycle, "monthly")
    let monthlyConverted: number | undefined
    let monthlyConvertedRaw: number | undefined
    if (targetCurrency && rates) {
      try {
        monthlyConvertedRaw = convertPrice(monthly, sub.currency, targetCurrency, rates.rates)
        monthlyConverted = roundCurrency(monthlyConvertedRaw)
      } catch { /* keep original */ }
    }
    entries.push({
      name: sub.name,
      price: sub.price,
      currency: sub.currency,
      cycle: sub.cycle,
      monthly: roundCurrency(monthly),
      ...(monthlyConverted !== undefined ? { monthlyConverted } : {}),
    })
    rawMonthlyTotal += monthlyConvertedRaw ?? monthly
  }

  const displayCcy = targetCurrency || "mixed"
  const monthlyTotal = rawMonthlyTotal

  const yearlyTotal = monthlyTotal * 12

  return {
    content: [{
      type: "text",
      text: JSON.stringify({
        months,
        currency: displayCcy,
        monthlyTotal: roundCurrency(monthlyTotal),
        yearlyTotal: roundCurrency(yearlyTotal),
        totalSubscriptions: entries.length,
        entries,
      }),
    }],
  }
}

export async function handleCompare(args?: Record<string, unknown>): Promise<McpResponse> {
  const periodValue = String(args?.period ?? "monthly")
  if (!isValidNamedCycle(periodValue)) return errorResponse(`Invalid period: ${periodValue}`)
  const period = periodValue as NamedCycle
  const targetCurrency = args?.currency as Currency | undefined

  let rates: FxRates | null = null
  if (targetCurrency) {
    try { rates = await fetchFxRates() } catch { /* fall through */ }
  }

  const subs = getSubscriptions()
  const activeSubs = subs.filter((s) => s.status !== "cancelled")

  const currentTotals = calcSubTotal(activeSubs, rates, targetCurrency, period)
  const previousTotals = calcPreviousTotals(activeSubs, rates, targetCurrency, period)

  const allCurrencies = [...new Set([...Object.keys(currentTotals), ...Object.keys(previousTotals)])].sort()

  const currencyRows = allCurrencies.map((ccy) => ({
    currency: ccy,
    current: roundCurrency(currentTotals[ccy] ?? 0),
    previous: roundCurrency(previousTotals[ccy] ?? 0),
  }))

  const grandCurrent = currencyRows.reduce((s, r) => s + r.current, 0)
  const grandPrevious = currencyRows.reduce((s, r) => s + r.previous, 0)

  return {
    content: [{
      type: "text",
      text: JSON.stringify({
        period,
        currency: targetCurrency || null,
        rows: currencyRows,
        grandTotal: {
          current: grandCurrent,
          previous: grandPrevious,
          change: grandCurrent - grandPrevious,
          changePercent: grandPrevious > 0
            ? Math.round(((grandCurrent - grandPrevious) / grandPrevious) * 10000) / 100
            : 0,
        },
      }),
    }],
  }
}

export async function handleBulkOperations(args?: Record<string, unknown>): Promise<McpResponse> {
  const action = String(args?.action ?? "")
  if (action === "delete" && args?.confirm !== true) {
    return errorResponse("Bulk delete requires confirm: true")
  }
  const filters: { tag?: string; status?: string; name?: string } = {}
  if (args?.filter_tag) filters.tag = String(args.filter_tag)
  if (args?.filter_status) filters.status = String(args.filter_status)
  if (args?.filter_name) filters.name = String(args.filter_name)

  const subs = getSubscriptions()
  let matched = subs

  if (filters.tag) {
    const tagSet = new Set(filters.tag.split(",").map((t: string) => t.trim()))
    matched = matched.filter((s) => s.tags?.some((t) => tagSet.has(t)))
  }
  if (filters.status) {
    matched = matched.filter((s) => s.status === filters.status)
  }
  if (filters.name) {
    matched = matched.filter((s) => s.name.toLowerCase().includes(filters.name!.toLowerCase()))
  }

  let affected = matched
  if (action === "status") {
    affected = matched.filter((s) => s.status !== "cancelled")
  }

  const affectedIds = affected.map((s) => s.id)
  if (affectedIds.length > MAX_BULK_AFFECTED) {
    return errorResponse(`Bulk operation would affect ${affectedIds.length} subscriptions (max ${MAX_BULK_AFFECTED})`)
  }
  let resultCount = 0
  const errors: string[] = []

  const reportError = (id: number, error: unknown) => {
    errors.push(`id ${id}: ${error instanceof Error ? error.message : String(error)}`)
  }

  switch (action) {
    case "status": {
      const targetStatus = String(args?.status ?? "active")
      if (!isValidStatus(targetStatus)) {
        return { content: [{ type: "text", text: `Invalid status "${targetStatus}". Use: active, paused, cancelled, archived` }], isError: true }
      }
      for (const id of affectedIds) {
        try {
          if (updateSubscription(id, { status: targetStatus as Status }, { persist: false })) resultCount++
        } catch (error) { reportError(id, error) }
      }
      break
    }
    case "delete": {
      for (const id of affectedIds) {
        try {
          if (deleteSubscription(id, { persist: false })) resultCount++
        } catch (error) { reportError(id, error) }
      }
      break
    }
    case "tag_add": {
      const tagName = String(args?.tag_name ?? "").trim()
      const tagError = validateTags(tagName)
      if (!tagName) return errorResponse("tag_name is required for tag_add action")
      if (tagName.includes(",")) return errorResponse("tag_name must be a single tag")
      if (tagError !== true) return errorResponse(`Invalid tag name: ${tagError}`)
      for (const s of affected) {
        const currentTags = s.tags ?? []
        if (!currentTags.includes(tagName)) {
          if (currentTags.length >= MAX_TAG_COUNT) {
            reportError(s.id, new Error(`Maximum ${MAX_TAG_COUNT} tags allowed`))
            continue
          }
          try {
            if (updateSubscription(s.id, { tags: [...currentTags, tagName] }, { persist: false })) resultCount++
          } catch (error) { reportError(s.id, error) }
        }
      }
      break
    }
    case "tag_remove": {
      const tagName = String(args?.tag_name ?? "").trim()
      const tagError = validateTags(tagName)
      if (!tagName) return errorResponse("tag_name is required for tag_remove action")
      if (tagName.includes(",")) return errorResponse("tag_name must be a single tag")
      if (tagError !== true) return errorResponse(`Invalid tag name: ${tagError}`)
      for (const s of affected) {
        const currentTags = s.tags ?? []
        if (currentTags.includes(tagName)) {
          try {
            if (updateSubscription(s.id, { tags: currentTags.filter((t) => t !== tagName) }, { persist: false })) resultCount++
          } catch (error) { reportError(s.id, error) }
        }
      }
      break
    }
    default:
      return { content: [{ type: "text", text: `Unknown bulk action: ${action}. Use: status, delete, tag_add, tag_remove` }], isError: true }
  }

  if (resultCount > 0) saveDb()
  return {
    content: [{
      type: "text",
      text: JSON.stringify({
        action,
        filters,
        matchedCount: affected.length,
        affectedCount: resultCount,
        affectedIds,
        errors,
      }),
    }],
  }
}

export async function handleGetTrials(args?: Record<string, unknown>): Promise<McpResponse> {
  const expiringSoon = args?.expiring_soon as number | undefined
  let entries
  if (expiringSoon !== undefined) {
    entries = getTrialsExpiringSoon(expiringSoon)
  } else {
    entries = getTrials()
  }
  return { content: [{ type: "text", text: JSON.stringify(entries) }] }
}

export async function handleListTags(_args?: Record<string, unknown>): Promise<McpResponse> {
  const tags = getTagsWithCount()
  return { content: [{ type: "text", text: JSON.stringify(tags) }] }
}

export async function handleGetTagSubscriptions(args?: Record<string, unknown>): Promise<McpResponse> {
  if (!args?.tag) {
    return { content: [{ type: "text", text: "tag is required" }], isError: true }
  }
  const names = String(args.tag).split(",").map((t: string) => t.trim()).filter(Boolean)
  if (names.length === 0) {
    return { content: [{ type: "text", text: "tag is required" }], isError: true }
  }
  const subs = tagsSubscription(names)
  return { content: [{ type: "text", text: JSON.stringify(subs) }] }
}

export async function handleGetUsageTotal(args?: Record<string, unknown>): Promise<McpResponse> {
  let from: string
  let to: string
  if (args?.from && args?.to) {
    from = String(args.from)
    to = String(args.to)
  } else {
    const range = getPeriodDateRange("monthly")
    from = range.from
    to = range.to
  }
  const total = getLlmUsageTotal(from, to)
  const tokens = getLlmUsageTokenTotal(from, to)
  const byProvider = getLlmUsageTotalByProvider(from, to)
  const byModel = getLlmUsageTotalByModel(from, to)
  return {
    content: [{
      type: "text",
      text: JSON.stringify({ from, to, total, tokens, byProvider, byModel }),
    }],
  }
}

export async function handleListUsage(args?: Record<string, unknown>): Promise<McpResponse> {
  const entries = getLlmUsage({
    provider: args?.provider as string | undefined,
    from: args?.from as string | undefined,
    to: args?.to as string | undefined,
    limit: (args?.limit as number | undefined) ?? 100,
    minCost: 0,
  })
  return { content: [{ type: "text", text: JSON.stringify(entries) }] }
}

/** Map of tool name to handler function. */
export const HANDLER_MAP: Record<string, (args?: Record<string, unknown>) => Promise<McpResponse>> = {
  list_subscriptions: handleListSubscriptions,
  get_subscription: handleGetSubscription,
  search_subscriptions: handleSearchSubscriptions,
  add_subscription: handleAddSubscription,
  delete_subscription: handleDeleteSubscription,
  get_summary: handleGetSummary,
  get_upcoming: handleGetUpcoming,
  get_calendar: handleGetCalendar,
  export_data: handleExportData,
  edit_subscription: handleEditSubscription,
  get_history: handleGetHistory,
  get_analytics: handleGetAnalytics,
  get_forecast: handleGetForecast,
  compare: handleCompare,
  bulk_operations: handleBulkOperations,
  get_trials: handleGetTrials,
  list_tags: handleListTags,
  get_tag_subscriptions: handleGetTagSubscriptions,
  get_usage_total: handleGetUsageTotal,
  list_usage: handleListUsage,
}

/**
 * Guarded tool dispatch: rate limit → size/schema validation → handler
 * lookup → execution, mapping every rejection to a well-formed McpResponse.
 */
export async function callTool(name: string, args?: Record<string, unknown>): Promise<McpResponse> {
  if (!rateLimiter.tryConsume()) {
    return {
      content: [{ type: "text", text: "Rate limit exceeded. Please slow down." }],
      isError: true,
    }
  }
  const validationError = validateToolCall(name, args)
  if (validationError) {
    return { content: [{ type: "text", text: validationError }], isError: true }
  }
  const handler = HANDLER_MAP[name]
  if (!handler) {
    return {
      content: [{ type: "text", text: `Unknown tool: ${name}` }],
      isError: true,
    }
  }
  try {
    return await handler(args)
  } catch (error) {
    return {
      content: [{ type: "text", text: error instanceof Error ? error.message : String(error) }],
      isError: true,
    }
  }
}
