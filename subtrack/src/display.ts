import { consola } from "consola"
import pc from "picocolors"
import CliTable3 from "cli-table3"
import type { SharedArgs, Currency, LlmUsageEntry } from "./types.ts"
import { getSubscriptions } from "./db.ts"
import { fetchFxRates, convertPrice } from "./fx.ts"
import type { FxRates } from "./fx.ts"

export function formatPrice(price: number, currency: string): string {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency,
    minimumFractionDigits: 0,
    maximumFractionDigits: 0,
  }).format(price)
}

function buildRow(sub: SharedArgs, price: string): [string, string, string, string] {
  return [
    String(sub.name),
    String(sub.cycle),
    sub.tags.length > 0 ? sub.tags.join(", ") : "-",
    price,
  ]
}

const HEADERS = ["name", "cycle", "tags", "price"] as const
const MIN_WIDTHS = [10, 6, 8, 8] as const
const MAX_WIDTHS = [40, 20, 60, 20] as const
const BORDER_AND_PADDING = 13

function calcColumnWidths(rows: string[][]): number[] {
  const termWidth = process.stdout.columns ?? 80
  const avail = Math.max(40, termWidth - BORDER_AND_PADDING)

  const weights = HEADERS.map((hdr, i) => {
    let max = hdr.length
    for (const row of rows) {
      const len = row[i].length
      if (len > max) max = len
    }
    return Math.min(max, MAX_WIDTHS[i])
  })

  const totalWeight = weights.reduce((a, b) => a + b, 0)
  const widths = weights.map((w, i) =>
    Math.max(
      MIN_WIDTHS[i],
      Math.min(MAX_WIDTHS[i], Math.round((avail * w) / totalWeight)),
    ),
  )

  // Adjust to exactly fit avail
  let sum = widths.reduce((a, b) => a + b, 0)
  let diff = sum - avail
  let iterations = 0

  while (diff > 0 && iterations < 100) {
    let idx = -1
    for (let i = 0; i < widths.length; i++) {
      if (widths[i] > MIN_WIDTHS[i] && (idx === -1 || widths[i] > widths[idx]))
        idx = i
    }
    if (idx === -1) break
    widths[idx]--
    diff--
    iterations++
  }

  iterations = 0
  while (diff < 0 && iterations < 100) {
    let idx = -1
    for (let i = 0; i < widths.length; i++) {
      if (widths[i] < MAX_WIDTHS[i] && (idx === -1 || weights[i] > weights[idx]))
        idx = i
    }
    if (idx === -1) break
    widths[idx]++
    diff++
    iterations++
  }

  return widths
}

function renderTable(rows: string[][]): string {
  const widths = calcColumnWidths(rows)

  const table = new CliTable3({
    chars: {
      top: "─",
      "top-mid": "┬",
      "top-left": "┌",
      "top-right": "┐",
      bottom: "─",
      "bottom-mid": "┴",
      "bottom-left": "└",
      "bottom-right": "┘",
      left: "│",
      "left-mid": "├",
      mid: "─",
      "mid-mid": "┼",
      right: "│",
      "right-mid": "┤",
      middle: "│",
    },
    style: {
      border: ["\x1b[90m", "\x1b[0m"],
      head: ["\x1b[1m\x1b[38;5;75m", "\x1b[0m"],
      "padding-left": 1,
      "padding-right": 1,
      compact: false,
    },
    colWidths: widths,
    head: [...HEADERS],
    wordWrap: true,
    wrapOnWordBoundary: true,
    colAligns: ["left", "left", "left", "right"],
  })

  for (let i = 0; i < rows.length; i++) {
    const row = rows[i]
    const isTotal = row[2].endsWith("TOTAL")
    if (isTotal) {
      table.push(row.map((cell, j) => {
        if (j === 2 || j === 3) return pc.bold(pc.yellow(cell))
        return cell
      }))
    } else {
      if (i % 2 === 0) {
        table.push(row.map(cell => `\x1b[48;5;236m${cell}\x1b[0m`))
      } else {
        table.push(row)
      }
    }
  }

  return table.toString()
}

