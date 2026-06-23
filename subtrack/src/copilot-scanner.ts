import { readFileSync, existsSync, readdirSync, statSync } from "node:fs"
import { homedir } from "node:os"
import { join } from "node:path"
import { consola } from "consola"
import type { AddLlmUsageFromLogArgs } from "./types.ts"
import type { Scanner, ScanResult } from "./scanner-types.ts"
import { isInDateRange } from "./date-utils.ts"

/**
 * Find Copilot CLI session event files.
 * Located at ~/.copilot/session-state/<session-id>/events.jsonl
 */
function findEventFiles(): string[] {
  const base = join(homedir(), ".copilot", "session-state")
  if (!existsSync(base)) return []

  const files: string[] = []
  try {
    const sessions = readdirSync(base)
    for (const sessionId of sessions) {
      const sessionDir = join(base, sessionId)
      if (!statSync(sessionDir).isDirectory()) continue
      const eventFile = join(sessionDir, "events.jsonl")
      if (existsSync(eventFile)) {
        files.push(eventFile)
      }
    }
  } catch {
    return []
  }
  return files
}

/**
 * Parse a single line from a Copilot events.jsonl file.
 * Returns the extracted usage data or null.
 */
function parseEventLine(
  line: string,
  fileName: string,
): AddLlmUsageFromLogArgs | null {
  let data: Record<string, unknown>
  try {
    data = JSON.parse(line)
  } catch {
    return null
  }

  // Copilot events have a "type" field.
  // Look for completion events with token data.
  const eventType = data.type as string | undefined
  if (!eventType || !eventType.includes("completion") && !eventType.includes("Completions")) return null

  const payload = data.payload as Record<string, unknown> | undefined
  if (!payload) return null

  // Extract token counts — Copilot stores them in various places
  const tokensUsed = (payload.tokensUsed as number) ?? (payload.tokenCount as number) ?? 0
  if (tokensUsed <= 0) return null

  const model = (payload.model as string) ?? (data.model as string) ?? "copilot"
  const provider = "github"

  // Copilot typically only stores total tokens
  const inputTokens = Math.round(tokensUsed * 2 / 3)
  const outputTokens = tokensUsed - inputTokens

  // Extract timestamp
  const ts = (data.timestamp as number) ?? (data.createdAt as number) ?? Date.now()
  // Copilot timestamps are typically in seconds
  const tsMs = ts < 1e12 ? ts * 1000 : ts
  const date = new Date(tsMs).toISOString().split("T")[0]

  // Generate a consistent generation_id
  const eventId = (data.id as string) ?? `${fileName}-${ts}`
  const generationId = `copilot-${eventId}`

  return {
    provider,
    model,
    input_tokens: inputTokens,
    output_tokens: outputTokens,
    cost: 0,
    date,
    description: null,
    generation_id: generationId,
  }
}

/**
 * Scan GitHub Copilot CLI session event files and extract usage entries.
 */
export function scanCopilot(from?: string, to?: string): ScanResult {
  const files = findEventFiles()

  if (files.length === 0) {
    consola.info("Copilot CLI session files not found — skip")
    return { source: "copilot", entries: [] }
  }

  consola.info(`Found ${files.length} Copilot session file${files.length === 1 ? "" : "s"}`)

  const entries: AddLlmUsageFromLogArgs[] = []

  for (const filePath of files) {
    const basename = filePath.split("/").pop() ?? ""

    let content: string
    try {
      content = readFileSync(filePath, "utf-8")
    } catch {
      continue
    }

    const lines = content.split(/\r?\n/)
    for (const line of lines) {
      const trimmed = line.trim()
      if (!trimmed) continue

      const parsed = parseEventLine(trimmed, basename)
      if (parsed && isEntryInDateRange(parsed, from, to)) {
        entries.push(parsed)
      }
    }
  }

  consola.info(`Found ${entries.length} usage entr${entries.length === 1 ? "y" : "ies"} in Copilot CLI sessions`)
  return { source: "copilot", entries }
}

function isEntryInDateRange(entry: { date: string }, from?: string, to?: string): boolean {
  if (from && entry.date < from) return false
  if (to && entry.date > to) return false
  return true
}

/**
 * Create a Scanner instance for GitHub Copilot.
 */
export function createCopilotScanner(): Scanner {
  return {
    name: "copilot",
    scan(from?: string, to?: string): ScanResult {
      return scanCopilot(from, to)
    },
  }
}
