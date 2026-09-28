/**
 * Pure input validators and choice constants.
 *
 * Separated from prompts.ts so non-interactive command paths (list, summary,
 * import, mcp, ...) never load the interactive prompt machinery.
 */

import { cycleDays } from "@subtrack/lib/date"
import type { NamedCycle } from "@subtrack/lib/date"
import type { Currency, Cycle, Status } from "./types.ts"

export const CURRENCY_CHOICES: { name: string; value: Currency }[] = [
  { name: "AED (UAE Dirham)", value: "AED" },
  { name: "ARS (Argentine Peso)", value: "ARS" },
  { name: "AUD (Australian Dollar)", value: "AUD" },
  { name: "BRL (Brazilian Real)", value: "BRL" },
  { name: "CAD (Canadian Dollar)", value: "CAD" },
  { name: "CHF (Swiss Franc)", value: "CHF" },
  { name: "CLP (Chilean Peso)", value: "CLP" },
  { name: "CNY (Chinese Yuan)", value: "CNY" },
  { name: "COP (Colombian Peso)", value: "COP" },
  { name: "CZK (Czech Koruna)", value: "CZK" },
  { name: "DKK (Danish Krone)", value: "DKK" },
  { name: "EGP (Egyptian Pound)", value: "EGP" },
  { name: "EUR (Euro)", value: "EUR" },
  { name: "GBP (British Pound)", value: "GBP" },
  { name: "HKD (Hong Kong Dollar)", value: "HKD" },
  { name: "HUF (Hungarian Forint)", value: "HUF" },
  { name: "IDR (Indonesian Rupiah)", value: "IDR" },
  { name: "ILS (Israeli Shekel)", value: "ILS" },
  { name: "INR (Indian Rupee)", value: "INR" },
  { name: "JPY (日本円)", value: "JPY" },
  { name: "KRW (South Korean Won)", value: "KRW" },
  { name: "MXN (Mexican Peso)", value: "MXN" },
  { name: "MYR (Malaysian Ringgit)", value: "MYR" },
  { name: "NGN (Nigerian Naira)", value: "NGN" },
  { name: "NOK (Norwegian Krone)", value: "NOK" },
  { name: "NZD (New Zealand Dollar)", value: "NZD" },
  { name: "PHP (Philippine Peso)", value: "PHP" },
  { name: "PLN (Polish Zloty)", value: "PLN" },
  { name: "SAR (Saudi Riyal)", value: "SAR" },
  { name: "SEK (Swedish Krona)", value: "SEK" },
  { name: "SGD (Singapore Dollar)", value: "SGD" },
  { name: "THB (Thai Baht)", value: "THB" },
  { name: "TRY (Turkish Lira)", value: "TRY" },
  { name: "TWD (Taiwan Dollar)", value: "TWD" },
  { name: "USD (US Dollar)", value: "USD" },
  { name: "VND (Vietnamese Dong)", value: "VND" },
  { name: "ZAR (South African Rand)", value: "ZAR" },
]

export const STATUS_CHOICES: { name: string; value: Status }[] = [
  { name: "active", value: "active" },
  { name: "paused", value: "paused" },
  { name: "cancelled", value: "cancelled" },
  { name: "archived", value: "archived" },
]

/** Sentinel value returned by the cycle prompt when a custom day count is chosen. */
export const CUSTOM_CYCLE = "custom"

export const CUSTOM_CYCLE_CHOICE = {
  name: "custom (every N days)",
  value: CUSTOM_CYCLE,
} as const

export const CYCLE_CHOICES: { name: string; value: NamedCycle }[] = [
  { name: "weekly", value: "weekly" },
  { name: "bi-weekly", value: "bi-weekly" },
  { name: "monthly", value: "monthly" },
  { name: "quarterly", value: "quarterly" },
  { name: "semi-annual", value: "semi-annual" },
  { name: "yearly", value: "yearly" },
]

/**
 * Validate a currency code format (ISO 4217 3-letter).
 * --currency flag accepts any valid 3-letter code; interactive prompt restricts to CURRENCY_CHOICES.
 */
export function isValidCurrency(v: string): v is Currency {
  return /^[A-Z]{3}$/.test(v) && CURRENCY_CHOICES.some((c) => c.value === v)
}

export function isValidCycle(v: string): v is Cycle {
  if (CYCLE_CHOICES.some((c) => c.value === v)) return true
  return cycleDays(v as Cycle) !== null
}

export function isValidNamedCycle(v: string): v is NamedCycle {
  return CYCLE_CHOICES.some((choice) => choice.value === v)
}

export function validateCycleDays(v: string): string | true {
  if (!v.trim()) return "Days per cycle cannot be empty"
  const n = Number(v)
  if (!Number.isInteger(n) || n < 1 || n > 365)
    return "Enter a number between 1 and 365"
  return true
}

