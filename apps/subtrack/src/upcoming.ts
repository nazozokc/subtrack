import { consola } from "@subtrack/lib/logger"
import pc from "@subtrack/lib/ansi"
import type { Currency } from "./types.ts"
import { getNonCancelledSubscriptions } from "./db/subscriptions.ts"
import { formatPrice, roundCurrency } from "./price.ts"
import { fetchFxRates, tryConvert } from "./fx.ts"
import { formatDate, formatShortDate, daysUntil } from "@subtrack/lib/date"
import { runPreCommandHooks } from "./pre-command.ts"
import {
  nextDateForCycle,
  calculateNextBilling,
  upcomingWithinDays,
} from "./domain/billing.ts"
import type { UpcomingEntry } from "./domain/billing.ts"

// Re-export the pure billing-date helpers moved to domain/billing.ts for
// backward compatibility (cancel.ts, export.ts import them from here).
export { nextDateForCycle, calculateNextBilling, upcomingWithinDays }
export type { UpcomingEntry }

/**
 * Subscriptions that bill within `days` from today.
 * Thin wrapper over the pure `upcomingWithinDays` that loads the active list.
 */
export function calcUpcoming(days: number = 7): UpcomingEntry[] {
  return upcomingWithinDays(getNonCancelledSubscriptions(), days)
}

export async function calcUpcomingWithCurrency(days: number = 7, targetCurrency?: string): Promise<UpcomingEntry[]> {
  const entries = calcUpcoming(days)
  if (!targetCurrency || entries.length === 0) return entries

  try {
    const rates = await fetchFxRates()
    for (const entry of entries) {
      const converted = tryConvert(entry.amount, entry.sub.currency, targetCurrency as Currency, rates.rates)
      if (converted !== null) {
        entry.amount = roundCurrency(converted)
        entry.sub = { ...entry.sub, price: roundCurrency(converted), currency: targetCurrency }
      }
    }
  } catch {
    consola.warn("Failed to fetch exchange rates; showing in original currencies")
  }

  return entries
}

export async function showUpcoming(days: number = 7, options: { currency?: string } = {}): Promise<void> {
  const entries = await calcUpcomingWithCurrency(days, options.currency)

  if (entries.length === 0) {
    consola.info(`No upcoming bills in the next ${days} day${days > 1 ? "s" : ""}`)
    return
  }

  consola.log(pc.bold(`Upcoming bills (next ${days} day${days > 1 ? "s" : ""}):`))
  consola.log("")

  const currencyTotals: Record<string, number> = {}
  for (const entry of entries) {
    const dateStr = formatShortDate(entry.nextDate)
    const dayLabel = daysUntil(entry.nextDate) === 0 ? " (today)" : daysUntil(entry.nextDate) === 1 ? " (tomorrow)" : ""
    consola.log(
      `  ${pc.cyan(dateStr)}${pc.dim(dayLabel)}  ${pc.bold(entry.sub.name)}  ${formatPrice(entry.sub.price, entry.sub.currency)}/${entry.sub.cycle}  ${pc.dim(entry.sub.tags.length > 0 ? `[${entry.sub.tags.join(", ")}]` : "")}`,
    )
    currencyTotals[entry.sub.currency] = (currencyTotals[entry.sub.currency] ?? 0) + entry.sub.price
  }

  if (entries.length > 1) {
    consola.log("")
    const totalParts = Object.entries(currencyTotals)
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([ccy, total]) => formatPrice(roundCurrency(total), ccy))
    consola.log(`  ${pc.bold("Total:")} ${totalParts.join(" + ")} (across ${entries.length} subscription${entries.length > 1 ? "s" : ""})`)
  }
}

// ── Command handler ──────────────────────────────────────

export async function handleUpcoming(days: number = 7, options: { json?: boolean; currency?: string } = {}): Promise<void> {
  // Show notification banner for non-JSON output
  await runPreCommandHooks(options)

  if (options.json) {
    const entries = options.currency ? await calcUpcomingWithCurrency(days, options.currency) : calcUpcoming(days)
    const data = entries.map((e) => ({
      id: e.sub.id,
      name: e.sub.name,
      price: e.sub.price,
      currency: e.sub.currency,
      cycle: e.sub.cycle,
      nextDate: formatDate(e.nextDate),
      amount: roundCurrency(e.amount),
      tags: e.sub.tags,
    }))
    process.stdout.write(JSON.stringify(data, null, 2) + "\n")
    return
  }
  await showUpcoming(days, { currency: options.currency })
}