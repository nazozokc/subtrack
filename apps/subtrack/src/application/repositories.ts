import {
  addLlmUsageFromLog,
  batchAddLlmUsageFromLog,
  deleteLlmUsage,
  deleteSubscription,
  deleteTrial,
  findSubscriptionByName,
  getAllPriceChanges,
  getAllTags,
  deleteTag,
  addAuditLog,
  getAuditLogCount,
  getAuditLogs,
  pruneAuditLogs,
  collectStats,
  dismissAllSuggestions,
  dismissSuggestion,
  getPendingSuggestionCount,
  getSuggestion,
  getSuggestions,
  markSuggestionAsAdded,
  pruneTags,
  writeSuggestion,
  writeSuggestionBatch,
  getDbPath,
  getTagsWithCount,
  mergeTag,
  renameTag,
  getLlmUsage,
  getLlmUsageTokenTotal,
  getLlmUsageTotal,
  getLlmUsageTotalByModel,
  getLlmUsageTotalByProvider,
  getNonCancelledSubscriptions,
  getPriceHistory,
  getSubscription,
  getSubscriptions,
  searchSubscriptions,
  getTrial,
  getTrials,
  getTrialsExpiringSoon,
  mapTags,
  mergeSubscriptions,
  addLlmUsage,
  saveDb,
  unarchiveSubscription,
  updateLlmUsage,
  updateSubscription,
  archiveSubscription,
  writePriceHistory,
  writeSubscription,
  writeTrial,
} from "../db.ts"
import type {
  AuditRepository,
  SearchRepository,
  StatsRepository,
  DatabaseInfo,
  PriceHistoryRepository,
  SuggestionRepository,
  SubscriptionRepository,
  TagRepository,
  TrialRepository,
  UsageRepository,
} from "./ports.ts"

// ── Unit of work ──────────────────────────────────────────────────────────
//
// The database file is encrypted and rewritten wholesale on every flush, so a
// bulk edit must persist once at the end rather than per row. Inside
// `withBatch`, `write()` hands the db function `persist: false` and records
// that this unit of work now owes a flush; the outermost batch pays it. A
// write outside any batch is flushed immediately by the db function itself.
//
// The two halves travel together on purpose: a function that only skipped the
// save without marking dirty would lose the write, and one that only marked
// dirty without skipping the save would flush twice. Use `write()`, never
// `persistOpt()` and `markDirty()` separately.
let batchDepth = 0
let batchDirty = false

/**
 * Run one write against the db layer, participating in the current batch.
 *
 * `mutated` decides whether the batch now owes a flush. It defaults to "the
 * result is not `false`", which covers the two shapes the db layer returns:
 * `boolean` (false = nothing matched) and `void` (the write happened). Callers
 * that return a count pass `(n) => n > 0` so an empty prune is not a rewrite.
 */
function write<T>(
  fn: (options: { persist: boolean }) => T,
  mutated: (result: T) => boolean = (result) => result !== false,
): T {
  const inBatch = batchDepth > 0
  const result = fn({ persist: !inBatch })
  if (inBatch && mutated(result)) batchDirty = true
  return result
}

export function withBatch<T>(fn: () => T): T {
  batchDepth++
  let callbackError: unknown
  try {
    return fn()
  } catch (error) {
    callbackError = error
    throw error
  } finally {
    batchDepth--
    if (batchDepth === 0 && batchDirty) {
      batchDirty = false
      try {
        saveDb()
      } catch (flushError) {
        // Swallowing is deliberate here. The callback's error is already on its
        // way out, and throwing from `finally` would replace it with an
        // encryption or disk error, hiding the actual cause. The flush failure
        // is attached instead so it is still reachable.
        if (callbackError instanceof Error && callbackError.cause === undefined) {
          callbackError.cause = flushError
        } else if (callbackError === undefined) {
          throw flushError
        }
      }
    }
  }
}

export const databaseInfo: DatabaseInfo = { path: getDbPath }

export const searchRepository: SearchRepository = { find: searchSubscriptions }

export const statsRepository: StatsRepository = { snapshot: collectStats }

export const auditRepository: AuditRepository = {
  list: (options) => getAuditLogs(options),
  count: (options) => getAuditLogCount(options),
  record: (args) => write((o) => addAuditLog(args, o), (inserted) => inserted),
  prune: (before) => write((o) => pruneAuditLogs(before, o), (n) => n > 0),
}

export const suggestionRepository: SuggestionRepository = {
  list: getSuggestions,
  get: getSuggestion,
  pendingCount: getPendingSuggestionCount,
  record: (data) => write((o) => writeSuggestion(data, o)),
  recordBatch: (entries) => write((o) => writeSuggestionBatch(entries, o), (n) => n > 0),
  markAdded: (suggestionId, subscriptionId) =>
    write((o) => markSuggestionAsAdded(suggestionId, subscriptionId, o)),
  dismiss: (id) => write((o) => dismissSuggestion(id, o)),
  dismissAll: () => write((o) => dismissAllSuggestions(o), (n) => n > 0),
}

export const tagRepository: TagRepository = {
  list: getAllTags,
  listWithCount: getTagsWithCount,
  rename: (from, to) => write((o) => renameTag(from, to, o)),
  remove: (name) => write((o) => deleteTag(name, o)),
  merge: (source, target) => write((o) => mergeTag(source, target, o)),
  prune: () => write((o) => pruneTags(o), (n) => n > 0),
}

// ── Adapters ──────────────────────────────────────────────────────────────

export const subscriptionRepository: SubscriptionRepository = {
  list: (options) => getSubscriptions(options),
  listActive: getNonCancelledSubscriptions,
  get: getSubscription,
  findByName: findSubscriptionByName,
  withTags: mapTags,
  add: (data) => write((o) => writeSubscription(data, o)),
  update: (id, fields) => write((o) => updateSubscription(id, fields, o)),
  remove: (id) => write((o) => deleteSubscription(id, o)),
  archive: (id) => write((o) => archiveSubscription(id, o)),
  unarchive: (id) => write((o) => unarchiveSubscription(id, o)),
  merge: (keepId, removeId) => write((o) => mergeSubscriptions(keepId, removeId, o)),
}

export const usageRepository: UsageRepository = {
  list: getLlmUsage,
  add: (data) => write((o) => addLlmUsage(data, o)),
  update: (id, fields) => write((o) => updateLlmUsage(id, fields, o)),
  addFromLog: (data) => write((o) => addLlmUsageFromLog(data, o)),
  addBatch: (entries) =>
    write((o) => batchAddLlmUsageFromLog(entries, o), (r) => r.added > 0),
  remove: (id) => write((o) => deleteLlmUsage(id, o)),
  totalCost: getLlmUsageTotal,
  totalTokens: getLlmUsageTokenTotal,
  totalCostByProvider: getLlmUsageTotalByProvider,
  totalCostByModel: getLlmUsageTotalByModel,
}

export const trialRepository: TrialRepository = {
  list: getTrials,
  get: getTrial,
  listExpiringSoon: getTrialsExpiringSoon,
  add: (data) => write((o) => writeTrial(data, o)),
  remove: (id) => write((o) => deleteTrial(id, o)),
}

export const priceHistoryRepository: PriceHistoryRepository = {
  record: (subscriptionId, oldPrice, newPrice, oldCurrency, newCurrency) =>
    write((o) => writePriceHistory(subscriptionId, oldPrice, newPrice, oldCurrency, newCurrency, o)),
  listForSubscription: getPriceHistory,
  listRecent: getAllPriceChanges,
}
