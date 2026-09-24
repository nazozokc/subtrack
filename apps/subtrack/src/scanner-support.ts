import { existsSync } from "node:fs"
import { DatabaseSync } from "node:sqlite"
import { consola } from "@subtrack/lib/logger"
import { isDateInRange } from "@subtrack/lib/date"
import type { AddLlmUsageFromLogArgs } from "./types.ts"
import type { ScanResult } from "./scanner-types.ts"

type KvRow = Readonly<{ key: string; value: string }>

function readKvRows(db: DatabaseSync, query: string): KvRow[] {
  return db.prepare(query).all() as unknown as KvRow[]
}

/** Find the first existing path from platform-specific candidates. */
export function findExistingPath(candidates: readonly string[]): string | null {
  return candidates.find((candidate) => existsSync(candidate)) ?? null
}

/** Scan an editor's SQLite key-value table and delegate row interpretation. */
export function scanSqliteKv(
  source: string,
  dbPath: string | null,
  tableNames: readonly string[],
  rowQuery: (tableName: string) => string,
  parseRow: (key: string, value: string) => AddLlmUsageFromLogArgs | null,
  from?: string,
  to?: string,
): ScanResult {
  if (!dbPath) {
    consola.info(`${source} DB not found — skip`)
    return { source, entries: [] }
  }

  consola.info(`Reading ${source} DB: ${dbPath}`)
  const entries: AddLlmUsageFromLogArgs[] = []
  let db: DatabaseSync | null = null

  try {
    db = new DatabaseSync(dbPath, { readOnly: true })
    const tables = db.prepare("SELECT name FROM sqlite_master WHERE type='table'").all() as unknown as ReadonlyArray<{ name: string }>
    const availableTables = new Set(tables.map((table) => String(table.name)))
    const tableName = tableNames.find((name) => availableTables.has(name))
    if (!tableName) {
      consola.info(`No known ${source} KV table found`)
      return { source, entries: [] }
    }

    const rows = readKvRows(db, rowQuery(tableName))
    for (const row of rows) {
      const parsed = parseRow(String(row.key ?? ""), String(row.value ?? ""))
      if (parsed && isDateInRange(parsed.date, from, to)) entries.push(parsed)
    }
  } catch (err) {
    consola.warn(`Error scanning ${source} DB: ${String(err)}`)
    return { source, entries: [] }
  } finally {
    db?.close()
  }

  consola.info(`Found ${entries.length} usage entr${entries.length === 1 ? "y" : "ies"} in ${source} DB`)
  return { source, entries }
}