export function isValidStatus(v: string): v is Status {
  return v === "active" || v === "paused" || v === "cancelled" || v === "archived"
}

export function validateBillingDay(v: string): string | true {
  if (!v.trim()) return true // empty = not set
  const n = Number(v)
  if (!Number.isInteger(n) || n < 1 || n > 31) return "Enter a number between 1 and 31"
  return true
}

export function validateName(v: string): string | true {
  if (!v.trim()) return "Name cannot be empty"
  if (v.length > 100) return "Name too long (max 100 chars)"
  return true
}

export function validatePrice(v: string): string | true {
  if (!v.trim()) return "Please enter a valid number"
  const n = Number(v)
  if (!Number.isFinite(n) || n < 0)
    return "Please enter a valid non-negative number"
  if (n > 99999999) return "Price too high (max 99,999,999)"
  return true
}

export function validateNotes(v: string): string | true {
  if (v.length > 500) return "Notes too long (max 500 chars)"
  return true
}

export function validatePaymentMethod(v: string): string | true {
  if (v.length > 50) return "Payment method too long (max 50 chars)"
  return true
}

/** Validate a calendar date without allowing JavaScript's date rollover. */
function isValidIsoDate(v: string): boolean {
  const d = new Date(`${v}T00:00:00.000Z`)
  return Number.isFinite(d.getTime()) && d.toISOString().slice(0, 10) === v
}

export function validateDateString(v: string): string | true {
  if (!v.trim()) return true // empty = not set
  if (!/^\d{4}-\d{2}-\d{2}$/.test(v)) return "Use YYYY-MM-DD format"
  if (!isValidIsoDate(v)) return "Invalid date"
  return true
}

export function validateVendorName(v: string): string | true {
  if (v.length > 100) return "Vendor name too long (max 100 chars)"
  return true
}

export function validateVendorUrl(v: string): string | true {
  if (!v.trim()) return true
  if (v.length > 500) return "URL too long (max 500 chars)"
  let parsed: URL
  try {
    parsed = new URL(v)
  } catch {
    return "Invalid URL (must start with http:// or https://)"
  }
  // Reject non-http(s) schemes (javascript:, file:, data:, ...)
  if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
    return "Invalid URL (must start with http:// or https://)"
  }
  return true
}

export function validatePlanTier(v: string): string | true {
  if (v.length > 100) return "Plan tier too long (max 100 chars)"
  return true
}

export function validateDiscountValue(v: string): string | true {
  if (!v.trim()) return true
  const n = Number(v)
  if (!Number.isFinite(n) || n < 0) return "Please enter a non-negative number"
  if (n > 99999999) return "Value too high"
  return true
}

export function validateDiscountType(v: string): string | true {
  if (!v.trim()) return true
  if (v !== "percentage" && v !== "fixed") return 'Must be "percentage" or "fixed"'
  return true
}

export function validateAutoRenewal(v: string): string | true {
  if (v !== "true" && v !== "false") return 'Must be "true" or "false"'
  return true
}

export function validateTrialName(v: string): string | true {
  if (!v.trim()) return "Name cannot be empty"
  if (v.length > 100) return "Name too long (max 100 chars)"
  return true
}

export function validateExpiresAt(v: string): string | true {
  if (!v.trim()) return "Expiration date cannot be empty"
  if (!/^\d{4}-\d{2}-\d{2}$/.test(v)) return "Use YYYY-MM-DD format"
  if (!isValidIsoDate(v)) return "Invalid date"
  return true
}

export function validateTags(v: string): string | true {
  if (!v.trim()) return true
  const tags = v.split(",").map((t) => t.trim()).filter(Boolean)
  if (tags.length > 10) return "Maximum 10 tags allowed"
  for (const tag of tags) {
    if (tag.length > 50) return `Tag too long: "${tag}" (max 50 chars)`
  }
  return true
}

// ── LLM API usage validators ─────────────────────────────

export function validateTokens(v: string): string | true {
  if (!v.trim()) return "Please enter a number"
  const n = Number(v)
  if (!Number.isInteger(n) || n < 0) return "Please enter a non-negative integer"
  if (n > 9_999_999_999) return "Number too large (max 9,999,999,999)"
  return true
}

export function validateDate(v: string): string | true {
  if (!v.trim()) return true // empty means today
  if (!/^\d{4}-\d{2}-\d{2}$/.test(v)) return "Use YYYY-MM-DD format"
  if (!isValidIsoDate(v)) return "Invalid date"
  return true
}

export function validateModelName(v: string): string | true {
  if (!v.trim()) return "Model name cannot be empty"
  if (v.length > 200) return "Model name too long (max 200 chars)"
  return true
}