import { readFileSync, writeFileSync, mkdirSync, existsSync, statSync } from "node:fs"
import path from "node:path"
import { homedir } from "node:os"
import { safeJsonParse } from "./safe-json.ts"

export type ModelPricingEntry = {
  input_cost_per_token?: number
  output_cost_per_token?: number
  output_cost_per_reasoning_token?: number
  litellm_provider?: string
  mode?: string
  [key: string]: unknown
}

export type PricingCache = Record<string, ModelPricingEntry>

let _cache: PricingCache | null = null
let _cachePromise: Promise<PricingCache | null> | null = null

const CACHE_FRESHNESS_MS = 24 * 60 * 60 * 1000
const FETCH_TIMEOUT_MS = 15_000
const GITHUB_JSON_URL =
  "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json"
const MODEL_CATALOG_API = "https://api.litellm.ai/model_catalog"

function getConfigDir(): string {
  return process.env.SUBSC_CLI_DB_DIR ?? path.join(homedir(), ".config", "subtrack")
}

function getCachePath(): string {
  return path.join(getConfigDir(), "litellm_cache.json")
}

/**
 * Load pricing cache: try fresh on-disk cache first, fall back to GitHub fetch.
 * Returns `null` only when both cache and fetch fail.
 */
export async function ensurePricingCache(): Promise<PricingCache | null> {
  if (_cache) return _cache
  if (_cachePromise) return _cachePromise

  _cachePromise = (async () => {
    const cachePath = getCachePath()
    let staleCache: PricingCache | null = null

    // Try loading existing cache
    if (existsSync(cachePath)) {
      try {
        const data = readFileSync(cachePath, "utf-8")
        staleCache = JSON.parse(data) as PricingCache
        const st = statSync(cachePath)
        const age = Date.now() - st.mtimeMs
        if (age < CACHE_FRESHNESS_MS) {
          _cache = staleCache
          return staleCache
        }
      } catch {
        // corrupt cache — will refetch
      }
    }

    // Fetch from GitHub
    try {
      const controller = new AbortController()
      const timer = setTimeout(() => controller.abort(), FETCH_TIMEOUT_MS)
      let data: PricingCache
      try {
        const res = await fetch(GITHUB_JSON_URL, { signal: controller.signal })
        if (!res.ok) throw new Error(`GitHub responded with ${res.status}`)
        const text = await res.text()
        data = safeJsonParse<PricingCache>(text)
      } finally {
        clearTimeout(timer)
      }
      mkdirSync(getConfigDir(), { recursive: true })
      writeFileSync(cachePath, JSON.stringify(data))
      _cache = data
      return data
    } catch {
      // GitHub unavailable — use stale cache if we have it
      if (staleCache) {
        _cache = staleCache
        return staleCache
      }
      return null
    }
  })()

  return _cachePromise
}

/**
 * Look up a model's pricing in the cached LiteLLM data.
 * Tries several matching strategies from most to least specific.
 */
export function matchModel(
  cache: PricingCache,
  provider: string,
  modelName: string,
): ModelPricingEntry | null {
  const lowerModel = modelName.toLowerCase()

  // 1 — exact match on key
  if (cache[modelName]) return cache[modelName]
  if (cache[lowerModel]) return cache[lowerModel]

  // 2 — with provider prefix
  const prefixes = [
    `${provider}/`,
    `${provider}.`,
  ]
  for (const pfx of prefixes) {
    const k = `${pfx}${modelName}`
    if (cache[k]) return cache[k]
    const kl = `${pfx}${lowerModel}`
    if (cache[kl]) return cache[kl]
  }

  // 3 — narrow to relevant provider entries
  const candidates = Object.entries(cache).filter(
    ([, v]) => v.litellm_provider === provider,
  )
  if (candidates.length === 0) {
    // fall back to full search
    candidates.push(...Object.entries(cache))
  }

  // 4 — exact match on model name in value
  for (const [, v] of candidates) {
    if ((v as Record<string, unknown>).model_name === modelName) return v
  }

  // 5 — substring match on key
  for (const [key, v] of candidates) {
    if (key.includes(lowerModel)) return v
  }

  // 6 — strip common version suffixes
  const suffixes = ["-v1:0", "-v2:0", "-v1", "-v2", "-preview"]
  for (const suffix of suffixes) {
    if (lowerModel.endsWith(suffix)) {
      const base = lowerModel.slice(0, -suffix.length)
      if (cache[base]) return cache[base]
    }
  }

  return null
}

/**
 * Calculate cost in USD cents from token counts and pricing entry.
 * Returns a float (fractional cents are preserved).
 */
export function calculateCostCents(
  pricing: ModelPricingEntry,
  inputTokens: number,
  outputTokens: number,
): number {
  const inputCost = (pricing.input_cost_per_token ?? 0) * inputTokens
  const outputCost = (pricing.output_cost_per_token ?? 0) * outputTokens
  const reasoningCost =
    (pricing.output_cost_per_reasoning_token ?? 0) * outputTokens
  return (inputCost + outputCost + reasoningCost) * 100
}

/**
 * Query the LiteLLM Model Catalog API for a single model's pricing.
 * Used as fallback when the cached data doesn't contain the model.
 */
export async function getModelPricingDirect(
  modelName: string,
): Promise<ModelPricingEntry | null> {
  try {
    const url = `${MODEL_CATALOG_API}/${encodeURIComponent(modelName)}`
    const controller = new AbortController()
    const timer = setTimeout(() => controller.abort(), FETCH_TIMEOUT_MS)
    try {
      const res = await fetch(url, { signal: controller.signal })
      if (!res.ok) return null
      const text = await res.text()
      return safeJsonParse<ModelPricingEntry>(text)
    } finally {
      clearTimeout(timer)
    }
  } catch {
    return null
  }
}

/** Force-refresh the pricing cache (next call to ensurePricingCache re-fetches). */
export async function refreshPricingCache(): Promise<PricingCache | null> {
  _cache = null
  _cachePromise = null
  return ensurePricingCache()
}
