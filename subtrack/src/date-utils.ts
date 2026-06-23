/**
 * Convert a YYYY-MM-DD date string to a Unix timestamp in milliseconds
 * representing the start of that day (00:00:00 UTC).
 */
export function dateToStartOfDayMs(dateStr: string): number {
  const date = new Date(dateStr + "T00:00:00.000Z")
  return date.getTime()
}

/**
 * Convert a YYYY-MM-DD date string to a Unix timestamp in milliseconds
 * representing the end of that day (23:59:59.999 UTC).
 */
export function dateToEndOfDayMs(dateStr: string): number {
  const date = new Date(dateStr + "T23:59:59.999Z")
  return date.getTime()
}

/**
 * Get today's date as YYYY-MM-DD string (local timezone).
 */
export function today(): string {
  const now = new Date()
  return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`
}

/**
 * Get the first day of the current month as YYYY-MM-DD string.
 */
export function currentMonthStart(): string {
  const now = new Date()
  return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-01`
}

/**
 * Check if a Unix timestamp in milliseconds falls within a date range.
 * If from/to are undefined, any timestamp is considered in range.
 */
export function isInDateRange(
  timestampMs: number,
  from?: string,
  to?: string,
): boolean {
  if (!from && !to) return true
  const ts = timestampMs
  if (from && ts < dateToStartOfDayMs(from)) return false
  if (to && ts > dateToEndOfDayMs(to)) return false
  return true
}
