import type { Cycle, SharedArgs } from "../types.ts"
import {
  periodFactor,
  toDate,
  dateWithClampedDay,
  cycleDays,
  nextDayCycleDate,
  isDayCycle,
} from "@subtrack/lib/date"

export type CurrencyTotals = Record<string, number>

/** Pure billing calculation; presentation and currency conversion are deliberately outside. */
export function calculateTotals(subscriptions: SharedArgs[], period: Cycle): CurrencyTotals {
  const totals: CurrencyTotals = {}
  for (const subscription of subscriptions) {
    if (subscription.status === "cancelled") continue
    const amount = subscription.price * periodFactor(subscription.cycle, period)
    totals[subscription.currency] = (totals[subscription.currency] ?? 0) + amount
  }
  return totals
}

/** Return the monthly equivalent of a subscription's price. */
export function calculateMonthlyTotal(subscription: SharedArgs): number {
  return subscription.price * periodFactor(subscription.cycle, "monthly")
}

// ── Next-billing-date helpers ────────────────────────────
// Pure date logic kept out of upcoming.ts so display/banner paths don't pull
// the DB + FX machinery into their load graph.

function getBillingDay(sub: SharedArgs): number {
  if (sub.billingDay) return sub.billingDay
  // Fall back to created_at day
  const created = toDate(sub.createdAt)
  return created.getDate()
}

/**
 * Date of the k-th period occurrence anchored on `anchorDate`,
 * billed on `day` (clamped to the month length).
 */
function periodDate(anchorDate: Date, periodMonths: number, k: number, day: number): Date {
  const monthIndex = anchorDate.getMonth() + k * periodMonths
  const year = anchorDate.getFullYear() + Math.floor(monthIndex / 12)
  const month = ((monthIndex % 12) + 12) % 12
  return dateWithClampedDay(year, month, day)
}

export function nextDateForCycle(anchorDay: number, anchorDate: Date, cycle: Cycle, fromDate: Date): Date {
  // Every-N-days cycles (custom "Nd", weekly, bi-weekly) share one formula
  if (isDayCycle(cycle) || cycle === "weekly" || cycle === "bi-weekly") {
    const days = cycleDays(cycle) ?? (cycle === "weekly" ? 7 : 14)
    return nextDayCycleDate(anchorDate, anchorDay, days, fromDate)
  }
  switch (cycle) {
    case "monthly": {
      // Calculate next billing date based on anchor day
      const candidate = dateWithClampedDay(fromDate.getFullYear(), fromDate.getMonth(), anchorDay)
      if (candidate >= fromDate) return candidate
      // Move to next month
      return dateWithClampedDay(fromDate.getFullYear(), fromDate.getMonth() + 1, anchorDay)
    }
    case "yearly": {
      const candidate = dateWithClampedDay(fromDate.getFullYear(), anchorDate.getMonth(), anchorDay)
      if (candidate >= fromDate) return candidate
      return dateWithClampedDay(fromDate.getFullYear() + 1, anchorDate.getMonth(), anchorDay)
    }
    case "quarterly":
    case "semi-annual": {
      // Every 3/6 months from the anchor month, billed on anchorDay
      const periodMonths = cycle === "quarterly" ? 3 : 6
      let k = 0
      for (;;) {
        const candidate = periodDate(anchorDate, periodMonths, k, anchorDay)
        if (candidate >= fromDate) return candidate
        k++
      }
    }
  }
}

export function calculateNextBilling(sub: SharedArgs, fromDate: Date): Date {
  const anchorDate = toDate(sub.createdAt)
  const day = getBillingDay(sub)
  return nextDateForCycle(day, anchorDate, sub.cycle, fromDate)
}

export type UpcomingEntry = {
  sub: SharedArgs
  nextDate: Date
  amount: number
}

/**
 * Subscriptions that bill within `days` from `from` (inclusive).
 * Pure version — callers supply the subscription list.
 */
export function upcomingWithinDays(
  subs: SharedArgs[],
  days: number = 7,
  from: Date = new Date(),
): UpcomingEntry[] {
  if (subs.length === 0) return []

  const now = new Date(from)
  now.setHours(0, 0, 0, 0)
  const endDate = new Date(now)
  endDate.setDate(endDate.getDate() + days)

  const entries: UpcomingEntry[] = []

  for (const sub of subs) {
    const next = calculateNextBilling(sub, now)
    if (next >= now && next <= endDate) {
      entries.push({ sub, nextDate: next, amount: sub.price })
    }
  }

  entries.sort((a, b) => a.nextDate.getTime() - b.nextDate.getTime())
  return entries
}
