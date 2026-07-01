import { Server } from "@modelcontextprotocol/sdk/server/index.js"
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js"
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js"
import {
  getSubscriptions,
  getSubscription,
  writeSubscription,
  deleteSubscription,
  updateSubscription,
  getDb,
  mapTags,
  getPriceHistory,
  getAllPriceChanges,
  getTrials,
  getTrialsExpiringSoon,
} from "./db.ts"
import { calcSummary, getPeriodDateRange, getPreviousPeriodDateRange } from "./payment.ts"
import { calcCalendarEntries } from "./calendar.ts"
import { exportCsv, exportJson, exportMd } from "./export.ts"
import { fetchFxRates, convertPrice } from "./fx.ts"
import { periodFactor } from "./types.ts"

import type { SharedArgs, AddSharedArgs, Cycle, Status, Currency } from "./types.ts"
import type { FxRates } from "./fx.ts"
import type { SqlValue } from "sql.js"

// ── Date helpers ────────────────────────────────────────

export function formatDateISO(d: Date): string {
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, "0")
  const day = String(d.getDate()).padStart(2, "0")
  return `${y}-${m}-${day}`
}

function getBillingDay(sub: SharedArgs): number {
  if (sub.billingDay) return sub.billingDay
  const [y, m, d] = sub.createdAt.split("-").map(Number)
  return new Date(y, m - 1, d).getDate()
}

function addMonths(date: Date, n: number): Date {
  const result = new Date(date)
  result.setMonth(result.getMonth() + n)
  return result
}

export function nextDateForCycle(
  anchorDay: number,
  anchorDate: Date,
  cycle: Cycle,
  fromDate: Date,
): Date {
  switch (cycle) {
    case "monthly": {
      const candidate = new Date(fromDate.getFullYear(), fromDate.getMonth(), anchorDay)
      if (candidate > fromDate) return candidate
      return new Date(fromDate.getFullYear(), fromDate.getMonth() + 1, anchorDay)
    }
    case "yearly": {
      const candidate = new Date(fromDate.getFullYear(), anchorDate.getMonth(), anchorDay)
      if (candidate > fromDate) return candidate
      return new Date(fromDate.getFullYear() + 1, anchorDate.getMonth(), anchorDay)
    }
    case "weekly": {
      const diff = fromDate.getTime() - anchorDate.getTime()
      const weeksSince = Math.ceil(diff / (7 * 24 * 60 * 60 * 1000))
      return new Date(anchorDate.getTime() + weeksSince * 7 * 24 * 60 * 60 * 1000)
    }
    case "bi-weekly": {
      const diff = fromDate.getTime() - anchorDate.getTime()
      const periodsSince = Math.ceil(diff / (14 * 24 * 60 * 60 * 1000))
      return new Date(anchorDate.getTime() + periodsSince * 14 * 24 * 60 * 60 * 1000)
    }
    case "quarterly": {
      const monthsSince =
        (fromDate.getFullYear() - anchorDate.getFullYear()) * 12 +
        (fromDate.getMonth() - anchorDate.getMonth())
      const quartersSince = Math.ceil(monthsSince / 3)
      return addMonths(new Date(anchorDate), quartersSince * 3)
    }
    case "semi-annual": {
      const monthsSince =
        (fromDate.getFullYear() - anchorDate.getFullYear()) * 12 +
        (fromDate.getMonth() - anchorDate.getMonth())
      const halvesSince = Math.ceil(monthsSince / 6)
      return addMonths(new Date(anchorDate), halvesSince * 6)
    }
  }
}

function calculateNextBilling(sub: SharedArgs, fromDate: Date): Date {
  const [y, m, d] = sub.createdAt.split("-").map(Number)
  const anchorDate = new Date(y, m - 1, d)
  const day = getBillingDay(sub)

  if (
    sub.cycle === "monthly" ||
    sub.cycle === "yearly" ||
    sub.cycle === "quarterly" ||
    sub.cycle === "semi-annual"
  ) {
    const candidate = nextDateForCycle(day, anchorDate, sub.cycle, fromDate)
    if (candidate.getDate() !== day) {
      candidate.setDate(0)
    }
    return candidate
  }

  return nextDateForCycle(day, anchorDate, sub.cycle, fromDate)
}

// ── Upcoming calculation ────────────────────────────────

