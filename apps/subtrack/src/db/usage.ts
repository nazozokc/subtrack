import type { SQLInputValue } from "node:sqlite"
import { execObj, execObjs, getDb, saveDb } from "./connection.ts"
import type { PersistOptions } from "./connection.ts"
import type { LlmUsageEntry, AddLlmUsageArgs, AddLlmUsageFromLogArgs, GetLlmUsageOptions } from "../types.ts"

export const addLlmUsage = (data: AddLlmUsageArgs, options: PersistOptions = {}): void => {
  const db = getDb()
  db.prepare(
    `INSERT INTO llm_usage (provider, model, input_tokens, output_tokens, cost, date, description)
     VALUES (?, ?, ?, ?, ?, ?, ?)`,
  ).run(
    data.provider,
    data.model,
    data.input_tokens,
    data.output_tokens,
    data.cost,
    data.date,
    data.description,
  )
  if (options.persist !== false) saveDb()
}

/** Add from log import if not duplicate (by generation_id). Returns true if added, false if duplicate. */
export const addLlmUsageFromLog = (data: AddLlmUsageFromLogArgs, options: PersistOptions = {}): boolean => {
  const db = getDb()

  // Dedup: skip if generation_id already exists
  if (data.generation_id) {
    const existing = execObj<{ id: number }>(
      db,
      "SELECT id FROM llm_usage WHERE generation_id = ?",
      [data.generation_id],
    )
    if (existing) return false
  }

  db.prepare(
    `INSERT INTO llm_usage (provider, model, input_tokens, output_tokens, cost, date, description, generation_id)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
  ).run(
    data.provider,
    data.model,
    data.input_tokens,
    data.output_tokens,
    data.cost,
    data.date,
    data.description,
    data.generation_id ?? null,
  )
  if (options.persist !== false) saveDb()
  return true
}

/**
 * Batch add usage entries from log sources.
 * Checks all generation_ids upfront, inserts new entries in a single transaction.
 * Much faster than calling addLlmUsageFromLog individually for each entry.
 */
export const batchAddLlmUsageFromLog = (
  entries: AddLlmUsageFromLogArgs[],
  options: PersistOptions = {},
): { added: number; skipped: number } => {
  if (entries.length === 0) return { added: 0, skipped: 0 }

  const db = getDb()

  // Collect existing generation_ids for dedup
  const existing = new Set<string>()
  const rows = execObjs<{ generation_id: string | null }>(
    db,
    "SELECT generation_id FROM llm_usage WHERE generation_id IS NOT NULL",
  )
  for (const row of rows) {
    if (row.generation_id) existing.add(row.generation_id)
  }

  let added = 0
  let skipped = 0

  db.exec("BEGIN TRANSACTION")
  try {
    for (const entry of entries) {
      if (existing.has(entry.generation_id)) {
        skipped++
        continue
      }
      existing.add(entry.generation_id)

      db.prepare(
        `INSERT INTO llm_usage (provider, model, input_tokens, output_tokens, cost, date, description, generation_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
      ).run(
        entry.provider,
        entry.model,
        entry.input_tokens,
        entry.output_tokens,
        entry.cost,
        entry.date,
        entry.description,
        entry.generation_id,
      )
      added++
    }
    db.exec("COMMIT")
    if (options.persist !== false) saveDb()
  } catch (error) {
    db.exec("ROLLBACK")
    throw error
  }

  return { added, skipped }
}

export const getLlmUsage = (options?: GetLlmUsageOptions): LlmUsageEntry[] => {
  const db = getDb()

  const conditions: string[] = []
  const params: SQLInputValue[] = []

  if (options?.provider) {
    conditions.push("provider = ?")
    params.push(options.provider)
  }
  if (options?.from) {
    conditions.push("date >= ?")
    params.push(options.from)
  }
  if (options?.to) {
    conditions.push("date <= ?")
    params.push(options.to)
  }
  if (options?.minCost !== undefined) {
    conditions.push("cost >= ?")
    params.push(options.minCost)
  }

  const where = conditions.length > 0 ? `WHERE ${conditions.join(" AND ")}` : ""
  const limitClause = options?.limit !== undefined ? ` LIMIT ?` : ""
  if (options?.limit !== undefined) params.push(options.limit)
  const offsetClause = options?.offset !== undefined ? ` OFFSET ?` : ""
  if (options?.offset !== undefined) params.push(options.offset)

  // SQLite requires LIMIT before OFFSET — emit LIMIT -1 (unlimited) when only an offset is given
  const pagination = limitClause
    ? `${limitClause}${offsetClause}`
    : offsetClause
      ? ` LIMIT -1${offsetClause}`
      : ""

  return execObjs<LlmUsageEntry>(
    db,
    `SELECT id, provider, model, input_tokens, output_tokens, cost, date, description
     FROM llm_usage ${where} ORDER BY date DESC, id DESC${pagination}`,
    params,
  )
}

