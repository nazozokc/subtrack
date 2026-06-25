// ── Re-exports from domain modules ──────────────────────

export { handleList, handleAdd, handleDelete, handleEdit, handleTags } from "./subscription.ts"
export { handleBackup, handleRestore } from "./backup.ts"
export { handleTagList, handleTagRename, handleTagDelete, handleTagPrune } from "./tag.ts"

// ── Export handler ──────────────────────────────────────

import { consola } from "consola"
import { writeFileSync } from "node:fs"
import type { Currency, Cycle, SubtrackConfig } from "./types.ts"
import { tagsSubscription, getSubscriptions } from "./db.ts"
import { formatPrice } from "./display.ts"
import { showPayment, showSummary } from "./payment.ts"
import { showUpcoming } from "./upcoming.ts"
import { showAnalytics } from "./analytics.ts"
import { exportCsv, exportMd, exportJson } from "./export.ts"
import { fetchFxRates, convertPrice } from "./fx.ts"
import { loadConfig, setConfig, resetConfig, CONFIG_KEYS, getConfigPath } from "./config.ts"
import { unlinkSync, existsSync } from "node:fs"
import os from "node:os"
import { resolveSafeOutputPath } from "./path-utils.ts"

export async function handleExport(
  format: string,
  options: { currency?: string; tags?: string; output?: string },
) {
  if (format !== "csv" && format !== "json" && format !== "md") {
    consola.error(`Unsupported export format: "${format}". Supported: csv, json, md`)
    return
  }

  let list = options.tags
    ? tagsSubscription(options.tags.split(",").map((t) => t.trim()))
    : getSubscriptions()

  if (list.length === 0) {
    consola.info("No subscriptions found")
    return
  }

  if (options.currency) {
    try {
      const rates = await fetchFxRates()
      const targetCurrency = options.currency as Currency
      list = list.map((sub) => ({
        ...sub,
        price: Math.round(
          convertPrice(sub.price, sub.currency, targetCurrency, rates.rates),
        ),
        currency: targetCurrency,
      }))
    } catch (e) {
      consola.fail(
        `Failed to fetch exchange rates; exporting in original currencies: ${String(e)}`,
      )
    }
  }

  const content = format === "csv" ? exportCsv(list) : format === "json" ? exportJson(list) : exportMd(list)
  if (options.output) {
    const safePath = resolveSafeOutputPath([os.homedir(), os.tmpdir()], options.output)
    if (!safePath) {
      consola.error(`Invalid output path — must be within home directory`)
      return
    }
    writeFileSync(safePath, content, { mode: 0o600 })
    consola.success(`Exported to: ${safePath}`)
  } else {
    consola.log(content)
  }
}

// ── Thin CLI wrappers ───────────────────────────────────

export async function handlePayment(
  period: Cycle,
  options: { currency?: string; api?: boolean },
) {
  await showPayment(period, options.currency as Currency | undefined, undefined, options.api)
}

export async function handleSummary() {
  await showSummary()
}

export function handleUpcoming(days?: number): void {
  showUpcoming(days)
}

export function handleAnalytics(): void {
  showAnalytics()
}

// ── Config handlers ─────────────────────────────────────

export function handleConfigList(): void {
  const config = loadConfig()
  for (const key of CONFIG_KEYS) {
    consola.log(`${key}: ${config[key]}`)
  }
}

export function handleConfigGet(key: string): void {
  const config = loadConfig()
  if (!(CONFIG_KEYS as readonly string[]).includes(key)) {
    consola.error(`Unknown config key: "${key}". Valid: ${CONFIG_KEYS.join(", ")}`)
    return
  }
  consola.log(`${key}: ${config[key as keyof SubtrackConfig]}`)
}

export function handleConfigSet(key: string, value: string): void {
  if (!(CONFIG_KEYS as readonly string[]).includes(key)) {
    consola.error(`Unknown config key: "${key}". Valid: ${CONFIG_KEYS.join(", ")}`)
    return
  }
  setConfig(key as (typeof CONFIG_KEYS)[number], value)
}

export async function handleConfigReset(): Promise<void> {
  const configPath = getConfigPath()
  if (existsSync(configPath)) {
    try {
      unlinkSync(configPath)
    } catch (err) {
      consola.error(`Failed to remove config file: ${err instanceof Error ? err.message : String(err)}`)
      return
    }
  }
  resetConfig()
  consola.success("Config reset to defaults")
}
