import { homedir } from "node:os"
import { join } from "node:path"
import type { AddLlmUsageFromLogArgs } from "./types.ts"
import { defineScanner, type ScanResult } from "./scanner-types.ts"
import { findExistingPath, scanSqliteKv } from "./scanner-support.ts"
import { safeJsonParse } from "@subtrack/lib/json"
import { estimateTokenSplit } from "@subtrack/lib/date"

/**
 * Known paths for Cursor's state.vscdb across platforms.
 */
function findCursorDb(): string | null {
  const candidates = [
    // Linux
    join(homedir(), ".config", "Cursor", "User", "globalStorage", "state.vscdb"),
    // macOS
    join(homedir(), "Library", "Application Support", "Cursor", "User", "globalStorage", "state.vscdb"),
    // Windows (via WSL or Git Bash)
    join(homedir(), "AppData", "Roaming", "Cursor", "User", "globalStorage", "state.vscdb"),
  ]
  return findExistingPath(candidates)
}

/**
 * Try to extract token usage data from a cursorDiskKV value blob.
 * The value is typically a JSON string with token fields nested inside.
 */
function parseCursorKvValue(key: string, value: string): AddLlmUsageFromLogArgs | null {
  if (!key.startsWith("bubbleId:")) return null

  let data: Record<string, unknown>
  try {
    data = safeJsonParse<Record<string, unknown>>(value)
  } catch {
    return null
  }

  // Cursor stores token data in various nested formats.
  // Common paths:
  //   data.tokensUsed or data.tokens or data.usage?.totalTokens
  //   data.model
  //   data.provider
  const tokensUsed = (data.tokensUsed as number) ?? (data.tokens as number) ?? 0
  if (tokensUsed <= 0) return null

  const model = (data.model as string) ?? "unknown"
  const provider = (data.provider as string) ?? "cursor"

  // Cursor only stores total tokens typically — estimate 2:1 split
  const { inputTokens, outputTokens } = estimateTokenSplit(tokensUsed)

  // Extract timestamp
  const ts = (data.timestamp as number) ?? (data.createdAt as number) ?? (data.time as number) ?? 0
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
    generation_id: `cursor-${key}`,
  }
}

/**
 * Scan Cursor editor's state.vscdb and extract LLM usage entries.
 */
export function scanCursor(from?: string, to?: string): ScanResult {
  const dbPath = findCursorDb()
  return scanSqliteKv("Cursor", dbPath, ["cursorDiskKV", "ItemTable"],
    (table) => `SELECT key, value FROM "${table}" WHERE key LIKE 'bubbleId:%'`,
    parseCursorKvValue, from, to)
}

/**
 * Scanner instance for Cursor.
 */
export const createCursorScanner = defineScanner("cursor", scanCursor)
