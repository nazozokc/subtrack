import type { Cycle } from "@subtrack/lib/date"

export type Currency = string

export type Status = "active" | "paused" | "cancelled" | "archived"

export type { Cycle }

export type DiscountType = "percentage" | "fixed" | null

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
  contractStart: string | null // YYYY-MM-DD
  contractEnd: string | null // YYYY-MM-DD
  autoRenewal: boolean
  vendorName: string | null
  vendorUrl: string | null
  planTier: string | null
  discountAmount: number | null
  discountType: DiscountType
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
  contractStart?: string | null
  contractEnd?: string | null
  autoRenewal?: boolean
  vendorName?: string | null
  vendorUrl?: string | null
  planTier?: string | null
  discountAmount?: number | null
  discountType?: DiscountType
}

export type SubscriptionTemplate = {
  name: string
  price: number
  currency: Currency
  cycle: Cycle
  tags: string[]
  billingDay?: number | null
  notes?: string | null
  paymentMethod?: string | null
  vendorName?: string | null
  vendorUrl?: string | null
  planTier?: string | null
  autoRenewal?: boolean
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
  contractStart?: string
  contractEnd?: string
  autoRenewal?: string
  vendorName?: string
  vendorUrl?: string
  planTier?: string
  discountAmount?: string
  discountType?: string
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

/**
 * Filter and ordering options for subscription queries.
 *
 * Lives here rather than in `db/subscriptions.ts` so the application ports can
 * describe a query without importing from the database layer.
 */
export type SubscriptionQueryOptions = {
  sort?: string
  desc?: boolean
  limit?: number
  offset?: number
  includeArchived?: boolean
  status?: string
  minPrice?: number
  maxPrice?: number
  /** Every listed subscription must carry all of these tags (AND). */
  tags?: string[]
}

/** A recorded price or currency change for a subscription. */
export type PriceHistoryEntry = {
  id: number
  subscriptionId: number
  subscriptionName: string
  oldPrice: number | null
  newPrice: number
  oldCurrency: string | null
  newCurrency: string
  changedAt: string
}

/** Token counts aggregated over a date range. */
export type UsageTokenTotal = {
  inputTokens: number
  outputTokens: number
}

/** Cost in USD cents attributed to one LLM provider. */
export type UsageProviderTotal = {
  provider: string
  total: number
}

/** Cost and token counts attributed to one model. */
export type UsageModelTotal = {
  model: string
  provider: string
  total: number
  inputTokens: number
  outputTokens: number
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

export type ListFlags = {
  currency?: string
  sort?: string
  desc?: boolean
  api?: boolean
  notes?: boolean
  method?: boolean
  tags?: string
  showContract?: boolean
  showVendor?: boolean
  json?: boolean
  status?: string
  all?: boolean
  minPrice?: number
  maxPrice?: number
  limit?: number
}

export type TagListFlags = {
  json?: boolean
  sort?: "name" | "count"
}

export type AnalyticsOptions = {
  json?: boolean
  currency?: string
  period?: "monthly" | "yearly"
}

export type CompareOptions = {
  currency?: string
  api?: boolean
  json?: boolean
}

export type ProfileFilter = {
  tags?: string[]
  status?: Status
  paymentMethod?: string
}

export type BudgetEntry = {
  name: string
  amount: number
  currency: string
  categories?: string[] // filter by tags
  period?: "monthly" | "yearly"
}

export type NotifyChannel = "os" | "slack" | "webhook"

export type SubtrackConfig = {
  defaultCurrency: string
  monthlyBudget: number
  theme: string
  notifyDays: number
  /** ISO datetime of last successful suggestion scan. */
  suggestLastScan?: string
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
  /** Multiple named budgets for budget vs actual tracking */
  budgets?: BudgetEntry[]
  /** Yearly budget target (can be separate from monthlyBudget) */
  yearlyBudget?: number
  /** Notification channels (default: ["os"]) */
  notifyChannels?: NotifyChannel[]
  /** Slack webhook URL */
  slackWebhook?: string
  /** Generic webhook URL for notifications */
  webhookUrl?: string
  /** Display theme preset name (default | light | high-contrast | none) */
  tableBorderColor?: string
  /** Override border color (ColorName) */
  tableHeaderColor?: string
  /** Override table header color (ColorName) */
  tableZebraColor?: string
  /** Override zebra stripe background color (ColorName) */
  accentColor?: string
  /** Override accent color used for headings (ColorName) */
  tableZebra?: "on" | "off"
  /** Enable/disable zebra striping */
  tableMinWidth?: number
  /** Minimum table width in columns (default 40) */
  dateFormat?: "iso" | "short"
  /** Show notes column in `subtrack list` by default */
  listShowNotes?: "on" | "off"
  /** Show payment method column in `subtrack list` by default */
  listShowMethod?: "on" | "off"
  templates?: Record<string, SubscriptionTemplate>
}

// ── Audit ─────────────────────────────────────────────────────────────

export type AuditAction =
  | "subscription.add"
  | "subscription.edit"
  | "subscription.delete"
  | "subscription.archive"
  | "subscription.unarchive"
  | "subscription.restore"
  | "subscription.import"
  | "subscription.bulk_status"
  | "subscription.bulk_delete"
  | "subscription.bulk_tag_add"
  | "subscription.bulk_tag_remove"
  | "subscription.clone"
  | "subscription.merge"
  | "subscription.cancel"
  | "subscription.pause"
  | "subscription.resume"
  | "subscription.renew"
  | "trial.add"
  | "trial.delete"
  | "tag.rename"
  | "tag.delete"
  | "tag.prune"
  | "tag.merge"
  | "config.set"
  | "config.reset"
  | "backup.restore"
  | "usage.add"
  | "usage.edit"
  | "usage.delete"
  | "cleanup"
  | "suggestion.receipt"
  | "template.add"
  | "template.edit"
  | "template.delete"
  | "template.use"

export type AuditEntry = {
  id: number
  action: AuditAction
  target_type: string | null
  target_id: number | null
  details: string | null
  created_at: string
}

export type AddAuditArgs = {
  action: AuditAction
  targetType?: string | null
  targetId?: number | null
  details?: string | null
}

// ── Suggestions ──────────────────────────────────────────────────────

export type SuggestionStatus = "pending" | "dismissed" | "added"

export type SuggestionSource = "email" | "manual"

export type Suggestion = {
  id: number
  name: string
  price: number | null
  currency: string | null
  cycle: string | null
  vendorName: string | null
  vendorUrl: string | null
  planTier: string | null
  paymentMethod: string | null
  source: string
  sourceDetail: string | null
  emailSubject: string | null
  emailFrom: string | null
  emailDate: string | null
  confidence: number
  status: SuggestionStatus
  matchedSubId: number | null
  createdAt: string
}
/** A candidate subscription awaiting review, as handed to persistence. */
export type SuggestionInput = {
  name: string
  price: number | null
  currency: string | null
  cycle: string | null
  vendorName?: string | null
  vendorUrl?: string | null
  planTier?: string | null
  paymentMethod?: string | null
  source: string
  sourceDetail?: string | null
  emailSubject?: string | null
  emailFrom?: string | null
  emailDate?: string | null
  confidence?: number
}

/** Row counts and aggregates rendered by `subtrack stats`. */
export type StatsSnapshot = {
  total: number
  active: number
  paused: number
  cancelled: number
  archived: number
  totalTags: number
  totalTrials: number
  totalUsage: number
  dbSizeBytes: number
  priceRange: { min: number; max: number; currencies: string[] }
}
