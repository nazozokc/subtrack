import { consola } from "@subtrack/lib/logger"
import pc from "@subtrack/lib/ansi"
import { CliTable3 } from "@subtrack/lib/table"
import { formatShortDate, daysUntil, formatCycle } from "@subtrack/lib/date"
import type { SharedArgs, Currency, LlmUsageEntry } from "./types.ts"
import { calculateNextBilling } from "./domain/billing.ts"

import { formatPrice, formatUsdCost } from "./price.ts"
import {
  TABLE_CHARS,
  getTableStyle,
  statusColor,
  zebraRow,
  sectionTitle,
  calcColumnWidths,
} from "./display-constants.ts"
import type { ColumnConfig } from "./display-constants.ts"

/**
 * Next billing date cell for the list table.
 * Cancelled/archived subs have no upcoming bill ("-").
 * Urgency coloring: today = cyan bold, ≤3 days = red, ≤7 days = yellow, else dim.
 */
function nextBillingCell(sub: SharedArgs): string {
  if (sub.status === "cancelled" || sub.status === "archived") return "-"
  const next = calculateNextBilling(sub, new Date())
  const days = daysUntil(next)
  if (days <= 0) return pc.bold(pc.cyan("today"))
  const label = formatShortDate(next)
  if (days <= 3) return pc.red(label)
  if (days <= 7) return pc.yellow(label)
  return pc.dim(label)
}

function buildRow(sub: SharedArgs, price: string, showNotes: boolean, showMethod: boolean, showContract?: boolean, showVendor?: boolean): string[] {
  const row = [
    String(sub.name),
    statusColor(sub.status),
    formatCycle(sub.cycle),
    nextBillingCell(sub),
    sub.tags.length > 0 ? sub.tags.join(", ") : "-",
    price,
  ]

  // Insert columns before price (last column), shifting as needed
  let insertIdx = row.length - 1

  if (showMethod) {
    const method = sub.paymentMethod ?? ""
    row.splice(insertIdx++, 0, method.length > 20 ? method.slice(0, 17) + "..." : method)
  }

  if (showContract) {
    const period = sub.contractStart
      ? `${sub.contractStart}${sub.contractEnd ? ` ~ ${sub.contractEnd}` : " ~ ongoing"}`
      : "-"
    const renewal = sub.autoRenewal ? "auto" : "manual"
    row.splice(insertIdx++, 0, period.length > 16 ? period.slice(0, 13) + "..." : period)
    row.splice(insertIdx++, 0, renewal)
  }

  if (showVendor) {
    const vendor = sub.vendorName ?? "-"
    const tier = sub.planTier ?? "-"
    row.splice(insertIdx++, 0, vendor.length > 15 ? vendor.slice(0, 12) + "..." : vendor)
    row.splice(insertIdx++, 0, tier.length > 12 ? tier.slice(0, 9) + "..." : tier)
  }

  // Insert notes column before the last column
  if (showNotes) {
    const insertAt = row.length - 1
    const notes = sub.notes ?? ""
    row.splice(insertAt, 0, notes.length > 40 ? notes.slice(0, 37) + "..." : notes)
  }

  return row
}

const BASE_COLS: ColumnConfig = {
  headers: ["name", "status", "cycle", "next", "tags", "price"] as const,
  minWidths: [10, 8, 6, 8, 8, 8] as const,
  maxWidths: [40, 12, 20, 10, 60, 20] as const,
}

const NOTES_COLS: ColumnConfig = {
  headers: ["name", "status", "cycle", "next", "tags", "notes", "price"] as const,
  minWidths: [10, 8, 6, 8, 8, 15, 8] as const,
  maxWidths: [40, 12, 20, 10, 60, 50, 20] as const,
}

const METHOD_COLS: ColumnConfig = {
  headers: ["name", "status", "cycle", "next", "tags", "method", "price"] as const,
  minWidths: [10, 8, 6, 8, 8, 10, 8] as const,
  maxWidths: [40, 12, 20, 10, 60, 30, 20] as const,
}

const ALL_COLS: ColumnConfig = {
  headers: ["name", "status", "cycle", "next", "tags", "method", "notes", "price"] as const,
  minWidths: [10, 8, 6, 8, 8, 10, 15, 8] as const,
  maxWidths: [40, 12, 20, 10, 60, 30, 50, 20] as const,
}

const CONTRACT_COLS: ColumnConfig = {
  headers: ["name", "status", "cycle", "next", "tags", "contract", "renewal", "price"] as const,
  minWidths: [10, 8, 6, 8, 8, 18, 6, 8] as const,
  maxWidths: [40, 12, 20, 10, 60, 30, 10, 20] as const,
}

