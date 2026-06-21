import { consola } from "consola"
import { existsSync, statSync, readFileSync } from "node:fs"
import { writeSubscription } from "./db.ts"
import { validateName, validatePrice, validateTags, isValidCurrency, isValidCycle } from "./prompts.ts"

const MAX_CSV_SIZE = 10 * 1024 * 1024 // 10 MB

// ── CSV Parser ────────────────────────────────────────────

export function parseCsvLine(line: string): string[] {
  const fields: string[] = []
  let current = ""
  let inQuotes = false
  for (let i = 0; i < line.length; i++) {
    const ch = line[i]
    if (inQuotes) {
      if (ch === '"') {
        if (i + 1 < line.length && line[i + 1] === '"') {
          current += '"'
          i++
        } else {
          inQuotes = false
        }
      } else {
        current += ch
      }
    } else if (ch === ",") {
      fields.push(current)
      current = ""
    } else if (ch === '"') {
      inQuotes = true
    } else {
      current += ch
    }
  }
  fields.push(current)
  return fields
}

// ── Import Handler ────────────────────────────────────────

export async function handleImport(
  file: string,
  options: { dryRun?: boolean },
) {
  if (!file) {
    consola.error("Usage: subtrack import <file> [--dry-run]")
    return
  }

  if (!existsSync(file)) {
    consola.error(`File not found: ${file}`)
    return
  }

  const st = statSync(file)
  if (st.size > MAX_CSV_SIZE) {
    consola.error(
      `File too large (${(st.size / 1024 / 1024).toFixed(1)} MB). Maximum: ${MAX_CSV_SIZE / 1024 / 1024} MB`,
    )
    return
  }

  const content = readFileSync(file, "utf-8")
  // Strip BOM
  const clean = content.charCodeAt(0) === 0xfeff ? content.slice(1) : content
  const lines = clean.split("\n").map((l) => l.trim()).filter(Boolean)

  if (lines.length < 2) {
    consola.error("CSV file must have a header row and at least one data row")
    return
  }

  // Validate header
  const header = parseCsvLine(lines[0]).map((h) => h.toLowerCase().trim())
  if (header.join(",") !== "name,cycle,tags,price,currency") {
    consola.error(
      `Invalid CSV header. Expected: name,cycle,tags,price,currency`,
    )
    return
  }

  let success = 0
  let failed = 0

  for (let i = 1; i < lines.length; i++) {
    const fields = parseCsvLine(lines[i])
    if (fields.length < 5) {
      consola.warn(`Line ${i + 1}: skipping (expected 5 fields, got ${fields.length})`)
      failed++
      continue
    }

    const [name, cycle, tagsStr, priceStr, currency] = fields

    // Validate
    const nameErr = validateName(name)
    if (nameErr !== true) { consola.warn(`Line ${i + 1}: ${nameErr}`); failed++; continue }

    const priceErr = validatePrice(priceStr)
    if (priceErr !== true) { consola.warn(`Line ${i + 1}: ${priceErr}`); failed++; continue }

    if (!isValidCurrency(currency)) {
      consola.warn(`Line ${i + 1}: invalid currency "${currency}"`)
      failed++
      continue
    }
    if (!isValidCycle(cycle)) {
      consola.warn(`Line ${i + 1}: invalid cycle "${cycle}"`)
      failed++
      continue
    }

    const tags = tagsStr.split(";").map((t) => t.trim()).filter(Boolean)
    const tagsErr = validateTags(tags.join(","))
    if (tagsErr !== true) { consola.warn(`Line ${i + 1}: ${tagsErr}`); failed++; continue }

    if (options.dryRun) {
      consola.info(`[dry-run] Would import: ${name} (${priceStr} ${currency}, ${cycle})`)
      success++
    } else {
      try {
        writeSubscription({
          name: name.trim(),
          price: Number(priceStr),
          currency,
          cycle,
          tags,
        })
        success++
      } catch (e) {
        consola.warn(`Line ${i + 1}: failed to import: ${String(e)}`)
        failed++
      }
    }
  }

  if (options.dryRun) {
    consola.success(`Dry-run complete: ${success} valid, ${failed} invalid`)
  } else {
    consola.success(`Import complete: ${success} imported, ${failed} failed`)
  }
}
