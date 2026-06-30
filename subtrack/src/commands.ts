// ── Re-exports from domain modules ──────────────────────

export { handleList, handleAdd, handleDelete, handleEdit, handleTags } from "./subscription.ts"
export { handleSearch } from "./search.ts"
export { handleTrialAdd, handleTrialList, handleTrialExpiring, handleTrialDelete } from "./trial.ts"
export { handleBulkStatus, handleBulkDelete, handleBulkTagAdd, handleBulkTagRemove } from "./bulk.ts"
export { handleForecast } from "./forecast.ts"
export { handleBackup, handleRestore } from "./backup.ts"
export { handleTagList, handleTagRename, handleTagDelete, handleTagPrune } from "./tag.ts"

// Lazy import for TUI to avoid loading Ink/React/yoga-layout WASM in tests
export async function handleTui(): Promise<void> {
  const { handleTui: tui } = await import("./tui.tsx")
  return tui()
}

// ── Export handler ──────────────────────────────────────

import { consola } from "consola"
import { writeFileSync } from "node:fs"
import type { Currency, Cycle, SubtrackConfig } from "./types.ts"
import { tagsSubscription, getSubscriptions } from "./db.ts"
import { formatPrice } from "./display.ts"
import { showPayment, showSummary, calcSummary } from "./payment.ts"
import { showUpcoming, calcUpcoming } from "./upcoming.ts"
import { showAnalytics } from "./analytics.ts"
import { showCompare } from "./compare.ts"
import { exportCsv, exportMd, exportJson, exportExcel, exportIcs } from "./export.ts"
import { showCalendar, calcCalendarEntries } from "./calendar.ts"
import { fetchFxRates, convertPrice } from "./fx.ts"
import { loadConfig, setConfig, resetConfig, CONFIG_KEYS, getConfigPath } from "./config.ts"
import { unlinkSync, existsSync } from "node:fs"
import os from "node:os"
import { resolveSafeOutputPath } from "./path-utils.ts"
import type { SharedArgs } from "./types.ts"
import { periodFactor } from "./types.ts"

export async function handleExport(
  format: string,
  options: { currency?: string; tags?: string; output?: string },
) {
  const supported = ["csv", "json", "md", "excel", "ics"] as const
  if (!(supported as readonly string[]).includes(format)) {
    consola.error(`Unsupported export format: "${format}". Supported: ${supported.join(", ")}`)
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

  if (format === "excel") {
    const buf = await exportExcel(list)
    if (options.output) {
      const safePath = resolveSafeOutputPath([os.homedir(), os.tmpdir()], options.output)
      if (!safePath) {
        consola.error(`Invalid output path — must be within home directory`)
        return
      }
      writeFileSync(safePath, buf, { mode: 0o600 })
      consola.success(`Exported to: ${safePath}`)
    } else {
      process.stdout.write(buf)
    }
    return
  }

  const content = format === "csv"
    ? exportCsv(list)
    : format === "json"
      ? exportJson(list)
      : format === "md"
        ? exportMd(list)
        : exportIcs(list)

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

export type JsonOptions = { json?: boolean }

export async function handlePayment(
  period: Cycle,
  options: { currency?: string; api?: boolean; method?: boolean } & JsonOptions,
) {
  if (options.json) {
    const subs = getSubscriptions()
    const data = subs.map((sub) => ({
      id: sub.id,
      name: sub.name,
      price: sub.price,
      currency: sub.currency,
      cycle: sub.cycle,
      convertedPrice: Math.round(sub.price * periodFactor(sub.cycle, period)),
      status: sub.status,
    }))
    process.stdout.write(JSON.stringify(data, null, 2) + "\n")
    return
  }
  await showPayment(period, options.currency as Currency | undefined, undefined, options.api, options.method)
}

export async function handleSummary(options: JsonOptions = {}) {
  if (options.json) {
    const subs = getSubscriptions()
    const data = calcSummary(subs)
    process.stdout.write(JSON.stringify(data, null, 2) + "\n")
    return
  }
  await showSummary()
}

export function handleUpcoming(days: number = 7, options: JsonOptions = {}): void {
  if (options.json) {
    const entries = calcUpcoming(days)
    const data = entries.map((e) => ({
      id: e.sub.id,
      name: e.sub.name,
      price: e.sub.price,
      currency: e.sub.currency,
      cycle: e.sub.cycle,
      nextDate: `${e.nextDate.getFullYear()}-${String(e.nextDate.getMonth() + 1).padStart(2, "0")}-${String(e.nextDate.getDate()).padStart(2, "0")}`,
      amount: Math.round(e.amount),
      tags: e.sub.tags,
    }))
    process.stdout.write(JSON.stringify(data, null, 2) + "\n")
    return
  }
  showUpcoming(days)
}

export function handleAnalytics(): void {
  showAnalytics()
}

export async function handleCompare(
  period: Cycle,
  options: { currency?: string; api?: boolean },
): Promise<void> {
  await showCompare(period, options)
}

// ── Calendar handler ────────────────────────────────────

export function handleCalendar(options: { month?: number; year?: number; json?: boolean }): void {
  showCalendar(options)
}

// ── MCP handler ─────────────────────────────────────────

export async function handleMcp(): Promise<void> {
  const { startMcpServer } = await import("./mcp.ts")
  await startMcpServer()
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
