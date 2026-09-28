import { safeResponseJson } from "@subtrack/lib/json"
import { roundCurrency } from "./price.ts"
import type { SharedArgs } from "./types.ts"

export type FxRates = {
  base: string
  rates: Record<string, number>
}

const FX_FETCH_TIMEOUT_MS = 10_000

/**
 * Fetch current USD-based exchange rates from open.er-api.com.
 * Returns a map of currency codes to their exchange rate relative to USD.
 */
export async function fetchFxRates(): Promise<FxRates> {
  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), FX_FETCH_TIMEOUT_MS)
  try {
    const res = await fetch("https://open.er-api.com/v6/latest/USD", {
      signal: controller.signal,
    })
    if (!res.ok) {
      throw new Error(`FX API responded with ${res.status}`)
    }
    return await safeResponseJson<FxRates>(res)
  } finally {
    clearTimeout(timer)
  }
}

const MIN_SANE_RATE = 0.0001
const MAX_SANE_RATE = 1_000_000

/** Validate a rate value is within a sane range. */
function validateRate(rate: number, currency: string): void {
  if (rate <= 0 || rate > MAX_SANE_RATE || rate < MIN_SANE_RATE) {
    throw new Error(
      `Suspicious exchange rate for ${currency}: ${rate}` +
      ` (expected between ${MIN_SANE_RATE} and ${MAX_SANE_RATE})`,
    )
  }
}

export function convertPrice(
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
  validateRate(fromRate, from)
  validateRate(toRate, to)

  // Convert via USD base: source → USD → target
  const inUsd = from === "USD" ? price : price / fromRate
  return to === "USD" ? inUsd : inUsd * toRate
}

/**
 * Convert each subscription's price to the target currency using fetched rates.
 * All-or-nothing: throws if any conversion fails (e.g. missing rate),
 * matching the previous per-site try/catch behavior.
 */
export function convertSubsWithRates(
  subs: SharedArgs[],
  targetCurrency: string,
  rates: FxRates,
): SharedArgs[] {
  return subs.map((s) => ({
    ...s,
    price: roundCurrency(convertPrice(s.price, s.currency, targetCurrency, rates.rates)),
    currency: targetCurrency,
  }))
}

/**
 * Convert a single price, returning null (instead of throwing) when
 * no rate is available. Callers keep the original price on null.
 */
export function tryConvert(
  price: number,
  from: string,
  to: string,
  rates: Record<string, number>,
): number | null {
  try {
    return convertPrice(price, from, to, rates)
  } catch {
    return null
  }
}

/**
 * Convert each subscription's price to the target currency, keeping
 * subscriptions whose rate is missing in their original currency.
 * Returns the converted list plus a flag indicating any missing rate.
 */
export function tryConvertSubs(
  subs: SharedArgs[],
  targetCurrency: string,
  rates: FxRates,
): { list: SharedArgs[]; hasMissing: boolean } {
  let hasMissing = false
  const list = subs.map((s) => {
    const amount = tryConvert(s.price, s.currency, targetCurrency, rates.rates)
    if (amount === null) {
      hasMissing = true
      return s
    }
    return { ...s, price: roundCurrency(amount), currency: targetCurrency }
  })
  return { list, hasMissing }
}

/**
 * Fetch rates and convert a subscription list to the target currency.
 * Returns null when the rate fetch itself fails (callers then fall back
 * to the original-currency display).
 */
export async function fetchConvertedSubs(
  subs: SharedArgs[],
  targetCurrency: string,
): Promise<{ list: SharedArgs[]; hasMissing: boolean } | null> {
  let rates: FxRates
  try {
    rates = await fetchFxRates()
  } catch {
    return null
  }
  return tryConvertSubs(subs, targetCurrency, rates)
}

/**
 * Convert a list of amounts to the target currency, keeping the original
 * amount (null) for entries with a missing rate. Callers sum the non-null
 * values and report `hasMissing`.
 */
export function convertAmounts(
  amounts: readonly { amount: number; currency: string }[],
  targetCurrency: string,
  rates: FxRates,
): { converted: (number | null)[]; hasMissing: boolean } {
  const converted = amounts.map(({ amount, currency }) =>
    tryConvert(amount, currency, targetCurrency, rates.rates),
  )
  return { converted, hasMissing: converted.some((v) => v === null) }
}
