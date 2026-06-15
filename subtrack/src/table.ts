import { consola } from "consola";
import { type SharedArgs, type Currency, getSubscriptions } from "./basefs.ts";

type FxRates = {
  base: string
  rates: Record<string, number>
}

type RowData = [string, string, string, string]

async function fetchFxRates(): Promise<FxRates> {
  const res = await fetch("https://open.er-api.com/v6/latest/USD")
  if (!res.ok) {
    throw new Error(`FX API responded with ${res.status}`)
  }
  return res.json() as Promise<FxRates>
}

function convertPrice(
  price: number,
  from: string,
  to: string,
  rates: Record<string, number>,
): number {
  if (from === to) return price
  const fromRate = rates[from]
  const toRate = rates[to]
  if (!fromRate || !toRate) {
    throw new Error(`No rate available for ${from} → ${to}`)
  }
  // Convert via USD base: source → USD → target
  const inUsd = from === "USD" ? price : price / fromRate
  return to === "USD" ? inUsd : inUsd * toRate
}

export function formatPrice(price: number, currency: string): string {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency,
    minimumFractionDigits: 0,
    maximumFractionDigits: 0,
  }).format(price)
}

const HEADERS = ["name", "cycle", "tags", "price"] as const
const MIN_WIDTHS = [10, 6, 8, 8] as const
const MAX_WIDTHS = [40, 20, 60, 20] as const
const BORDER_AND_PADDING = 13

function wrapCell(text: string, width: number): string[] {
  if (!text || text.length === 0) return [""]
  if (text.length <= width) return [text]

  const lines: string[] = []
  let remaining = text

  while (remaining.length > 0) {
    if (remaining.length <= width) {
      lines.push(remaining)
      break
    }

    const slice = remaining.slice(0, width)
    const sp = slice.lastIndexOf(" ")

    if (sp > 0) {
      lines.push(remaining.slice(0, sp))
      remaining = remaining.slice(sp + 1)
    } else {
      lines.push(slice)
      remaining = remaining.slice(width)
    }
  }

  return lines
}

function renderTable(rows: RowData[]): string {
  const termWidth = process.stdout.columns ?? 80
  const avail = Math.max(40, termWidth - BORDER_AND_PADDING)

  // Weights = max content length per column (capped at MAX_WIDTHS)
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
    Math.max(MIN_WIDTHS[i], Math.min(MAX_WIDTHS[i], Math.round(avail * w / totalWeight))),
  )

  // Adjust to exactly fit avail
  let sum = widths.reduce((a, b) => a + b, 0)
  let diff = sum - avail

  let iterations = 0
  while (diff > 0 && iterations < 100) {
    let idx = -1
    for (let i = 0; i < widths.length; i++) {
      if (widths[i] > MIN_WIDTHS[i] && (idx === -1 || widths[i] > widths[idx])) {
        idx = i
      }
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
      if (widths[i] < MAX_WIDTHS[i] && (idx === -1 || weights[i] > weights[idx])) {
        idx = i
      }
    }
    if (idx === -1) break
    widths[idx]++
    diff++
    iterations++
  }

  // ── Render ──────────────────────────────────────────
  const PAD = 1
  const [H, V] = ["─", "│"]
  const [tl, tm, tr] = ["┌", "┬", "┐"]
  const [ml, mm, mr] = ["├", "┼", "┤"]
  const [bl, bm, br] = ["└", "┴", "┘"]

  function border(l: string, m: string, r: string): string {
    return l + widths.map((w) => H.repeat(w + PAD * 2)).join(m) + r
  }

  function dataRow(row: string[]): string[] {
    const wrapped = row.map((text, i) => wrapCell(text, widths[i]))
    const maxLines = Math.max(...wrapped.map((w) => w.length))
    const lines: string[] = []

    for (let li = 0; li < maxLines; li++) {
      const parts: string[] = [V]
      for (let ci = 0; ci < row.length; ci++) {
        const text = wrapped[ci][li] ?? ""
        parts.push(" ".repeat(PAD), text.padEnd(widths[ci]), " ".repeat(PAD), V)
      }
      lines.push(parts.join(""))
    }

    return lines
  }

  const out: string[] = []
  out.push(border(tl, tm, tr))
  out.push(...dataRow([...HEADERS]))
  out.push(border(ml, mm, mr))

  for (const row of rows) out.push(...dataRow(row))

  out.push(border(bl, bm, br))
  return out.join("\n")
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

  const rows: RowData[] = []

  if (currency) {
    // --currency specified: fetch rates and convert all to the target currency
    let rates: FxRates | null = null
    try {
      consola.start("Fetching exchange rates...")
      rates = await fetchFxRates()
      consola.success("Exchange rates fetched")
    } catch (e) {
      consola.fail(`Failed to fetch exchange rates: ${e}`)
    }

    if (rates) {
      const fmt = new Intl.NumberFormat(
        currency === "JPY" ? "ja-JP" : "en-US",
        {
          style: "currency",
          currency,
          minimumFractionDigits: currency === "JPY" ? 0 : 2,
          maximumFractionDigits: currency === "JPY" ? 0 : 2,
        },
      )

      let total = 0
      let hasMissingRate = false

      for (const sub of list) {
        try {
          const converted = convertPrice(sub.price, sub.currency, currency, rates.rates)
          total += converted
          rows.push([
            String(sub.name),
            String(sub.cycle),
            sub.tags.length > 0 ? sub.tags.join(", ") : "-",
            fmt.format(converted),
          ])
        } catch {
          hasMissingRate = true
          rows.push([
            String(sub.name),
            String(sub.cycle),
            sub.tags.length > 0 ? sub.tags.join(", ") : "-",
            `? (${formatPrice(sub.price, sub.currency)})`,
          ])
        }
      }

      rows.push(["", "", `${currency} TOTAL`, fmt.format(total)])

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

  // Display subscriptions with their original currencies
  for (const sub of list) {
    rows.push([
      String(sub.name),
      String(sub.cycle),
      sub.tags.length > 0 ? sub.tags.join(", ") : "-",
      formatPrice(sub.price, sub.currency),
    ])
  }

  // Group totals by currency
  const totals: Record<string, number> = {}
  for (const sub of list) {
    totals[sub.currency] = (totals[sub.currency] || 0) + sub.price
  }

  for (const [currencyCode, total] of Object.entries(totals)) {
    rows.push(["", "", `${currencyCode} TOTAL`, formatPrice(total, currencyCode)])
  }

  consola.log(renderTable(rows))
}
