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
  getDb,
  mapTags,
} from "./db.ts"
import { calcSummary } from "./payment.ts"
import { calcCalendarEntries } from "./calendar.ts"
import { exportCsv, exportJson, exportMd } from "./export.ts"

import type { SharedArgs, AddSharedArgs, Cycle, Status } from "./types.ts"
import type { SqlValue } from "sql.js"

// ── Date helpers ────────────────────────────────────────

function formatDateISO(d: Date): string {
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

function nextDateForCycle(
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

function calcUpcoming(days: number) {
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

function searchSubscriptions(
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
