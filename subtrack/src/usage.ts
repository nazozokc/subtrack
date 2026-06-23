import { readFileSync } from "node:fs"
import { input, select, confirm, checkbox, search } from "@inquirer/prompts"
import { consola } from "consola"
import type { UsageAddFlags, UsageImportFlags, UsageRefreshFlags, LlmUsageEntry } from "./types.ts"
import { addLlmUsage, addLlmUsageFromLog, batchAddLlmUsageFromLog, getLlmUsage, deleteLlmUsage } from "./db.ts"
import { runAllScanners } from "./scanner.ts"
import { currentMonthStart, today } from "./date-utils.ts"
import {
  LLM_PROVIDER_CHOICES,
  validateTokens,
  validateDate,
  validateModelName,
} from "./prompts.ts"
import {
  ensurePricingCache,
  searchPricingModels,
  getModelPricing,
  calculateCostCents,
  lookupModelKey,
} from "./pricing.ts"
import { renderUsageTable } from "./display.ts"

// ── Workflow ──────────────────────────────────────────────

async function resolveUsageAddOptions(flags: UsageAddFlags) {
  let manualCostCents: number | null = null
  if (flags.cost !== undefined) {
    const costNum = Number(flags.cost)
    if (isNaN(costNum) || costNum < 0) {
      consola.error("Invalid cost. Enter a non-negative number (e.g. 0.50 for 50 cents)")
      return null
    }
    manualCostCents = Math.round(costNum * 100)
  }

  // Provider
  let provider = flags.provider
  let prompted = false
  if (provider && !LLM_PROVIDER_CHOICES.some((c) => c.value === provider)) {
    consola.error(`Invalid provider "${provider}". Use one of: openai, anthropic, google-ai, mistral, groq, together, deepseek, cohere, or omit for interactive.`)
    return null
  }
  if (!provider) {
    prompted = true
    const picked = await select({
      message: "Provider",
      choices: LLM_PROVIDER_CHOICES,
    })
    if (picked === "__other__") {
      provider = await input({
        message: "Custom provider name",
        validate: (v) => (v.trim() ? true : "Provider name cannot be empty"),
      })
    } else {
      provider = picked
    }
  }

  // Load pricing cache for model search/lookup
  const cache = await ensurePricingCache()

  // Model — search prompt (interactive) or direct lookup (flag)
  let model: string
  let pricing: ReturnType<typeof getModelPricing> = null
  let costCents: number | null = null

  if (flags.model !== undefined) {
    // Non-interactive: look up model in cache
    model = flags.model
    if (cache) {
      const modelKey = lookupModelKey(cache, model, provider)
      if (modelKey) {
        model = modelKey
        pricing = getModelPricing(cache, modelKey)
      }
    }
  } else {
    // Interactive: search prompt
    prompted = true
    if (!cache || Object.keys(cache).length === 0) {
      consola.error("No pricing data available. Cannot look up models.")
      return null
    }

    const selected = await search({
      message: "Search and select a model",
      source: (input) => {
        const q = (input ?? "").trim()
        // Return models filtered by provider from cache
        const results = searchPricingModels(cache!, q, provider === "__other__" ? undefined : provider)
        // Limit to reasonable display
        return results.slice(0, 50)
      },
      pageSize: 15,
    })
    model = selected
    pricing = getModelPricing(cache, model)
  }

  // Input tokens
  let inputTokens: number
  if (flags.inputTokens !== undefined) {
    const result = validateTokens(flags.inputTokens)
    if (result !== true) { consola.error(result); return null }
    inputTokens = Number(flags.inputTokens)
  } else {
    prompted = true
    const str = await input({
      message: "Input tokens used",
      default: "0",
      validate: validateTokens,
    })
    inputTokens = Number(str)
  }

  // Output tokens
  let outputTokens: number
  if (flags.outputTokens !== undefined) {
    const result = validateTokens(flags.outputTokens)
    if (result !== true) { consola.error(result); return null }
    outputTokens = Number(flags.outputTokens)
  } else {
    prompted = true
    const str = await input({
      message: "Output tokens used",
      default: "0",
      validate: validateTokens,
    })
    outputTokens = Number(str)
  }

  // Date
  let date: string
  const today = new Date().toISOString().split("T")[0]
  if (flags.date !== undefined) {
    const result = validateDate(flags.date)
    if (result !== true) { consola.error(result); return null }
    date = flags.date
  } else {
    date = today
  }

  // Description
  let description: string | null = flags.description ?? null
  if (flags.description === undefined && prompted) {
    const str = await input({
      message: "Description (optional)",
      default: "",
    })
    description = str.trim() || null
  }

  // Cost calculation
  let manualCost = false
  if (pricing) {
    costCents = calculateCostCents(pricing, inputTokens, outputTokens)
  }

  if (costCents === null) {
    // Fallback: --cost flag or manual input
    if (manualCostCents !== null) {
      costCents = manualCostCents
      manualCost = true
    } else if (!prompted) {
      consola.error(
        `Could not find pricing for "${model}". Provide --cost to set cost manually (e.g. --cost 0.50 for 50 cents).`,
      )
      return null
    } else {
      consola.warn(
        `Could not find pricing for "${model}" in LiteLLM data. Enter cost manually.`,
      )
      const costStr = await input({
        message: "Total cost in USD (e.g. 0.50 for 50 cents)",
        validate: (v) =>
          !isNaN(Number(v)) && Number(v) >= 0 ? true : "Enter a valid non-negative number",
      })
      costCents = Math.round(Number(costStr) * 100)
      manualCost = true
    }
  }

  // Confirm (only when interactive)
  if (prompted) {
    const costDisplay = manualCost
      ? `$${((costCents ?? 0) / 100).toFixed(2)} (manual)`
      : `$${((costCents ?? 0) / 100).toFixed(4)}`
    consola.info(
      `Cost: ${costDisplay} (${(inputTokens ?? 0).toLocaleString()} in / ${(outputTokens ?? 0).toLocaleString()} out)`,
    )
    const ok = await confirm({
      message: `Save this usage entry (${provider}, ${model})?`,
      default: true,
    })
    if (!ok) {
      consola.info("Cancelled")
      return null
    }
  }

  return {
    provider,
    model,
    input_tokens: inputTokens,
    output_tokens: outputTokens,
    cost: costCents ?? 0,
    date,
    description,
  }
}

