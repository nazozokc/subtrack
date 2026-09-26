/**
 * Usage total command — aggregated LLM API usage summary.
 */

import { usageRepository } from "./application/index.ts"
import { consola } from "@subtrack/lib/logger"
import pc from "@subtrack/lib/ansi"
import { sectionTitle, divider } from "./display-constants.ts"
import { getPeriodDateRange } from "@subtrack/lib/date"
import type { NamedCycle } from "@subtrack/lib/date"
import { formatUsdCost } from "./price.ts"
import { isValidNamedCycle } from "./validation.ts"
import { fail } from "./error.ts"

export type UsageTotalOptions = {
  from?: string
  to?: string
  period?: NamedCycle
  json?: boolean
}

export function handleUsageTotal(options: UsageTotalOptions = {}): void {
  if (options.period !== undefined && !isValidNamedCycle(options.period)) {
    fail("period must be one of: weekly, bi-weekly, monthly, quarterly, semi-annual, yearly")
    return
  }

  let from: string
  let to: string

  if (options.from && options.to) {
    from = options.from
    to = options.to
  } else {
    const range = getPeriodDateRange(options.period ?? "monthly")
    from = range.from
    to = range.to
  }

  const total = usageRepository.totalCost(from, to)
  const byProvider = usageRepository.totalCostByProvider(from, to)
  const byModel = usageRepository.totalCostByModel(from, to)
  const tokens = usageRepository.totalTokens(from, to)

  if (options.json) {
    process.stdout.write(JSON.stringify({
      from,
      to,
      total,
      tokens,
      byProvider,
      byModel,
    }, null, 2) + "\n")
    return
  }

  if (total <= 0) {
    consola.info(`No API usage found from ${from} to ${to}`)
    return
  }

  consola.log(sectionTitle(`LLM API Usage (${from} → ${to})`))
  consola.log(pc.bold("  By provider:"))
  for (const p of byProvider) {
    consola.log(`    ${p.provider}: ${formatUsdCost(p.total, 2)}`)
  }
  if (byModel.length > 0) {
    consola.log(pc.bold("  By model:"))
    for (const m of byModel) {
      consola.log(
        `    ${m.model}: ${formatUsdCost(m.total, 2)} ` +
        `(${m.inputTokens.toLocaleString()} in / ${m.outputTokens.toLocaleString()} out)`,
      )
    }
  }
  consola.log(`  ${divider(20)}`)
  consola.log(
    `  Tokens: ${tokens.inputTokens.toLocaleString()} in / ${tokens.outputTokens.toLocaleString()} out`,
  )
  consola.log(`  Total: ${pc.bold(pc.yellow(formatUsdCost(total, 2)))}`)
}