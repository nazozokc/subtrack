import { consola } from "consola"
import pc from "picocolors"
import type { SharedArgs } from "./types.ts"
import { getSubscriptions } from "./db.ts"
import { formatPrice } from "./display.ts"
import { calcSummary } from "./payment.ts"
import { loadConfig } from "./config.ts"
import { periodFactor } from "./types.ts"

export function showAnalytics(): void {
  const list = getSubscriptions().filter((s) => s.status !== "cancelled")
  if (list.length === 0) {
    consola.info("No active subscriptions found")
    return
  }

  const config = loadConfig()
  const data = calcSummary(list)

  // Header
  consola.log(pc.bold("📊 Subscription Analytics"))
  consola.log("")

  // Overview
  consola.log(pc.bold("Overview:"))
  consola.log(`  Total subscriptions:  ${pc.bold(String(data.totalCount))}`)
  consola.log(`  Status breakdown:`)
  const activeCount = list.filter((s) => s.status === "active").length
  const pausedCount = list.filter((s) => s.status === "paused").length
  const cancelledCount = getSubscriptions().filter((s) => s.status === "cancelled").length
  consola.log(`    ${pc.green(`active: ${activeCount}`)}`)
  if (pausedCount > 0) consola.log(`    ${pc.yellow(`paused: ${pausedCount}`)}`)
  if (cancelledCount > 0) consola.log(`    ${pc.red(`cancelled: ${cancelledCount}`)}`)

  if (data.mostExpensive) {
    const me = data.mostExpensive
    consola.log(`  Most expensive:       ${pc.bold(me.name)} (${formatPrice(me.price, me.currency)}/${me.cycle})`)
  }

  // Monthly spending
  consola.log("")
  consola.log(pc.bold("Monthly spending:"))
  for (const [ccy, total] of Object.entries(data.monthlyByCurrency).sort()) {
    consola.log(`  ${ccy}    ${formatPrice(Math.round(total), ccy)}`)
  }

  // Budget
  if (config.monthlyBudget > 0) {
    const defaultCurrency = config.defaultCurrency || "USD"
    // Sum all monthly costs in their original currencies
    const monthlyTotal = list.reduce((sum, sub) => sum + sub.price * periodFactor(sub.cycle, "monthly"), 0)
    // For budget display, use default currency as reference
    const budgetDisplay = formatPrice(config.monthlyBudget, defaultCurrency)
    const spentDisplay = formatPrice(monthlyTotal, "USD")
    consola.log(`  ${pc.dim("─".repeat(30))}`)
    consola.log(`  Budget:     ${pc.bold(budgetDisplay)}`)
    const remaining = config.monthlyBudget - monthlyTotal
    const remainingDisplay = formatPrice(remaining, "USD")
    if (remaining >= 0) {
      consola.log(`  Remaining:  ${pc.green(remainingDisplay)}`)
    } else {
      consola.log(`  Over budget: ${pc.red(remainingDisplay.replace("-", ""))}`)
    }
  }

  // Tags breakdown
  if (Object.keys(data.monthlyByTag).length > 0) {
    consola.log("")
    consola.log(pc.bold("Monthly by tag:"))
    const sorted = Object.entries(data.monthlyByTag).sort((a, b) => b[1].monthly - a[1].monthly)
    for (const [tag, info] of sorted) {
      consola.log(
        `  ${tag.padEnd(16)} ${formatPrice(Math.round(info.monthly), "USD")}/month (${info.count} sub${info.count > 1 ? "s" : ""})`,
      )
    }
  }
}