export function calcUpcoming(days: number) {
  const list = getSubscriptions().filter((s) => s.status !== "cancelled")
  const now = new Date()
  now.setHours(0, 0, 0, 0)
  const endDate = new Date(now)
  endDate.setDate(endDate.getDate() + days)

  type UpcomingEntry = { sub: SharedArgs; nextDate: Date; amount: number }
  const entries: UpcomingEntry[] = []

  for (const sub of list) {
    const next = calculateNextBilling(sub, now)
    if (next >= now && next <= endDate) {
      const amount = sub.price
      entries.push({ sub, nextDate: next, amount })
    }
  }

  entries.sort((a, b) => a.nextDate.getTime() - b.nextDate.getTime())

  return entries.map((e) => ({
    id: e.sub.id,
    name: e.sub.name,
    price: e.sub.price,
    currency: e.sub.currency,
    cycle: e.sub.cycle,
    amount: Math.round(e.amount),
    nextDate: formatDateISO(e.nextDate),
    tags: e.sub.tags,
  }))
}

// ── Search implementation ────────────────────────────────

export function searchSubscriptions(
  query: string,
  fields: { names?: boolean; notes?: boolean; tags?: boolean },
): SharedArgs[] {
  const db = getDb()
  const pattern = `%${query}%`
  const conditions: string[] = []
  const params: SqlValue[] = []

  const searchFields = {
    names: fields.names ?? (!fields.notes && !fields.tags),
    notes: fields.notes ?? (!fields.names && !fields.tags),
    tags: fields.tags ?? (!fields.names && !fields.notes),
  }

  if (searchFields.names) {
    conditions.push("s.name LIKE ?")
    params.push(pattern)
  }
  if (searchFields.notes) {
    conditions.push("s.notes LIKE ?")
    params.push(pattern)
  }
  if (searchFields.tags) {
    conditions.push(
      "s.id IN (SELECT st.subscription_id FROM subscription_tags st JOIN tags t ON t.id = st.tag_id WHERE t.name LIKE ?)",
    )
    params.push(pattern)
  }

  const whereClause = conditions.length > 0 ? `WHERE ${conditions.join(" OR ")}` : ""
  const sql = `SELECT DISTINCT s.id, s.name, s.price, s.currency, s.cycle, s.status, s.billing_day AS billingDay, s.created_at AS createdAt, s.notes, s.payment_method AS paymentMethod FROM subscriptions s ${whereClause} ORDER BY s.name`

  const results = db.exec(sql, params)
  if (!results.length) return []

  const { columns, values } = results[0]
  const subs: SharedArgs[] = values.map((row) => {
    const obj: Record<string, unknown> = {}
    for (let i = 0; i < columns.length; i++) {
      obj[columns[i]] = row[i]
    }
    return {
      id: Number(obj.id),
      name: String(obj.name),
      price: Number(obj.price),
      currency: String(obj.currency),
      cycle: String(obj.cycle) as Cycle,
      status: String(obj.status) as Status,
      billingDay: obj.billingDay !== null ? Number(obj.billingDay) : null,
      createdAt: String(obj.createdAt),
      notes: obj.notes !== null ? String(obj.notes) : null,
      paymentMethod: obj.paymentMethod !== null ? String(obj.paymentMethod) : null,
      tags: [],
    } as SharedArgs
  })

  return mapTags(subs)
}

// ── Compare helpers ──────────────────────────────────────

type CcyTotals = Record<string, number>

function calcSubTotalHelper(
  subs: SharedArgs[],
  rates: FxRates | null,
  targetCurrency: Currency | undefined,
): CcyTotals {
  const totals: CcyTotals = {}
  for (const sub of subs) {
    if (sub.status === "cancelled") continue
    const monthly = sub.price * periodFactor(sub.cycle, "monthly")
    if (targetCurrency && rates) {
      try {
        const converted = convertPrice(monthly, sub.currency, targetCurrency, rates.rates)
        totals[targetCurrency] = (totals[targetCurrency] ?? 0) + converted
      } catch {
        totals[sub.currency] = (totals[sub.currency] ?? 0) + monthly
      }
    } else {
      totals[sub.currency] = (totals[sub.currency] ?? 0) + monthly
    }
  }
  return totals
}