export const spreadSubscription = async (
  get?: SharedArgs[],
  currency?: Currency,
): Promise<void> => {
  const list = get ?? getSubscriptions()

  if (list.length === 0) {
    consola.info("No subscriptions found")
    return
  }

  const rows: string[][] = []

  if (currency) {
    // --currency specified: fetch rates and convert all to the target currency
    let rates: FxRates | null = null
    try {
      consola.info("Fetching the latest exchange rates...")
      rates = await fetchFxRates()
      consola.success("Exchange rates updated")
    } catch (e) {
      consola.fail(`Failed to fetch exchange rates: ${e}`)
    }

    if (rates) {
      let total = 0
      let hasMissingRate = false

      for (const sub of list) {
        try {
          const converted = convertPrice(
            sub.price,
            sub.currency,
            currency,
            rates.rates,
          )
          total += converted
          rows.push(buildRow(sub, formatPrice(Math.round(converted), currency)))
        } catch {
          hasMissingRate = true
          rows.push(
            buildRow(sub, `? (${formatPrice(sub.price, sub.currency)})`),
          )
        }
      }

      rows.push(["", "", `${currency} TOTAL`, formatPrice(Math.round(total), currency)])

      if (hasMissingRate) {
        consola.warn(
          "Some prices could not be converted (missing rate). They are shown in original currency.",
        )
      }

      consola.log(renderTable(rows))
      return
    }
    // fallback: continue to the no-currency path below
  }

  // Display subscriptions grouped by currency
  const groups: Record<string, SharedArgs[]> = {}
  for (const sub of list) {
    ;(groups[sub.currency] ??= []).push(sub)
  }

  const groupEntries = Object.entries(groups)
  for (let i = 0; i < groupEntries.length; i++) {
    const [currencyCode, subs] = groupEntries[i]
    const groupRows: string[][] = []

    let total = 0
    for (const sub of subs) {
      groupRows.push(buildRow(sub, formatPrice(sub.price, sub.currency)))
      total += sub.price
    }
    groupRows.push([
      "",
      "",
      `${currencyCode} TOTAL`,
      formatPrice(total, currencyCode),
    ])

    consola.log(renderTable(groupRows))
    if (i < groupEntries.length - 1) consola.log("")
  }
}

// ── LLM API Usage table ──────────────────────────────────

type UsageRow = [string, string, string, string, string, string, string]

const USAGE_HEADERS = ["Provider", "Model", "Input", "Output", "Cost", "Date", "Description"] as const
const USAGE_MIN_WIDTHS = [10, 20, 10, 10, 10, 12, 15] as const
const USAGE_MAX_WIDTHS = [20, 50, 14, 14, 14, 12, 60] as const

function calcUsageColumnWidths(rows: UsageRow[]): number[] {
  const termWidth = process.stdout.columns ?? 80
  const avail = Math.max(50, termWidth - BORDER_AND_PADDING)

  const weights = USAGE_HEADERS.map((hdr, i) => {
    let max = hdr.length
    for (const row of rows) {
      const len = row[i].length
      if (len > max) max = len
    }
    return Math.min(max, USAGE_MAX_WIDTHS[i])
  })

  const totalWeight = weights.reduce((a, b) => a + b, 0)
  const widths = weights.map((w, i) =>
    Math.max(
      USAGE_MIN_WIDTHS[i],
      Math.min(USAGE_MAX_WIDTHS[i], Math.round((avail * w) / totalWeight)),
    ),
  )

  // Fit to available width
  let sum = widths.reduce((a, b) => a + b, 0)
  let diff = sum - avail
  let iterations = 0
  while (diff > 0 && iterations < 100) {
    let idx = -1
    for (let i = 0; i < widths.length; i++) {
      if (widths[i] > USAGE_MIN_WIDTHS[i] && (idx === -1 || widths[i] > widths[idx])) idx = i
    }
    if (idx === -1) break
    widths[idx]--
    diff--
    iterations++
  }
  iterations = 0
  while (diff < 0 && iterations < 100) {
    let idx = -1
    for (let i = 0; i < widths.length; i++) {
      if (widths[i] < USAGE_MAX_WIDTHS[i] && (idx === -1 || weights[i] > weights[idx])) idx = i
    }
    if (idx === -1) break
    widths[idx]++
    diff++
    iterations++
  }

  return widths
}

