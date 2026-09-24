/**
 * Startup header shown by the interactive main menu: title, version,
 * subscription count, DB path, and a financial overview.
 *
 * Display-only — all calculations are delegated to their domain modules
 * (payment, upcoming, budget). No FX fetching keeps menu startup fast.
 */

import pc from "@subtrack/lib/ansi"
import { createRequire } from "node:module"
import { getSubscriptions, getNonCancelledSubscriptions, getDbPath } from "../db.ts"
import { calcSummary, calcSubTotal } from "../payment.ts"
import { calcUpcoming } from "../upcoming.ts"
import { resolveBudget } from "../budget.ts"
import { formatPrice } from "../price.ts"
import { divider } from "../display-constants.ts"

const require = createRequire(import.meta.url)

export function showMenuHeader(): void {
  const pkg = require("../package.json") as { version: string }
  const count = getSubscriptions().length
  console.log(pc.bold(pc.cyan(`subtrack v${pkg.version}`)))
  console.log(
    pc.dim(
      `  ${count} subscription${count === 1 ? "" : "s"} · ${getDbPath()}`,
    ),
  )

  // Financial overview (synchronous only — no FX fetch, keeps menu startup fast)
  const active = getNonCancelledSubscriptions()
  if (active.length > 0) {
    const summary = calcSummary(active)
    const monthly = Object.entries(summary.monthlyByCurrency)
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([ccy, total]) => formatPrice(Math.round(total), ccy))
    console.log(`  Monthly: ${monthly.join(" + ")}`)

    const upcoming = calcUpcoming(7)
    if (upcoming.length > 0) {
      const totals: Record<string, number> = {}
      for (const entry of upcoming) {
        totals[entry.sub.currency] = (totals[entry.sub.currency] ?? 0) + entry.amount
      }
      const parts = Object.entries(totals)
        .sort(([a], [b]) => a.localeCompare(b))
        .map(([ccy, total]) => formatPrice(Math.round(total), ccy))
      const billLabel = upcoming.length === 1 ? "bill" : "bills"
      console.log(`  Next 7 days: ${upcoming.length} ${billLabel} (${parts.join(" + ")})`)
    }

    // Budget overrun warning — only when comparable in a single currency
    const budget = resolveBudget("monthly")
    if (budget) {
      const totals = calcSubTotal(active, null, undefined, "monthly")
      const keys = Object.keys(totals)
      if (keys.length === 1 && keys[0] === budget.currency) {
        const spending = totals[keys[0]]!
        const remaining = budget.amount - spending
        if (remaining < 0) {
          console.log(
            pc.red(
              `  ⚠ Over budget: ${formatPrice(Math.round(-remaining), budget.currency)} (budget ${formatPrice(Math.round(budget.amount), budget.currency)}/month)`,
            ),
          )
        }
      }
    }
  } else {
    console.log(pc.dim("  No subscriptions yet — choose Add to create one"))
  }

  console.log(divider(52))
  console.log("")
}