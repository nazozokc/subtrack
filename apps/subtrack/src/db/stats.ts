/**
 * Aggregate row counts for the `stats` command.
 *
 * Each count is its own query rather than one big join: the tables are small,
 * and separate statements keep the result stable when a table is missing.
 */

import { getFileSize } from "@subtrack/lib/format"
import { getDb, getDbPath, execObjs } from "./connection.ts"

import type { Status, StatsSnapshot } from "../types.ts"

const countOf = (table: string): number => {
  const rows = execObjs<{ count: number }>(getDb(), `SELECT COUNT(*) AS count FROM ${table}`)
  return rows.length > 0 ? Number(rows[0].count) : 0
}

export const collectStats = (): StatsSnapshot => {
  const db = getDb()

  const statusRows = execObjs<{ status: Status; count: number }>(
    db,
    "SELECT status, COUNT(*) AS count FROM subscriptions GROUP BY status",
  )
  const statusMap = new Map(statusRows.map((r) => [r.status, Number(r.count)]))

  const priceRows = execObjs<{ price: number; currency: string }>(
    db,
    "SELECT price, currency FROM subscriptions WHERE status = 'active'",
  )
  const currencies = [...new Set(priceRows.map((r) => r.currency))]

  return {
    total: statusRows.reduce((a, r) => a + Number(r.count), 0),
    active: statusMap.get("active") ?? 0,
    paused: statusMap.get("paused") ?? 0,
    cancelled: statusMap.get("cancelled") ?? 0,
    archived: statusMap.get("archived") ?? 0,
    totalTags: countOf("tags"),
    totalTrials: countOf("trials"),
    totalUsage: countOf("llm_usage"),
    dbSizeBytes: getFileSize(getDbPath()),
    priceRange: {
      min: priceRows.length > 0 ? Math.min(...priceRows.map((r) => Number(r.price))) : 0,
      max: priceRows.length > 0 ? Math.max(...priceRows.map((r) => Number(r.price))) : 0,
      currencies,
    },
  }
}