// ── Handlers ──────────────────────────────────────────────

export async function handleUsageAdd(flags: UsageAddFlags) {
  const result = await resolveUsageAddOptions(flags)
  if (!result) return
  try {
    addLlmUsage(result)
    consola.success(
      `Added usage: ${result.provider}/${result.model} — $${(result.cost / 100).toFixed(4)} on ${result.date}`,
    )
  } catch (error) {
    consola.error("Failed to add usage entry:", error)
  }
}

export async function handleUsageList(options: { provider?: string; from?: string; to?: string }) {
  const entries = getLlmUsage({
    provider: options.provider,
    from: options.from,
    to: options.to,
    limit: 100,
    minCost: 0.01,
  })

  renderUsageTable(entries)
}

export async function handleUsageDelete(ids?: number[]) {
  if (ids && ids.length > 0) {
    for (const id of ids) {
      const deleted = deleteLlmUsage(id)
      if (deleted) {
        consola.success(`Deleted usage entry: ${id}`)
      } else {
        consola.error(`Usage entry with id ${id} not found`)
      }
    }
    return
  }

  const all = getLlmUsage({ limit: 500 })

  if (all.length === 0) {
    consola.info("No usage entries found")
    return
  }

  const selected = await checkbox({
    message: "Select usage entries to delete",
    choices: all.map((e) => ({
      name: `${e.date}  ${e.provider}/${e.model}  ${e.input_tokens.toLocaleString()} in / ${e.output_tokens.toLocaleString()} out  $${(e.cost / 100).toFixed(4)}${e.description ? `  — ${e.description}` : ""}`,
      value: e,
    })),
    loop: false,
    pageSize: 15,
  })

  if (selected.length === 0) {
    consola.info("Cancelled")
    return
  }

  const ok = await confirm({
    message: `Delete ${selected.length} usage entr${selected.length > 1 ? "ies" : "y"}?`,
    default: false,
  })

  if (!ok) {
    consola.info("Cancelled")
    return
  }

  for (const entry of selected) {
    deleteLlmUsage(entry.id)
    consola.success(`Deleted: ${entry.provider}/${entry.model} (${entry.date})`)
  }
}

// ── Log import ─────────────────────────────────────────────

type ParsedLogEntry = {
  generationId: string
  provider: string
  model: string
  inputTokens: number
  outputTokens: number
  costCents: number | null // null = needs pricing lookup
  date: string
}

function today(): string {
  return new Date().toISOString().split("T")[0]
}