function renderUsageTableBody(
  entries: LlmUsageEntry[],
  widths: number[],
): string {
  const table = new CliTable3({
    chars: {
      top: "─",
      "top-mid": "┬",
      "top-left": "┌",
      "top-right": "┐",
      bottom: "─",
      "bottom-mid": "┴",
      "bottom-left": "└",
      "bottom-right": "┘",
      left: "│",
      "left-mid": "├",
      mid: "─",
      "mid-mid": "┼",
      right: "│",
      "right-mid": "┤",
      middle: "│",
    },
    style: {
      border: ["\x1b[90m", "\x1b[0m"],
      head: ["\x1b[1m\x1b[38;5;75m", "\x1b[0m"],
      "padding-left": 1,
      "padding-right": 1,
      compact: false,
    },
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
      `$${(e.cost / 100).toFixed(4)}`,
      e.date,
      e.description ?? "",
    ]
    if (i % 2 === 0) {
      table.push(row.map((cell) => `\x1b[48;5;236m${cell}\x1b[0m`))
    } else {
      table.push(row)
    }
  }

  return table.toString()
}

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
    `$${(e.cost / 100).toFixed(4)}`,
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
      `$${(totalCost / 100).toFixed(2)}`,
      "",
      `(${entries.length} entr${entries.length === 1 ? "y" : "ies"})`,
    ] as UsageRow,
  ]

  const widths = calcUsageColumnWidths(allRows)
  consola.log(renderUsageTableBody(entries, widths))

  // Render TOTAL footer row
  const table = new CliTable3({
    chars: {
      top: "─",
      "top-mid": "┴",
      "top-left": "├",
      "top-right": "┤",
      bottom: "─",
      "bottom-mid": "┴",
      "bottom-left": "└",
      "bottom-right": "┘",
      left: "│",
      "left-mid": "├",
      mid: "─",
      "mid-mid": "┼",
      right: "│",
      "right-mid": "┤",
      middle: "│",
    },
    style: {
      border: ["\x1b[90m", "\x1b[0m"],
      "padding-left": 1,
      "padding-right": 1,
      compact: false,
    },
    colWidths: widths,
    colAligns: ["left", "left", "right", "right", "right", "left", "left"],
  })
  table.push([
    pc.bold(pc.yellow("Total")),
    "",
    "",
    "",
    pc.bold(pc.yellow(`$${(totalCost / 100).toFixed(2)}`)),
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
  consola.log(pc.bold(pc.cyan(`── API Usage (${periodLabel}) ──`)))

  if (total <= 0) {
    consola.info("No API usage found for this month")
    return
  }

  const apiTable = new CliTable3({
    chars: {
      top: "─",
      "top-mid": "┬",
      "top-left": "┌",
      "top-right": "┐",
      bottom: "─",
      "bottom-mid": "┴",
      "bottom-left": "└",
      "bottom-right": "┘",
      left: "│",
      "left-mid": "├",
      mid: "─",
      "mid-mid": "┼",
      right: "│",
      "right-mid": "┤",
      middle: "│",
    },
    style: {
      border: ["\x1b[90m", "\x1b[0m"],
      head: ["\x1b[1m\x1b[38;5;75m", "\x1b[0m"],
      "padding-left": 1,
      "padding-right": 1,
      compact: false,
    },
    head: ["Provider", "Cost"],
    colAligns: ["left", "right"],
  })

  for (const p of byProvider) {
    apiTable.push([p.provider, `$${(p.total / 100).toFixed(2)}`])
  }
  apiTable.push([
    pc.bold(pc.yellow("Total")),
    pc.bold(pc.yellow(`$${(total / 100).toFixed(2)}`)),
  ])

  consola.log(apiTable.toString())
}