export const deleteLlmUsage = (id: number, options: PersistOptions = {}): boolean => {
  const db = getDb()
  const { changes } = db.prepare("DELETE FROM llm_usage WHERE id = ?").run(id)
  const modified = Number(changes) > 0
  if (modified && options.persist !== false) saveDb()
  return modified
}

/** Update fields of a usage entry. Returns false if the entry does not exist. */
export const updateLlmUsage = (id: number, fields: Partial<AddLlmUsageArgs>, options: PersistOptions = {}): boolean => {
  const db = getDb()
  const allowed: (keyof AddLlmUsageArgs)[] = [
    "provider",
    "model",
    "input_tokens",
    "output_tokens",
    "cost",
    "date",
    "description",
  ]
  const sets: string[] = []
  const params: SQLInputValue[] = []
  for (const key of allowed) {
    if (fields[key] !== undefined) {
      sets.push(`${key} = ?`)
      params.push(fields[key] as SQLInputValue)
    }
  }
  if (sets.length === 0) return false

  params.push(id)
  const { changes } = db.prepare(`UPDATE llm_usage SET ${sets.join(", ")} WHERE id = ?`).run(...params)
  const modified = Number(changes) > 0
  if (modified && options.persist !== false) saveDb()
  return modified
}

/** Sum `cost` for all entries whose `date` falls within [from, to]. Returns USD cents. */
export const getLlmUsageTotal = (from: string, to: string): number => {
  const db = getDb()
  const row = execObj<{ total: number }>(
    db,
    "SELECT COALESCE(SUM(cost), 0) AS total FROM llm_usage WHERE date >= ? AND date <= ?",
    [from, to],
  )
  return row?.total ?? 0
}

/** Sum input/output tokens for all entries within [from, to]. */
export const getLlmUsageTokenTotal = (
  from: string,
  to: string,
): { inputTokens: number; outputTokens: number } => {
  const db = getDb()
  const row = execObj<{ inputTokens: number; outputTokens: number }>(
    db,
    `SELECT COALESCE(SUM(input_tokens), 0) AS inputTokens,
            COALESCE(SUM(output_tokens), 0) AS outputTokens
     FROM llm_usage WHERE date >= ? AND date <= ?`,
    [from, to],
  )
  return { inputTokens: row?.inputTokens ?? 0, outputTokens: row?.outputTokens ?? 0 }
}

/** Get the sum of `cost` grouped by provider for a date range. */
export const getLlmUsageTotalByProvider = (
  from: string,
  to: string,
): { provider: string; total: number }[] => {
  const db = getDb()
  return execObjs<{ provider: string; total: number }>(
    db,
    `SELECT provider, SUM(cost) AS total
     FROM llm_usage
     WHERE date >= ? AND date <= ?
     GROUP BY provider
     ORDER BY total DESC`,
    [from, to],
  )
}

/** Get cost + token totals grouped by model for a date range. */
export const getLlmUsageTotalByModel = (
  from: string,
  to: string,
): { model: string; provider: string; total: number; inputTokens: number; outputTokens: number }[] => {
  const db = getDb()
  return execObjs<{ model: string; provider: string; total: number; inputTokens: number; outputTokens: number }>(
    db,
    `SELECT model, MAX(provider) AS provider, SUM(cost) AS total,
            SUM(input_tokens) AS inputTokens, SUM(output_tokens) AS outputTokens
     FROM llm_usage
     WHERE date >= ? AND date <= ?
     GROUP BY model
     ORDER BY total DESC`,
    [from, to],
  )
}