function unixTsToDate(ts: number | undefined): string {
  if (!ts) return today()
  return new Date(ts * 1000).toISOString().split("T")[0]
}

/**
 * Try to parse a single API response JSON object and extract usage info.
 * Returns null if the object doesn't contain recognizable usage data.
 */
function parseResponseJson(obj: Record<string, unknown>): ParsedLogEntry | null {
  const usage = obj.usage
  if (!usage || typeof usage !== "object") return null

  const id = obj.id
  const model = obj.model
  if (typeof id !== "string" || !id) return null
  if (typeof model !== "string" || !model) return null

  const u = usage as Record<string, unknown>

  // Detect OpenRouter: has usage.cost
  const isOpenRouter = typeof u.cost === "number"
  // Detect OpenAI-style: has usage.prompt_tokens
  const isOpenAi = !isOpenRouter && typeof u.prompt_tokens === "number"
  // Detect Anthropic-style: has usage.input_tokens (but not prompt_tokens)
  const isAnthropic = !isOpenRouter && !isOpenAi && typeof u.input_tokens === "number"

  if (isOpenRouter) {
    // Model format: "openai/gpt-4o" or "anthropis/claude-sonnet-4" or "openai/gpt-4o-2024-08-06"
    const slashIdx = model.indexOf("/")
    const provider = slashIdx >= 0 ? model.slice(0, slashIdx) : "unknown"
    const modelName = slashIdx >= 0 ? model.slice(slashIdx + 1) : model
    const inputTokens = Number(u.prompt_tokens ?? 0)
    const outputTokens = Number(u.completion_tokens ?? 0)

    // OpenRouter cost is already in cents (1 credit = $0.01 = 1¢)
    const costCents = Number(u.cost)

    if (inputTokens === 0 && outputTokens === 0 && costCents === 0) return null

    return {
      generationId: id,
      provider,
      model: modelName,
      inputTokens,
      outputTokens,
      costCents,
      date: today(),
    }
  }

  if (isOpenAi) {
    const inputTokens = Number(u.prompt_tokens ?? 0)
    const outputTokens = Number(u.completion_tokens ?? 0)
    if (inputTokens === 0 && outputTokens === 0) return null

    return {
      generationId: id,
      provider: "openai",
      model,
      inputTokens,
      outputTokens,
      costCents: null,
      date: unixTsToDate(obj.created as number | undefined),
    }
  }

  if (isAnthropic) {
    const inputTokens = Number(u.input_tokens ?? 0)
    const outputTokens = Number(u.output_tokens ?? 0)
    if (inputTokens === 0 && outputTokens === 0) return null

    return {
      generationId: id,
      provider: "anthropic",
      model,
      inputTokens,
      outputTokens,
      costCents: null,
      date: today(),
    }
  }

  return null
}

type ImportResult = {
  added: number
  skipped: number
  noCost: number
  errors: number
}