const VENDOR_COLS: ColumnConfig = {
  headers: ["name", "status", "cycle", "next", "tags", "vendor", "plan", "price"] as const,
  minWidths: [10, 8, 6, 8, 8, 10, 8, 8] as const,
  maxWidths: [40, 12, 20, 10, 60, 30, 20, 20] as const,
}

const CONTRACT_VENDOR_COLS: ColumnConfig = {
  headers: ["name", "status", "cycle", "next", "tags", "contract", "renewal", "vendor", "plan", "price"] as const,
  minWidths: [10, 8, 6, 8, 8, 18, 6, 10, 8, 8] as const,
  maxWidths: [40, 12, 20, 10, 60, 30, 10, 30, 20, 20] as const,
}

const ALL_EXTRA_COLS: ColumnConfig = {
  headers: ["name", "status", "cycle", "next", "tags", "method", "contract", "renewal", "vendor", "plan", "notes", "price"] as const,
  minWidths: [8, 8, 6, 8, 8, 10, 18, 6, 10, 8, 15, 8] as const,
  maxWidths: [30, 12, 20, 10, 50, 30, 30, 10, 30, 20, 50, 20] as const,
}

function renderTable(rows: string[][], config: ColumnConfig): string {
  const widths = calcColumnWidths(rows, config)
  const colAligns = config.headers.map((_, i) =>
    i === config.headers.length - 1 ? "right" : "left",
  ) as ("left" | "right")[]

  const table = new CliTable3({
    chars: { ...TABLE_CHARS },
    style: getTableStyle(),
    colWidths: widths,
    head: [...config.headers],
    wordWrap: true,
    wrapOnWordBoundary: true,
    colAligns,
  })

  const colCount = config.headers.length
  const priceCol = colCount - 1
  const labelCol = priceCol - 1

  for (let i = 0; i < rows.length; i++) {
    const row = rows[i]
    // Total rows have empty strings in first three columns (name, status, cycle)
    const isTotal = row[0] === "" && row[1] === "" && row[2] === ""
    if (isTotal) {
      table.push(row.map((cell, j) => {
        if (j === labelCol || j === priceCol) return pc.bold(pc.yellow(cell))
        return cell
      }))
    } else {
      if (i % 2 === 0) {
        table.push(zebraRow(row))
      } else {
        table.push(row)
      }
    }
  }

  return table.toString()
}

/**
 * Render a subscription table.
 *
 * Display-only: this function never fetches or converts currency. When
 * `currency` is provided the caller must pass a list already converted to
 * that currency; entries that kept their original currency had a missing
 * exchange rate and are rendered as "? (original)".
 */
export const spreadSubscription = (
  list: SharedArgs[],
  currency?: Currency,
  showNotes?: boolean,
  showMethod?: boolean,
  showContract?: boolean,
  showVendor?: boolean,
): void => {
  const subs = list

  if (subs.length === 0) {
    consola.info("No subscriptions found — try `subtrack add`")
    return
  }

  let config = BASE_COLS
  if (showNotes && showMethod && showContract && showVendor) {
    config = ALL_EXTRA_COLS
  } else if (showContract && showVendor) {
    config = CONTRACT_VENDOR_COLS
  } else if (showContract) {
    config = CONTRACT_COLS
  } else if (showVendor) {
    config = VENDOR_COLS
  } else if (showNotes && showMethod) {
    config = ALL_COLS
  } else if (showNotes) {
    config = NOTES_COLS
  } else if (showMethod) {
    config = METHOD_COLS
  }
  const rows: string[][] = []

  if (currency) {
    // List is already converted to `currency`. Entries still in another
    // currency had no available rate — show them as "? (original price)".
    let total = 0
    for (const sub of subs) {
      if (sub.currency === currency) {
        total += sub.price
        rows.push(buildRow(sub, formatPrice(sub.price, currency), showNotes ?? false, showMethod ?? false, showContract, showVendor))
      } else {
        rows.push(
          buildRow(sub, `? (${formatPrice(sub.price, sub.currency)})`, showNotes ?? false, showMethod ?? false, showContract, showVendor),
        )
      }
    }

    const totalRow = new Array<string>(config.headers.length).fill("")
    totalRow[config.headers.length - 2] = `${currency} TOTAL`
    totalRow[config.headers.length - 1] = formatPrice(Math.round(total), currency)
    rows.push(totalRow)
    consola.log(renderTable(rows, config))
    return
  }

  // Display subscriptions grouped by currency
  const groups: Record<string, SharedArgs[]> = {}
  for (const sub of subs) {
    ;(groups[sub.currency] ??= []).push(sub)
  }

  const groupEntries = Object.entries(groups)
  for (let i = 0; i < groupEntries.length; i++) {
    const [currencyCode, currencySubs] = groupEntries[i]
    const groupRows: string[][] = []

    let total = 0
    for (const sub of currencySubs) {
      groupRows.push(buildRow(sub, formatPrice(sub.price, sub.currency), showNotes ?? false, showMethod ?? false, showContract, showVendor))
      total += sub.price
    }
    const totalRow = new Array<string>(config.headers.length).fill("")
    totalRow[config.headers.length - 2] = `${currencyCode} TOTAL`
    totalRow[config.headers.length - 1] = formatPrice(total, currencyCode)
    groupRows.push(totalRow)

    consola.log(renderTable(groupRows, config))
    if (i < groupEntries.length - 1) consola.log("")
  }
}

