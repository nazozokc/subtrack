import type {
  AddAuditArgs,
  AddLlmUsageArgs,
  AddLlmUsageFromLogArgs,
  AddSharedArgs,
  AddTrialArgs,
  AuditEntry,
  GetLlmUsageOptions,
  LlmUsageEntry,
  PriceHistoryEntry,
  SharedArgs,
  StatsSnapshot,
  SubscriptionQueryOptions,
  Suggestion,
  SuggestionInput,
  SuggestionStatus,
  TrialEntry,
  UsageModelTotal,
  UsageProviderTotal,
  UsageTokenTotal,
} from "../types.ts"

/**
 * Application-facing persistence contracts. Implementations stay in
 * infrastructure (`repositories.ts`).
 *
 * Ports speak domain language: a caller asks for "active subscriptions" or
 * "price changes in the last N days", never for a SQL string. Anything a port
 * cannot express cleanly is a sign the query belongs in the adapter, not that
 * the caller should reach past the boundary.
 */
export interface SubscriptionRepository {
  list(options?: SubscriptionQueryOptions): SharedArgs[]
  /** Subscriptions that are not cancelled — the default set for money totals. */
  listActive(): SharedArgs[]
  get(id: number): SharedArgs | undefined
  findByName(name: string): SharedArgs | undefined
  /** Attach a tag array to each row (tags live in a side table). */
  withTags(subs: SharedArgs[]): SharedArgs[]
  add(data: AddSharedArgs): number
  update(id: number, fields: Partial<AddSharedArgs>): boolean
  remove(id: number): boolean
  archive(id: number): boolean
  unarchive(id: number): boolean
  /** Fold `removeId` into `keepId`. Returns false when either is missing. */
  merge(keepId: number, removeId: number): boolean
}

export interface UsageRepository {
  list(options?: GetLlmUsageOptions): LlmUsageEntry[]
  add(data: AddLlmUsageArgs): void
  update(id: number, fields: Partial<AddLlmUsageArgs>): boolean
  addFromLog(data: AddLlmUsageFromLogArgs): boolean
  addBatch(entries: AddLlmUsageFromLogArgs[]): { added: number; skipped: number }
  remove(id: number): boolean
  /** Total cost in USD cents over an inclusive `YYYY-MM-DD` range. */
  totalCost(from: string, to: string): number
  totalTokens(from: string, to: string): UsageTokenTotal
  totalCostByProvider(from: string, to: string): UsageProviderTotal[]
  totalCostByModel(from: string, to: string): UsageModelTotal[]
}

export interface TrialRepository {
  list(): TrialEntry[]
  get(id: number): TrialEntry | undefined
  /** Trials whose end date falls within `days` of today. */
  listExpiringSoon(days: number): TrialEntry[]
  add(data: AddTrialArgs): void
  remove(id: number): boolean
}

/** Which columns a free-text search should look at. Unset means "all". */
export type SearchFields = {
  names?: boolean
  notes?: boolean
  tags?: boolean
}

export interface SearchRepository {
  /** Subscriptions whose name, notes, or tags contain `query`. */
  find(query: string, fields: SearchFields): SharedArgs[]
}

export interface StatsRepository {
  /** Row counts and aggregates for the `stats` command. */
  snapshot(): StatsSnapshot
}

export interface AuditQuery {
  action?: string
  from?: string
  to?: string
  limit?: number
  offset?: number
}

export interface AuditRepository {
  list(options?: AuditQuery): AuditEntry[]
  count(options?: AuditQuery): number
  /** Append one entry. */
  record(args: AddAuditArgs): void
  /** Drop entries older than `before`. Returns rows removed. */
  prune(before: string): number
}

export interface SuggestionRepository {
  list(status?: SuggestionStatus): Suggestion[]
  get(id: number): Suggestion | undefined
  pendingCount(): number
  record(data: SuggestionInput): void
  /** Insert many, skipping ones already pending. Returns rows inserted. */
  recordBatch(entries: SuggestionInput[]): number
  markAdded(suggestionId: number, subscriptionId: number): void
  dismiss(id: number): boolean
  dismissAll(): number
}

export interface TagRepository {
  /** Every tag in use, alphabetically. */
  list(): string[]
  listWithCount(): { name: string; count: number }[]
  rename(oldName: string, newName: string): boolean
  remove(name: string): boolean
  /** Fold `source` into `target`. */
  merge(source: string, target: string): boolean
  /** Drop tags no longer attached to any subscription. Returns rows removed. */
  prune(): number
}

export interface PriceHistoryRepository {
  /** Record a price (or currency) change for a subscription. */
  record(
    subscriptionId: number,
    oldPrice: number | null,
    newPrice: number,
    oldCurrency: string | null,
    newCurrency: string,
  ): void
  listForSubscription(subscriptionId: number): PriceHistoryEntry[]
  /** All changes, optionally limited to the last `days`. */
  listRecent(days?: number): PriceHistoryEntry[]
}

/**
 * Read-only access to the database file's own location. Callers that display a
 * path (menus, diagnostics) should not need a handle on the connection.
 */
export interface DatabaseInfo {
  path(): string
}
