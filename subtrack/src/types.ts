export type Currency = string

export type Cycle =
  | "weekly" | "bi-weekly" | "monthly"
  | "quarterly" | "semi-annual" | "yearly"

export const OCCURRENCES_PER_YEAR: Record<Cycle, number> = {
  weekly: 52,
  "bi-weekly": 26,
  monthly: 12,
  quarterly: 4,
  "semi-annual": 2,
  yearly: 1,
}

/**
 * Returns the multiplier to convert a price from one cycle to another.
 * e.g. periodFactor("yearly", "monthly") => 1/12
 *      periodFactor("monthly", "yearly") => 12
 */
export function periodFactor(from: Cycle, to: Cycle = "monthly"): number {
  return OCCURRENCES_PER_YEAR[from] / OCCURRENCES_PER_YEAR[to]
}

export type SharedArgs = {
  id: number
  name: string
  price: number
  currency: Currency
  cycle: Cycle
  tags: string[]
}

export type AddSharedArgs = Omit<SharedArgs, "id">

export type LlmUsageEntry = {
  id: number
  provider: string
  model: string
  input_tokens: number
  output_tokens: number
  cost: number // USD cents (float, may include fractional)
  date: string // YYYY-MM-DD
  description: string | null
}

export type AddLlmUsageArgs = Omit<LlmUsageEntry, "id">

export type GetLlmUsageOptions = {
  provider?: string
  from?: string
  to?: string
  limit?: number
  offset?: number
}

export type UsageAddFlags = {
  provider?: string
  model?: string
  inputTokens?: string
  outputTokens?: string
  date?: string
  description?: string
}

export type AddFlags = {
  name?: string
  price?: string
  currency?: string
  cycle?: string
  tags?: string
}
