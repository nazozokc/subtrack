import { suggestionRepository } from "./application/index.ts"
import { readFileSync, statSync } from "node:fs"
import { consola } from "@subtrack/lib/logger"
import { fail } from "./error.ts"
import { logAudit } from "./audit-log.ts"
import { parseEmail } from "./suggest/parser/index.ts"
import { parseCsvLine } from "./import-csv.ts"
import { handleSuggestReview } from "./suggest/suggest.ts"
import type { RawEmail } from "./suggest/types.ts"

const MAX_RECEIPT_SIZE = 5 * 1024 * 1024 // 5 MB — prevents memory exhaustion on huge files
const MAX_RECEIPT_ROWS = 10_000 // max data rows to prevent DoS

function readReceipts(file: string): RawEmail[] {
  let st
  try {
    st = statSync(file)
  } catch {
    throw new Error(`Cannot read file: ${file}`)
  }
  if (st.size > MAX_RECEIPT_SIZE) {
    throw new Error(
      `File too large (${(st.size / 1024 / 1024).toFixed(1)} MB). Maximum: ${MAX_RECEIPT_SIZE / 1024 / 1024} MB`,
    )
  }
  const text = readFileSync(file, "utf8")
  if (file.toLowerCase().endsWith(".json")) {
    const parsed = JSON.parse(text) as unknown
    const rows = Array.isArray(parsed) ? parsed : [parsed]
    if (rows.length > MAX_RECEIPT_ROWS) {
      throw new Error(`JSON file has ${rows.length} entries (max ${MAX_RECEIPT_ROWS})`)
    }
    return rows.map((value, index) => { const row = value as Record<string, unknown>; return { id: String(index), textBody: String(row.body ?? row.text ?? ""), subject: row.subject ? String(row.subject) : null, from: row.from ? String(row.from) : null, date: row.date ? new Date(String(row.date)) : null } })
  }
  if (file.toLowerCase().endsWith(".csv")) {
    const lines = text.split(/\r?\n/).slice(1).filter(Boolean)
    if (lines.length > MAX_RECEIPT_ROWS) {
      throw new Error(`CSV file has ${lines.length} data rows (max ${MAX_RECEIPT_ROWS})`)
    }
    return lines.map((line) => { const [body, subject, from, date] = parseCsvLine(line); return { id: line, textBody: body ?? "", subject: subject ?? null, from: from ?? null, date: date ? new Date(date) : null } })
  }
  return [{ id: file, textBody: text, subject: null, from: null, date: null }]
}

/** Parse a local receipt file into pending subscription suggestions. */
export async function handleReceipt(file: string, options: { dryRun?: boolean; review?: boolean; json?: boolean } = {}): Promise<void> {
  let candidates
  try { candidates = readReceipts(file).map(parseEmail).filter((candidate): candidate is NonNullable<ReturnType<typeof parseEmail>> => !!candidate) }
  catch (error) { fail(`Failed to read receipt: ${error instanceof Error ? error.message : String(error)}`); return }
  // json and dryRun are read-only previews: no writes, no console noise mixed into stdout
  if (options.json || options.dryRun) { process.stdout.write(JSON.stringify(candidates, null, 2) + "\n"); return }
  if (!candidates.length) { consola.info("No receipt candidates found"); return }
  suggestionRepository.recordBatch(candidates)
  logAudit("suggestion.receipt", { details: `${candidates.length} receipt candidate(s) imported` })
  consola.success(`Imported ${candidates.length} receipt candidate(s)`)
  if (options.review) await handleSuggestReview()
}