function calcPreviousTotals(
  activeSubs: SharedArgs[],
  rates: FxRates | null,
  targetCurrency: Currency | undefined,
): CcyTotals {
  // Check price history to see if any subscriptions had different prices before
  const priceChanges = getAllPriceChanges()
  const priceBefore: Record<number, { price: number; currency: string }> = {}

  for (const change of priceChanges) {
    // The previous price is the old price before the change
    if (change.oldPrice !== null && !priceBefore[change.subscriptionId]) {
      priceBefore[change.subscriptionId] = {
        price: change.oldPrice,
        currency: change.oldCurrency ?? change.newCurrency,
      }
    }
  }

  const totals: CcyTotals = {}
  for (const sub of activeSubs) {
    if (sub.status === "cancelled") continue
    const prev = priceBefore[sub.id]
    const price = prev?.price ?? sub.price
    const currency = prev?.currency ?? sub.currency
    const monthly = price * periodFactor(sub.cycle, "monthly")

    if (targetCurrency && rates) {
      try {
        const converted = convertPrice(monthly, currency, targetCurrency, rates.rates)
        totals[targetCurrency] = (totals[targetCurrency] ?? 0) + converted
      } catch {
        totals[currency] = (totals[currency] ?? 0) + monthly
      }
    } else {
      totals[currency] = (totals[currency] ?? 0) + monthly
    }
  }
  return totals
}

// ── MCP Server ───────────────────────────────────────────

const server = new Server(
  { name: "subtrack-mcp", version: "1.0.0" },
  { capabilities: { tools: {} } },
)