// ── LLM API Usage table ──────────────────────────────────

type UsageRow = [string, string, string, string, string, string, string]

const USAGE_HEADERS = ["Provider", "Model", "Input", "Output", "Cost", "Date", "Description"] as const
const USAGE_MIN_WIDTHS = [10, 20, 10, 10, 10, 12, 15] as const
const USAGE_MAX_WIDTHS = [20, 50, 14, 14, 14, 12, 60] as const

const USAGE_COLS: ColumnConfig = {
  headers: USAGE_HEADERS,
  minWidths: USAGE_MIN_WIDTHS,
  maxWidths: USAGE_MAX_WIDTHS,
  minAvail: 50,
}

function renderUsageTableBody(
  entries: LlmUsageEntry[],
  widths: number[],
): string {
  const table = new CliTable3({
    chars: { ...TABLE_CHARS },
    style: getTableStyle(),
    colWidths: widths,
    head: [...USAGE_HEADERS],
    wordWrap: true,
    wrapOnWordBoundary: true,
    colAligns: ["left", "left", "right", "right", "right", "left", "left"],
  })

  for (let i = 0; i < entries.length; i++) {
    const e = entries[i]
    const row: string[] = [
      e.provider,
      e.model,
      e.input_tokens.toLocaleString(),
      e.output_tokens.toLocaleString(),
      formatUsdCost(e.cost),
      e.date,
      e.description ?? "",
    ]
    if (i % 2 === 0) {
      table.push(zebraRow(row))
    } else {
      table.push(row)
    }
  }

  return table.toString()
}

/** TABLE_CHARS with a footer-style top border (used for the total row). */
const TABLE_CHARS_FOOTER = {
  ...TABLE_CHARS,
  "top-mid": "┴",
  "top-left": "├",
  "top-right": "┤",
} as const

export function renderUsageTable(entries: LlmUsageEntry[]): void {
  if (entries.length === 0) {
    consola.info("No paid usage entries found")
    return
  }

  const rows: UsageRow[] = entries.map((e) => [
    e.provider,
    e.model,
    e.input_tokens.toLocaleString(),
    e.output_tokens.toLocaleString(),
    formatUsdCost(e.cost),
    e.date,
    e.description ?? "",
  ])

  // TOTAL row appended for width calculation only
  const totalCost = entries.reduce((sum, e) => sum + e.cost, 0)
  const allRows = [
    ...rows,
    [
      "Total",
      "",
      "",
      "",
      formatUsdCost(totalCost, 2),
      "",
      `(${entries.length} entr${entries.length === 1 ? "y" : "ies"})`,
    ] as UsageRow,
  ]

  const widths = calcColumnWidths(allRows, USAGE_COLS)
  consola.log(renderUsageTableBody(entries, widths))

  // Render TOTAL footer row
  const table = new CliTable3({
    chars: { ...TABLE_CHARS_FOOTER },
    style: getTableStyle(),
    colWidths: widths,
    colAligns: ["left", "left", "right", "right", "right", "left", "left"],
  })
  table.push([
    pc.bold(pc.yellow("Total")),
    "",
    "",
    "",
    pc.bold(pc.yellow(formatUsdCost(totalCost, 2))),
    "",
    pc.dim(`(${entries.length} entr${entries.length === 1 ? "y" : "ies"})`),
  ])
  consola.log(table.toString())
}

// ── API Usage summary (list --api) ───────────────────────

export function showApiUsage(
  total: number,
  byProvider: { provider: string; total: number }[],
  periodLabel: string,
): void {
  consola.log("")
  consola.log(sectionTitle(`API Usage (${periodLabel})`))

  if (total <= 0) {
    consola.info("No API usage found for this month")
    return
  }

  const apiTable = new CliTable3({
    chars: { ...TABLE_CHARS },
    style: getTableStyle(),
    head: ["Provider", "Cost"],
    colAligns: ["left", "right"],
  })

  for (const p of byProvider) {
    apiTable.push([p.provider, formatUsdCost(p.total, 2)])
  }
  apiTable.push([
    pc.bold(pc.yellow("Total")),
    pc.bold(pc.yellow(formatUsdCost(total, 2))),
  ])

  consola.log(apiTable.toString())
}
