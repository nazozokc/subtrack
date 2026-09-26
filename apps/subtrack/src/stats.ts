/**
 * Database statistics command.
 */

import { consola } from "@subtrack/lib/logger"
import { sectionTitle } from "./display-constants.ts"
import { statsRepository } from "./application/index.ts"
import { formatBytes } from "@subtrack/lib/format"

export function handleStats(options: { json?: boolean } = {}): void {
  const stats = statsRepository.snapshot()

  if (options.json) {
    process.stdout.write(JSON.stringify(stats, null, 2) + "\n")
    return
  }

  consola.log(sectionTitle("Database Statistics"))
  consola.log(`  Subscriptions: ${stats.total}`)
  consola.log(`    Active:       ${stats.active}`)
  consola.log(`    Paused:       ${stats.paused}`)
  consola.log(`    Cancelled:    ${stats.cancelled}`)
  consola.log(`    Archived:     ${stats.archived}`)
  consola.log(`  Tags:           ${stats.totalTags}`)
  consola.log(`  Trials:         ${stats.totalTrials}`)
  consola.log(`  LLM Usage entries: ${stats.totalUsage}`)
  if (stats.priceRange.currencies.length > 0) {
    consola.log(`  Active price range:`)
    consola.log(`    Min:  ${stats.priceRange.min} (${stats.priceRange.currencies.join(", ")})`)
    consola.log(`    Max:  ${stats.priceRange.max} (${stats.priceRange.currencies.join(", ")})`)
  }
  consola.log(`  Database size:   ${formatBytes(stats.dbSizeBytes)}`)
}