server.setRequestHandler(ListToolsRequestSchema, async () => {
  return {
    tools: [
      {
        name: "list_subscriptions",
        description: "List all subscriptions",
        inputSchema: {
          type: "object",
          properties: {
            sort: {
              type: "string",
              description: "Sort field: name, price, currency, cycle, status",
            },
            desc: { type: "boolean", description: "Sort descending" },
          },
        },
      },
      {
        name: "get_subscription",
        description: "Get subscription by ID",
        inputSchema: {
          type: "object",
          properties: {
            id: { type: "number", description: "Subscription ID" },
          },
          required: ["id"],
        },
      },
      {
        name: "search_subscriptions",
        description: "Search subscriptions by name, notes, or tags",
        inputSchema: {
          type: "object",
          properties: {
            query: { type: "string", description: "Search query" },
            names: { type: "boolean", description: "Search in names" },
            notes: { type: "boolean", description: "Search in notes" },
            tags: { type: "boolean", description: "Search in tags" },
          },
          required: ["query"],
        },
      },
      {
        name: "add_subscription",
        description: "Add a new subscription",
        inputSchema: {
          type: "object",
          properties: {
            name: { type: "string", description: "Subscription name" },
            price: { type: "number", description: "Price in smallest currency unit" },
            currency: { type: "string", description: "Currency code (e.g. USD, JPY)" },
            cycle: {
              type: "string",
              description: "Billing cycle: weekly, bi-weekly, monthly, quarterly, semi-annual, yearly",
            },
            tags: { type: "string", description: "Comma-separated tags" },
            billingDay: { type: "number", description: "Billing day of month (1-31)" },
            status: { type: "string", description: "Status: active, paused, cancelled" },
            paymentMethod: { type: "string", description: "Payment method" },
            notes: { type: "string", description: "Notes" },
          },
          required: ["name", "price", "currency", "cycle"],
        },
      },
      {
        name: "delete_subscription",
        description: "Delete subscription by ID",
        inputSchema: {
          type: "object",
          properties: {
            id: { type: "number", description: "Subscription ID" },
          },
          required: ["id"],
        },
      },
      {
        name: "get_summary",
        description: "Get subscription summary statistics",
        inputSchema: {
          type: "object",
          properties: {},
        },
      },
      {
        name: "get_upcoming",
        description: "Get upcoming bills within a number of days",
        inputSchema: {
          type: "object",
          properties: {
            days: { type: "number", description: "Number of days (default: 7)" },
          },
        },
      },
      {
        name: "get_calendar",
        description: "Get calendar entries for a month",
        inputSchema: {
          type: "object",
          properties: {
            month: { type: "number", description: "Month (1-12)" },
            year: { type: "number", description: "Year" },
          },
        },
      },
      {
        name: "export_data",
        description: "Export subscriptions in a format",
        inputSchema: {
          type: "object",
          properties: {
            format: {
              type: "string",
              description: "Export format: csv, json, md",
            },
          },
          required: ["format"],
        },
      },
      {
        name: "edit_subscription",
        description: "Edit an existing subscription",
        inputSchema: {
          type: "object",
          properties: {
            id: { type: "number", description: "Subscription ID to edit" },
            name: { type: "string", description: "New name" },
            price: { type: "number", description: "New price in smallest currency unit" },
            currency: { type: "string", description: "New currency code" },
            cycle: { type: "string", description: "New billing cycle" },
            status: { type: "string", description: "New status: active, paused, cancelled" },
            billingDay: { type: "number", description: "New billing day of month (1-31)" },
            tags: { type: "string", description: "Comma-separated tags (replaces all)" },
            paymentMethod: { type: "string", description: "New payment method" },
            notes: { type: "string", description: "New notes" },
          },
          required: ["id"],
        },
      },
      {
        name: "get_history",
        description: "Get price change history for subscriptions",
        inputSchema: {
          type: "object",
          properties: {
            id: { type: "number", description: "Filter by subscription ID" },
            days: { type: "number", description: "Recent days to include (default: all)" },
          },
        },
      },
      {
        name: "get_analytics",
        description: "Get subscription analytics and statistics",
        inputSchema: {
          type: "object",
          properties: {},
        },
      },
      {
        name: "get_forecast",
        description: "Get spending forecast for upcoming months",
        inputSchema: {
          type: "object",
          properties: {
            months: { type: "number", description: "Number of months to forecast (default: 12)" },
            currency: { type: "string", description: "Convert all to target currency" },
            cancel: {
              type: "string",
              description: "Comma-separated subscription names to exclude from forecast",
            },
          },
        },
      },
      {
        name: "compare",
        description: "Compare subscription spending between current and previous period",
        inputSchema: {
          type: "object",
          properties: {
            period: {
              type: "string",
              description: "Period to compare: monthly (default), yearly, quarterly",
            },
            currency: { type: "string", description: "Convert all to target currency" },
          },
        },
      },
      {
        name: "bulk_operations",
        description: "Perform bulk operations on subscriptions matching filters",
        inputSchema: {
          type: "object",
          properties: {
            action: {
              type: "string",
              description: "Action: status, delete, tag_add, tag_remove",
            },
            status: {
              type: "string",
              description: "Target status for 'status' action: active, paused, cancelled",
            },
            tag_name: {
              type: "string",
              description: "Tag name for tag_add / tag_remove actions",
            },
            filter_tag: { type: "string", description: "Only affect subscriptions with this tag" },
            filter_status: { type: "string", description: "Only affect subscriptions with this status" },
            filter_name: { type: "string", description: "Only affect subscriptions whose name contains this" },
          },
          required: ["action"],
        },
      },
      {
        name: "get_trials",
        description: "Get trial periods",
        inputSchema: {
          type: "object",
          properties: {
            expiring_soon: {
              type: "number",
              description: "Filter trials expiring within N days",
            },
          },
        },
      },
    ],
  }
})

