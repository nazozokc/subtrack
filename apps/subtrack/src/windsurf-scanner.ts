import { homedir } from "node:os"
import { join } from "node:path"
import type { AddLlmUsageFromLogArgs } from "./types.ts"
import { defineScanner, type ScanResult } from "./scanner-types.ts"
import { safeJsonParse } from "@subtrack/lib/json"
import { estimateTokenSplit } from "@subtrack/lib/date"
import { findExistingPath, scanSqliteKv } from "./scanner-support.ts"

/**
 * Known paths for Windsurf's state.vscdb across platforms.
 */
function findWindsurfDb(): string | null {
  const candidates = [
    // Linux
    join(homedir(), ".config", "Windsurf", "User", "globalStorage", "state.vscdb"),
    // macOS
    join(homedir(), "Library", "Application Support", "Windsurf", "User", "globalStorage", "state.vscdb"),
    // Windows (via WSL or Git Bash)
    join(homedir(), "AppData", "Roaming", "Windsurf", "User", "globalStorage", "state.vscdb"),
  ]
  return findExistingPath(candidates)
}

/**
 * Try to extract token usage data from a windsurfDiskKV value blob.
 */
function parseWindsurfKvValue(key: string, value: string): AddLlmUsageFromLogArgs | null {
  // Windsurf stores metrics under various key prefixes
  if (!key.includes("token") && !key.includes("usage") && !key.includes("completion")) return null

  let data: Record<string, unknown>
  try {
    data = safeJsonParse<Record<string, unknown>>(value)
  } catch {
    return null
  }

  const tokensUsed = (data.tokensUsed as number) ?? (data.tokens as number) ?? (data.totalTokens as number) ?? 0
  if (tokensUsed <= 0) return null

  const model = (data.model as string) ?? "unknown"
  const provider = "windsurf"

  const { inputTokens, outputTokens } = estimateTokenSplit(tokensUsed)

  const ts = (data.timestamp as number) ?? (data.createdAt as number) ?? 0
  const date = ts > 0
    ? new Date(ts).toISOString().split("T")[0]
    : new Date().toISOString().split("T")[0]

  return {
    provider,
    model,
    input_tokens: inputTokens,
    output_tokens: outputTokens,
    cost: 0,
    date,
    description: null,
    generation_id: `windsurf-${key}`,
  }
}

/**
 * Scan Windsurf editor's state.vscdb and extract LLM usage entries.
 */
export function scanWindsurf(from?: string, to?: string): ScanResult {
  const dbPath = findWindsurfDb()
  return scanSqliteKv("Windsurf", dbPath, ["windsurfDiskKV", "ItemTable"],
    (table) => `SELECT key, value FROM "${table}" WHERE key LIKE '%token%' OR key LIKE '%usage%' OR key LIKE '%completion%'`,
    parseWindsurfKvValue, from, to)
}

/**
 * Scanner instance for Windsurf.
 */
export const createWindsurfScanner = defineScanner("windsurf", scanWindsurf)
