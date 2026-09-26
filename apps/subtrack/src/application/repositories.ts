import {
  addLlmUsageFromLog,
  batchAddLlmUsageFromLog,
  deleteLlmUsage,
  deleteSubscription,
  deleteTrial,
  findSubscriptionByName,
  getAllPriceChanges,
  getLlmUsage,
  getLlmUsageTokenTotal,
  getLlmUsageTotal,
  getLlmUsageTotalByModel,
  getLlmUsageTotalByProvider,
  getNonCancelledSubscriptions,
  getPriceHistory,
  getSubscription,
  getSubscriptions,
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
  PriceHistoryRepository,
  SubscriptionRepository,
  TrialRepository,
  UnitOfWork,
  UsageRepository,
} from "./ports.ts"

// ── Unit of work ──────────────────────────────────────────────────────────
//
// The database file is encrypted and rewritten wholesale on every flush, so a
// bulk edit must persist once at the end rather than per row. Writes issued
// inside `withBatch` are given `persist: false` and a single flush happens when
// the outermost batch closes. Handlers therefore never see `saveDb` or
// `persist`; they just say "these writes are one operation".
let batchDepth = 0
let batchDirty = false

const persistOpt = () => ({ persist: batchDepth === 0 })

/** Mark the current unit of work as needing a flush. */
function markDirty(): void {
  if (batchDepth > 0) batchDirty = true
}

export function withBatch<T>(fn: () => T): T {
  batchDepth++
  try {
    return fn()
  } finally {
    batchDepth--
    if (batchDepth === 0 && batchDirty) {
      batchDirty = false
      saveDb()
    }
  }
}

const unitOfWork: UnitOfWork = { batch: withBatch }

// ── Adapters ──────────────────────────────────────────────────────────────

export const subscriptionRepository: SubscriptionRepository = {
  list: (options) => getSubscriptions(options),
  listActive: () => getNonCancelledSubscriptions(),
  get: getSubscription,
  findByName: findSubscriptionByName,
  withTags: mapTags,
  add: (data) => {
    const id = writeSubscription(data, persistOpt())
    markDirty()
    return id
  },
  update: (id, fields) => {
    const ok = updateSubscription(id, fields, persistOpt())
    if (ok) markDirty()
    return ok
  },
  remove: (id) => {
    const ok = deleteSubscription(id, persistOpt())
    if (ok) markDirty()
    return ok
  },
  archive: (id) => {
    const ok = archiveSubscription(id)
    if (ok) markDirty()
    return ok
  },
  unarchive: (id) => {
    const ok = unarchiveSubscription(id)
    if (ok) markDirty()
    return ok
  },
  merge: (keepId, removeId) => {
    // Merging is transactional inside the db layer; it reports false when the
    // source row is already gone, so nothing needs persisting in that case.
    const ok = mergeSubscriptions(keepId, removeId)
    if (ok) markDirty()
    return ok
  },
}

export const usageRepository: UsageRepository = {
  list: getLlmUsage,
  add: (data) => {
    addLlmUsage(data)
    markDirty()
  },
  update: (id, fields) => {
    const ok = updateLlmUsage(id, fields)
    if (ok) markDirty()
    return ok
  },
  addFromLog: (data) => {
    const ok = addLlmUsageFromLog(data)
    if (ok) markDirty()
    return ok
  },
  addBatch: (entries) => {
    const result = batchAddLlmUsageFromLog(entries)
    if (result.added > 0) markDirty()
    return result
  },
  remove: (id) => {
    const ok = deleteLlmUsage(id)
    if (ok) markDirty()
    return ok
  },
  totalCost: getLlmUsageTotal,
  totalTokens: getLlmUsageTokenTotal,
  totalCostByProvider: getLlmUsageTotalByProvider,
  totalCostByModel: getLlmUsageTotalByModel,
}

export const trialRepository: TrialRepository = {
  list: getTrials,
  get: getTrial,
  listExpiringSoon: getTrialsExpiringSoon,
  add: (data) => {
    writeTrial(data)
    markDirty()
  },
  remove: (id) => {
    const ok = deleteTrial(id)
    if (ok) markDirty()
    return ok
  },
}

export const priceHistoryRepository: PriceHistoryRepository = {
  record: (subscriptionId, oldPrice, newPrice, oldCurrency, newCurrency) => {
    writePriceHistory(subscriptionId, oldPrice, newPrice, oldCurrency, newCurrency)
    markDirty()
  },
  listForSubscription: getPriceHistory,
  listRecent: getAllPriceChanges,
}

export { unitOfWork }
