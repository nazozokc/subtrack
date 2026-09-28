import { existsSync } from "node:fs"
import { homedir } from "node:os"
import { join } from "node:path"
import { consola } from "@subtrack/lib/logger"
import { DatabaseSync } from "node:sqlite"
import type { SQLInputValue } from "node:sqlite"
import type { AddLlmUsageFromLogArgs } from "./types.ts"
import { defineScanner, type ScanResult } from "./scanner-types.ts"
import { dateToStartOfDayMs, dateToEndOfDayMs, estimateTokenSplit } from "@subtrack/lib/date"

const STATE_DB_PATH = join(homedir(), ".codex", "state_5.sqlite")

/**
 * Scan Codex CLI database and extract LLM usage entries.
 */
export function scanCodexCli(from?: string, to?: string): ScanResult {
  if (!existsSync(STATE_DB_PATH)) {
    consola.info("Codex CLI DB not found — skip")
    return { source: "codex", entries: [] }
  }

  consola.info(`Reading Codex CLI DB: ${STATE_DB_PATH}`)

  let db: DatabaseSync | null = null
  const entries: AddLlmUsageFromLogArgs[] = []

  try {
    db = new DatabaseSync(STATE_DB_PATH, { readOnly: true })

    let sql = `SELECT id, tokens_used, model, model_provider, created_at_ms FROM threads WHERE tokens_used > 0`
    const params: SQLInputValue[] = []
    if (from) { sql += ` AND created_at_ms >= ?`; params.push(dateToStartOfDayMs(from)) }
    if (to) { sql += ` AND created_at_ms <= ?`; params.push(dateToEndOfDayMs(to)) }

    const rows = db.prepare(sql).all(...params) as unknown as {
      id: string
      tokens_used: number
      model: string
      model_provider: string
      created_at_ms: number
    }[]

    if (rows.length === 0) {
      consola.info("No usage data found in Codex CLI DB")
      return { source: "codex", entries: [] }
    }

    for (const row of rows) {
      const threadId = String(row.id)
      const tokensUsed = Number(row.tokens_used ?? 0)
      const model = String(row.model ?? "unknown")
      const provider = String(row.model_provider ?? "unknown")
      const createdAtMs = Number(row.created_at_ms ?? 0)

      if (tokensUsed <= 0) continue

      const { inputTokens, outputTokens } = estimateTokenSplit(tokensUsed)

      const date = createdAtMs > 0
        ? new Date(createdAtMs).toISOString().split("T")[0]
        : new Date().toISOString().split("T")[0]

      entries.push({
        provider: provider !== "unknown" ? provider : "codex",
        model,
        input_tokens: inputTokens,
        output_tokens: outputTokens,
        cost: 0,
        date,
        description: null,
        generation_id: `codex-${threadId}`,
      })
    }
  } catch (err) {
    consola.warn(`Error scanning Codex CLI DB: ${String(err)}`)
    return { source: "codex", entries: [] }
  } finally {
    if (db) db.close()
  }

  consola.info(`Found ${entries.length} usage entr${entries.length === 1 ? "y" : "ies"} in Codex CLI DB`)
  return { source: "codex", entries }
}

/**
 * Scanner instance for Codex CLI.
 */
export const createCodexScanner = defineScanner("codex", scanCodexCli)
