export type Currency = string

export type Status = "active" | "paused" | "cancelled"

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
  status: Status
  billingDay: number | null
  createdAt: string // YYYY-MM-DD
  notes: string | null
  paymentMethod: string | null
}

export type AddSharedArgs = {
  name: string
  price: number
  currency: Currency
  cycle: Cycle
  tags: string[]
  status?: Status
  billingDay?: number | null
  createdAt?: string // YYYY-MM-DD
  notes?: string | null
  paymentMethod?: string | null
}

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
  minCost?: number
}

export type UsageAddFlags = {
  provider?: string
  model?: string
  inputTokens?: string
  outputTokens?: string
  date?: string
  description?: string
  cost?: string
}

export type UsageImportFlags = {
  file?: string
  dryRun?: boolean
}

export type AddLlmUsageFromLogArgs = AddLlmUsageArgs & {
  generation_id: string
}

export type AddFlags = {
  name?: string
  price?: string
  currency?: string
  cycle?: string
  tags?: string
  billingDay?: string
  status?: string
  notes?: string
  paymentMethod?: string
}

export type UsageRefreshFlags = {
  from?: string
  to?: string
  all?: boolean
}

export type TrialEntry = {
  id: number
  name: string
  expiresAt: string // YYYY-MM-DD
  price: number | null
  currency: string | null
  cycle: string | null
  notes: string | null
  createdAt: string // YYYY-MM-DD
}

export type AddTrialArgs = {
  name: string
  expiresAt: string
  price?: number | null
  currency?: string | null
  cycle?: string | null
  notes?: string | null
}

export type TrialAddFlags = {
  name?: string
  expiresAt?: string
  price?: string
  currency?: string
  cycle?: string
  notes?: string
}

export type BackupFileInfo = {
  name: string
  path: string
  mtime: Date
  size: number
}

export type ProfileFilter = {
  tags?: string[]
  status?: Status
  paymentMethod?: string
}

export type SubtrackConfig = {
  defaultCurrency: string
  monthlyBudget: number
  theme: string
  notifyDays: number
  /** Saved filter profiles */
  profiles?: Record<string, ProfileFilter>
  /** Currently active profile name */
  activeProfile?: string
  /** TUI-specific settings (not shown in CLI config commands) */
  tui?: {
    showTagsCol?: boolean
    showNotesCol?: boolean
    showMethodCol?: boolean
  }
}