server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name, arguments: args } = request.params

  try {
    switch (name) {
      case "list_subscriptions": {
        const subs = getSubscriptions(
          args?.sort as string | undefined,
          args?.desc as boolean | undefined,
        )
        return { content: [{ type: "text", text: JSON.stringify(subs) }] }
      }

      case "get_subscription": {
        if (args?.id === undefined) {
          return {
            content: [{ type: "text", text: "id is required" }],
            isError: true,
          }
        }
        const sub = getSubscription(Number(args.id))
        return { content: [{ type: "text", text: JSON.stringify(sub ?? null) }] }
      }

      case "search_subscriptions": {
        if (!args?.query) {
          return {
            content: [{ type: "text", text: "query is required" }],
            isError: true,
          }
        }
        const results = searchSubscriptions(String(args.query), {
          names: args.names as boolean | undefined,
          notes: args.notes as boolean | undefined,
          tags: args.tags as boolean | undefined,
        })
        return { content: [{ type: "text", text: JSON.stringify(results) }] }
      }

      case "add_subscription": {
        if (!args?.name || args?.price === undefined || !args?.currency || !args?.cycle) {
          return {
            content: [{ type: "text", text: "name, price, currency, and cycle are required" }],
            isError: true,
          }
        }
        const tags = args.tags
          ? String(args.tags).split(",").map((t: string) => t.trim()).filter(Boolean)
          : []
        const addArgs: AddSharedArgs = {
          name: String(args.name),
          price: Number(args.price),
          currency: String(args.currency),
          cycle: String(args.cycle) as Cycle,
          tags,
          status: (args.status as Status | undefined) ?? "active",
          billingDay: args.billingDay !== undefined ? Number(args.billingDay) : null,
          paymentMethod: args.paymentMethod as string | undefined,
          notes: args.notes as string | undefined,
        }
        writeSubscription(addArgs)
        const db = getDb()
        const row = db.exec("SELECT last_insert_rowid() AS id")
        const id = row.length > 0 ? Number(row[0].values[0][0]) : 0
        return { content: [{ type: "text", text: JSON.stringify({ id }) }] }
      }

      case "delete_subscription": {
        if (args?.id === undefined) {
          return {
            content: [{ type: "text", text: "id is required" }],
            isError: true,
          }
        }
        const success = deleteSubscription(Number(args.id))
        return { content: [{ type: "text", text: JSON.stringify({ success }) }] }
      }

      case "get_summary": {
        const subs = getSubscriptions()
        const summary = calcSummary(subs)
        return { content: [{ type: "text", text: JSON.stringify(summary) }] }
      }

      case "get_upcoming": {
        const days = (args?.days as number | undefined) ?? 7
        const entries = calcUpcoming(days)
        return { content: [{ type: "text", text: JSON.stringify(entries) }] }
      }

      case "get_calendar": {
        const now = new Date()
        const month = (args?.month as number | undefined) ?? now.getMonth() + 1
        const year = (args?.year as number | undefined) ?? now.getFullYear()
        const entries = calcCalendarEntries(month, year)
        return { content: [{ type: "text", text: JSON.stringify(entries) }] }
      }

      case "export_data": {
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
              content: [
                {
                  type: "text",
                  text: `Unsupported format: ${format}. Supported formats: csv, json, md`,
                },
              ],
              isError: true,
            }
        }
        return { content: [{ type: "text", text: output }] }
      }

      case "edit_subscription": {
        if (args?.id === undefined) {
          return {
            content: [{ type: "text", text: "id is required" }],
            isError: true,
          }
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
          editFields.tags = String(args.tags).split(",").map((t: string) => t.trim()).filter(Boolean)
        }
        const success = updateSubscription(Number(args.id), editFields)
        return { content: [{ type: "text", text: JSON.stringify({ success }) }] }
      }

      case "get_history": {
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

      case "get_analytics": {
        const subs = getSubscriptions()
        const summary = calcSummary(subs)
        return { content: [{ type: "text", text: JSON.stringify(summary) }] }
      }

      case "get_forecast": {
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

        // Calculate monthly forecast per subscription
        const entries: { name: string; price: number; currency: string; cycle: string; monthly: number; monthlyConverted?: number }[] = []

        for (const sub of activeSubs) {
          const monthly = sub.price * periodFactor(sub.cycle, "monthly")
          let monthlyConverted: number | undefined
          if (targetCurrency && rates) {
            try {
              monthlyConverted = Math.round(convertPrice(monthly, sub.currency, targetCurrency, rates.rates))
            } catch { /* keep original */ }
          }
          entries.push({
            name: sub.name,
            price: sub.price,
            currency: sub.currency,
            cycle: sub.cycle,
            monthly,
            ...(monthlyConverted !== undefined ? { monthlyConverted } : {}),
          })
        }

        const displayCcy = targetCurrency || "mixed"
        const monthlyTotal = targetCurrency && rates
          ? entries.reduce((sum: number, e) => sum + (e.monthlyConverted ?? e.monthly), 0)
          : entries.reduce((sum: number, e) => sum + e.monthly, 0)

        const years = Math.ceil(months / 12)
        const yearlyTotal = monthlyTotal * 12

        return {
          content: [
            {
              type: "text",
              text: JSON.stringify({
                months,
                currency: displayCcy,
                monthlyTotal: Math.round(monthlyTotal),
                yearlyTotal: Math.round(yearlyTotal),
                totalSubscriptions: entries.length,
                entries,
              }),
            },
          ],
        }
      }

      case "compare": {
        const period = (args?.period as Cycle | undefined) ?? "monthly"
        const targetCurrency = args?.currency as Currency | undefined

        let rates: FxRates | null = null
        if (targetCurrency) {
          try { rates = await fetchFxRates() } catch { /* fall through */ }
        }

        const subs = getSubscriptions()
        const activeSubs = subs.filter((s) => s.status !== "cancelled")

        // Current period uses current prices
        const currentTotals = calcSubTotalHelper(activeSubs, rates, targetCurrency)
        // Previous period estimates from price history
        const previousTotals = calcPreviousTotals(activeSubs, rates, targetCurrency)

        const allCurrencies = [...new Set([...Object.keys(currentTotals), ...Object.keys(previousTotals)])].sort()

        const currencyRows = allCurrencies.map((ccy) => ({
          currency: ccy,
          current: Math.round(currentTotals[ccy] ?? 0),
          previous: Math.round(previousTotals[ccy] ?? 0),
        }))

        const grandCurrent = currencyRows.reduce((s, r) => s + r.current, 0)
        const grandPrevious = currencyRows.reduce((s, r) => s + r.previous, 0)

        return {
          content: [
            {
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
            },
          ],
        }
      }

      case "bulk_operations": {
        const action = String(args?.action ?? "")
        const filters: { tag?: string; status?: string; name?: string } = {}
        if (args?.filter_tag) filters.tag = String(args.filter_tag)
        if (args?.filter_status) filters.status = String(args.filter_status)
        if (args?.filter_name) filters.name = String(args.filter_name)

        const subs = getSubscriptions()
        let matched = subs

        // Apply filters
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

        // Filter out cancelled for status actions
        let affected = matched
        if (action === "status") {
          affected = matched.filter((s) => s.status !== "cancelled")
        }

        const affectedIds = affected.map((s) => s.id)
        let resultCount = 0

        switch (action) {
          case "status": {
            const targetStatus = String(args?.status ?? "active")
            for (const id of affectedIds) {
              try {
                updateSubscription(id, { status: targetStatus as Status })
                resultCount++
              } catch { /* skip */ }
            }
            break
          }
          case "delete": {
            for (const id of affectedIds) {
              try {
                deleteSubscription(id)
                resultCount++
              } catch { /* skip */ }
            }
            break
          }
          case "tag_add": {
            const tagName = String(args?.tag_name ?? "")
            if (!tagName) {
              return {
                content: [{ type: "text", text: "tag_name is required for tag_add action" }],
                isError: true,
              }
            }
            for (const s of affected) {
              const currentTags = s.tags ?? []
              if (!currentTags.includes(tagName)) {
                try {
                  updateSubscription(s.id, { tags: [...currentTags, tagName] })
                  resultCount++
                } catch { /* skip */ }
              }
            }
            break
          }
          case "tag_remove": {
            const tagName = String(args?.tag_name ?? "")
            if (!tagName) {
              return {
                content: [{ type: "text", text: "tag_name is required for tag_remove action" }],
                isError: true,
              }
            }
            for (const s of affected) {
              const currentTags = s.tags ?? []
              if (currentTags.includes(tagName)) {
                try {
                  updateSubscription(s.id, { tags: currentTags.filter((t) => t !== tagName) })
                  resultCount++
                } catch { /* skip */ }
              }
            }
            break
          }
          default:
            return {
              content: [{ type: "text", text: `Unknown bulk action: ${action}. Use: status, delete, tag_add, tag_remove` }],
              isError: true,
            }
        }

        return {
          content: [
            {
              type: "text",
              text: JSON.stringify({
                action,
                filters,
                matchedCount: affected.length,
                affectedCount: resultCount,
                affectedIds,
              }),
            },
          ],
        }
      }

      case "get_trials": {
        const expiringSoon = args?.expiring_soon as number | undefined
        let entries
        if (expiringSoon !== undefined) {
          entries = getTrialsExpiringSoon(expiringSoon)
        } else {
          entries = getTrials()
        }
        return { content: [{ type: "text", text: JSON.stringify(entries) }] }
      }

      default:
        return {
          content: [{ type: "text", text: `Unknown tool: ${name}` }],
          isError: true,
        }
    }
  } catch (error) {
    return {
      content: [
        {
          type: "text",
          text: error instanceof Error ? error.message : String(error),
        },
      ],
      isError: true,
    }
  }
})

export async function startMcpServer(): Promise<void> {
  const transport = new StdioServerTransport()
  await server.connect(transport)
}
