/**
 * Format a price for display.
 *
 * Currently prices are stored as major units (e.g. 14.99 USD → "$14.99").
 * This function formats consistently across CLI and TUI surfaces.
 * When migrating to smallest-unit storage (cents), add a `fromCents` parameter.
 */

// `Intl.NumberFormat` construction is expensive (~µs each) and `formatPrice`
// is called once per table cell / summary line, so cache one formatter per
// currency. The cache is process-lifetime; currencies are a closed set.
const formatterCache = new Map<string, Intl.NumberFormat>()

export function formatPrice(price: number, currency: string): string {
  let formatter = formatterCache.get(currency)
  if (!formatter) {
    try {
      formatter = new Intl.NumberFormat("en-US", {
        style: "currency",
        currency,
        minimumFractionDigits: 0,
        maximumFractionDigits: 2,
      })
    } catch {
      return `${currency} ${price}`
    }
    formatterCache.set(currency, formatter)
  }
  return formatter.format(price)
}

/** Round a major-unit monetary value to the two decimal places used by display. */
export function roundCurrency(value: number): number {
  return Math.round(value * 100) / 100
}

/**
 * Format an API usage cost (stored in cents) as a USD string.
 * Defaults to 4 decimal places because LLM API costs are small.
 */
export function formatUsdCost(cents: number, digits = 4): string {
  return `$${(cents / 100).toFixed(digits)}`
}
