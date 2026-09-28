/**
 * Shared per-currency total calculations with FX conversion.
 *
 * Kept out of payment.ts so the `budget`/`compare` commands don't pull the
 * full payment module graph (usage DB, pre-command hooks, output) into their
 * load path — they only need the totals math.
 */

import { priceHistoryRepository } from "./application/index.ts"
import type { Currency, SharedArgs } from "./types.ts"
import type { NamedCycle } from "@subtrack/lib/date"
import { periodFactor } from "@subtrack/lib/date"
import { tryConvert } from "./fx.ts"
import type { FxRates } from "./fx.ts"

export type CcyTotals = Record<string, number>

/**
 * Calculate per-currency monthly totals for a set of subscriptions,
 * optionally converting to a target currency.
 */
export function calcSubTotal(
  subs: SharedArgs[],
  rates: FxRates | null,
  targetCurrency: Currency | undefined,
  period: NamedCycle = "monthly",
): CcyTotals {
  const totals: CcyTotals = {}
  for (const sub of subs) {
    if (sub.status === "cancelled") continue
    const normalized = sub.price * periodFactor(sub.cycle, period)
    if (targetCurrency && rates) {
      const converted = tryConvert(normalized, sub.currency, targetCurrency, rates.rates)
      if (converted !== null) {
        totals[targetCurrency] = (totals[targetCurrency] ?? 0) + converted
      } else {
        totals[sub.currency] = (totals[sub.currency] ?? 0) + normalized
      }
    } else {
      totals[sub.currency] = (totals[sub.currency] ?? 0) + normalized
    }
  }
  return totals
}

/**
 * Calculate per-currency monthly totals using historical prices from
 * price history to estimate the previous period's costs.
 */
export function calcPreviousTotals(
  activeSubs: SharedArgs[],
  rates: FxRates | null,
  targetCurrency: Currency | undefined,
  period: NamedCycle = "monthly",
): CcyTotals {
  const priceChanges = priceHistoryRepository.listRecent()
  const priceBefore: Record<number, { price: number; currency: string }> = {}

  for (const change of priceChanges) {
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
    const monthly = price * periodFactor(sub.cycle, period)

    if (targetCurrency && rates) {
      const converted = tryConvert(monthly, currency, targetCurrency, rates.rates)
      if (converted !== null) {
        totals[targetCurrency] = (totals[targetCurrency] ?? 0) + converted
      } else {
        totals[currency] = (totals[currency] ?? 0) + monthly
      }
    } else {
      totals[currency] = (totals[currency] ?? 0) + monthly
    }
  }
  return totals
}