/**
 * Security limits, rate limiter, and input validation for the MCP server.
 */

// ── Security limits ──────────────────────────────────────

export const MAX_REQUEST_SIZE = 1024 * 100 // 100 KB max request payload
export const RATE_LIMIT_TOKENS = 60        // max requests per window
export const RATE_LIMIT_WINDOW_MS = 60_000 // 1 minute window
export const MAX_STRING_LENGTH = 500       // max length for string inputs
export const MAX_TAG_COUNT = 20            // preserve existing subscription capacity

type ValidationRule = {
  type: string
  maxLength?: number
  min?: number
  max?: number
  integer?: boolean
  enum?: readonly string[]
}

/** Simple fixed-window rate limiter. */
export class RateLimiter {
  private tokens: number
  private lastRefill: number

  constructor(private maxTokens: number, private windowMs: number) {
    this.tokens = maxTokens
    this.lastRefill = Date.now()
  }

  tryConsume(): boolean {
    const now = Date.now()
    const elapsed = now - this.lastRefill
    if (elapsed >= this.windowMs) {
      this.tokens = this.maxTokens
      this.lastRefill = now
    }
    if (this.tokens <= 0) return false
    this.tokens--
    return true
  }
}

export const rateLimiter = new RateLimiter(RATE_LIMIT_TOKENS, RATE_LIMIT_WINDOW_MS)

/** Validate argument types and lengths to prevent abuse. */
export function validateArgs(
  args: Record<string, unknown> | undefined,
  schema: Record<string, ValidationRule>,
): string | null {
  if (!args) return null
  for (const [key, rules] of Object.entries(schema)) {
    const value = args[key]
    if (value === undefined) continue
    if (rules.type === "string") {
      if (typeof value !== "string") return `${key} must be a string`
      if (rules.enum && !rules.enum.includes(value)) return `${key} has an unsupported value`
      const maxLen = rules.maxLength ?? MAX_STRING_LENGTH
      if (value.length > maxLen) return `${key} too long (max ${maxLen} chars)`
    } else if (rules.type === "number") {
      if (typeof value !== "number" || !Number.isFinite(value)) return `${key} must be a finite number`
      if (rules.integer && !Number.isInteger(value)) return `${key} must be an integer`
      const min = rules.min ?? 0
      const max = rules.max ?? 1_000_000_000
      if (value < min) return `${key} must be at least ${min}`
      if (value > max) return `${key} must be at most ${max}`
    } else if (rules.type === "boolean") {
      if (typeof value !== "boolean") return `${key} must be a boolean`
    }
  }
  return null
}

/**
 * Pre-invocation guard for `tools/call`: rejects oversized argument payloads
 * and inputs that violate the per-tool schema. Returns an error message, or
 * null when the call may proceed.
 */
export function validateToolCall(
  name: string,
  args: Record<string, unknown> | undefined,
): string | null {
  const rawSize = JSON.stringify(args ?? {}).length
  if (rawSize > MAX_REQUEST_SIZE) {
    return `Request too large (${rawSize} bytes, max ${MAX_REQUEST_SIZE})`
  }
  const schema = INPUT_VALIDATIONS[name]
  if (args && schema) {
    const err = validateArgs(args, schema)
    if (err) return `Validation error: ${err}`
  }
  return null
}

/** Input validation schemas per tool. */
export const INPUT_VALIDATIONS: Record<string, Record<string, ValidationRule>> = {
  add_subscription: {
    name: { type: "string", maxLength: 100 },
    price: { type: "number", min: 0, max: 99_999_999 },
    currency: { type: "string", maxLength: 3 },
    cycle: { type: "string", maxLength: 20 },
    tags: { type: "string", maxLength: 500 },
    billingDay: { type: "number", min: 1, max: 31, integer: true },
    status: { type: "string", maxLength: 10 },
    paymentMethod: { type: "string", maxLength: 50 },
    notes: { type: "string", maxLength: 500 },
  },
  edit_subscription: {
    id: { type: "number", min: 1, integer: true },
    name: { type: "string", maxLength: 100 },
    price: { type: "number", min: 0, max: 99_999_999 },
    currency: { type: "string", maxLength: 3 },
    cycle: { type: "string", maxLength: 20 },
    status: { type: "string", maxLength: 10 },
    tags: { type: "string", maxLength: 500 },
    paymentMethod: { type: "string", maxLength: 50 },
    notes: { type: "string", maxLength: 500 },
    billingDay: { type: "number", min: 1, max: 31, integer: true },
  },
  delete_subscription: {
    id: { type: "number", min: 1, integer: true },
  },
  search_subscriptions: {
    query: { type: "string", maxLength: 200 },
    names: { type: "boolean" },
    notes: { type: "boolean" },
    tags: { type: "boolean" },
  },
  list_subscriptions: {
    sort: { type: "string", maxLength: 20 },
    desc: { type: "boolean" },
    limit: { type: "number", min: 1, max: 10_000, integer: true },
    offset: { type: "number", min: 0, max: 1_000_000_000, integer: true },
  },
  get_subscription: {
    id: { type: "number", min: 1, integer: true },
  },
  get_summary: {},
  get_upcoming: {
    days: { type: "number", min: 0, max: 3_650, integer: true },
  },
  get_calendar: {
    month: { type: "number", min: 1, max: 12, integer: true },
    year: { type: "number", min: 1, max: 9_999, integer: true },
  },
  export_data: {
    format: { type: "string", maxLength: 10, enum: ["csv", "json", "md"] },
  },
  get_history: {
    id: { type: "number", min: 1, integer: true },
    days: { type: "number", min: 0, max: 3_650, integer: true },
  },
  get_analytics: {},
  get_forecast: {
    months: { type: "number", min: 1, max: 120, integer: true },
    currency: { type: "string", maxLength: 3 },
    cancel: { type: "string", maxLength: 500 },
  },
  compare: {
    period: { type: "string", maxLength: 20, enum: ["weekly", "bi-weekly", "monthly", "quarterly", "semi-annual", "yearly"] },
    currency: { type: "string", maxLength: 3 },
  },
  bulk_operations: {
    action: { type: "string", maxLength: 20, enum: ["status", "delete", "tag_add", "tag_remove"] },
    status: { type: "string", maxLength: 10, enum: ["active", "paused", "cancelled", "archived"] },
    tag_name: { type: "string", maxLength: 50 },
    filter_tag: { type: "string", maxLength: 50 },
    filter_status: { type: "string", maxLength: 10 },
    filter_name: { type: "string", maxLength: 200 },
    confirm: { type: "boolean" },
  },
  get_trials: {
    expiring_soon: { type: "number", min: 0, max: 3_650, integer: true },
  },
  list_tags: {},
  get_tag_subscriptions: {
    tag: { type: "string", maxLength: 200 },
  },
  get_usage_total: {
    from: { type: "string", maxLength: 10 },
    to: { type: "string", maxLength: 10 },
  },
  list_usage: {
    provider: { type: "string", maxLength: 50 },
    from: { type: "string", maxLength: 10 },
    to: { type: "string", maxLength: 10 },
    limit: { type: "number", min: 1, max: 1_000, integer: true },
  },
}