export async function handleUsageImport(flags: UsageImportFlags) {
  const filePath = flags.file
  if (!filePath) {
    consola.error("Usage: subtrack usage import <file> [--dry-run]")
    return
  }

  // Read file (or stdin)
  let content: string
  if (filePath === "-") {
    // Read from stdin
    const chunks: Buffer[] = []
    for await (const chunk of process.stdin) {
      chunks.push(Buffer.from(chunk))
    }
    content = Buffer.concat(chunks).toString("utf-8")
  } else {
    try {
      content = readFileSync(filePath, "utf-8")
    } catch (err) {
      consola.error(`Cannot read file: ${filePath} — ${String(err)}`)
      return
    }
  }

  if (!content.trim()) {
    consola.warn("File is empty")
    return
  }

  // Parse file content into individual JSON objects
  let objects: Record<string, unknown>[]
  const trimmed = content.trim()

  if (trimmed.startsWith("[")) {
    // JSON array
    try {
      const parsed = JSON.parse(trimmed)
      if (!Array.isArray(parsed)) {
        consola.error("File contains a JSON object, expected an array or JSONL")
        return
      }
      objects = parsed as Record<string, unknown>[]
    } catch {
      consola.error("Failed to parse JSON array")
      return
    }
  } else {
    // JSONL: one JSON per line
    objects = []
    const lines = content.split(/\r?\n/)
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i].trim()
      if (!line) continue
      try {
        objects.push(JSON.parse(line) as Record<string, unknown>)
      } catch {
        consola.warn(`Line ${i + 1}: Failed to parse JSON, skipping`)
      }
    }
  }

  if (objects.length === 0) {
    consola.warn("No JSON objects found in file")
    return
  }

  consola.info(`Found ${objects.length} JSON object${objects.length !== 1 ? "s" : ""} in file`)

  // Load pricing cache for cost calculation
  const cache = flags.dryRun ? null : await ensurePricingCache()

  const result: ImportResult = { added: 0, skipped: 0, noCost: 0, errors: 0 }

  for (let i = 0; i < objects.length; i++) {
    const entry = parseResponseJson(objects[i])
    if (!entry) {
      result.errors++
      continue
    }

    // Resolve cost
    let costCents = entry.costCents
    if (costCents === null) {
      // Try pricing cache
      if (cache) {
        const modelKey = lookupModelKey(cache, entry.model)
        if (modelKey) {
          const pricing = getModelPricing(cache, modelKey)
          if (pricing) {
            costCents = calculateCostCents(pricing, entry.inputTokens, entry.outputTokens)
          }
        }
      }
      if (costCents === null || costCents === undefined) {
        // Can't determine cost: skip
        consola.warn(
          `Entry ${i + 1} (${entry.generationId}): Cannot determine cost for "${entry.provider}/${entry.model}", skipping`,
        )
        result.noCost++
        continue
      }
    }

    if (flags.dryRun) {
      result.added++
      continue
    }

    // Write to DB (dedup by generation_id)
    const added = addLlmUsageFromLog({
      provider: entry.provider,
      model: entry.model,
      input_tokens: entry.inputTokens,
      output_tokens: entry.outputTokens,
      cost: costCents,
      date: entry.date,
      description: null,
      generation_id: entry.generationId,
    })

    if (added) {
      result.added++
    } else {
      result.skipped++
    }
  }

  // Report
  if (flags.dryRun) {
    consola.info(
      `Dry run: ${result.added} entries would be added` +
      (result.skipped > 0 ? `, ${result.skipped} duplicates` : "") +
      (result.noCost > 0 ? `, ${result.noCost} skipped (no cost data)` : "") +
      (result.errors > 0 ? `, ${result.errors} unparseable entries` : ""),
    )
  } else {
    consola.success(
      `Imported: ${result.added} entries added` +
      (result.skipped > 0 ? `, ${result.skipped} duplicates skipped` : "") +
      (result.noCost > 0 ? `, ${result.noCost} skipped (no cost data)` : "") +
      (result.errors > 0 ? `, ${result.errors} unparseable entries` : ""),
    )
  }
}

// ── Refresh (auto-scan known sources) ──────────────────────

export async function handleUsageRefresh(flags: UsageRefreshFlags = {}) {
  const from = flags.all ? undefined : (flags.from ?? currentMonthStart())
  const to = flags.all ? undefined : (flags.to ?? today())

  const result = runAllScanners(from, to)

  if (result.entries.length === 0) {
    consola.info("No new usage entries found")
    return
  }

  // Resolve cost via pricing for entries that don't have cost data
  const cache = await ensurePricingCache()
  let resolvedCount = 0
  for (const entry of result.entries) {
    if (entry.cost > 0) continue // already has cost (e.g. from OpenCode)

    if (!cache) continue

    const modelKey = lookupModelKey(cache, entry.model, entry.provider)
    if (!modelKey) continue

    const pricing = getModelPricing(cache, modelKey)
    if (!pricing) continue

    const costCents = calculateCostCents(pricing, entry.input_tokens, entry.output_tokens)
    if (costCents > 0) {
      entry.cost = costCents
      resolvedCount++
    }
  }

  if (resolvedCount > 0) {
    consola.info(`Estimated cost for ${resolvedCount} entr${resolvedCount === 1 ? "y" : "ies"} via pricing`)
  }

  const { added, skipped } = batchAddLlmUsageFromLog(result.entries)

  if (added === 0 && skipped === 0) {
    consola.info("No new usage entries found")
  } else if (added > 0) {
    const periodInfo = from && to ? ` (${from} to ${to})` : ""
    consola.success(
      `Refreshed: ${added} entr${added === 1 ? "y" : "ies"} added` +
      (skipped > 0 ? `, ${skipped} duplicate${skipped === 1 ? "" : "s"} skipped` : "") +
      periodInfo,
    )
  } else {
    consola.info(`All ${skipped} entr${skipped === 1 ? "y" : "ies"} already imported (no new data)`)
  }
}
