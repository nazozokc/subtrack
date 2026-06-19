import { input, select, confirm, checkbox } from "@inquirer/prompts"
import { consola } from "consola"
import type { UsageAddFlags, LlmUsageEntry } from "./types.ts"
import { addLlmUsage, getLlmUsage, deleteLlmUsage } from "./db.ts"
import {
  LLM_PROVIDER_CHOICES,
  validateTokens,
  validateDate,
  validateModelName,
} from "./prompts.ts"
import {
  ensurePricingCache,
  matchModel,
  calculateCostCents,
  getModelPricingDirect,
  refreshPricingCache,
} from "./pricing.ts"
import type { ModelPricingEntry } from "./pricing.ts"

// ── Workflow ──────────────────────────────────────────────

async function resolveUsageAddOptions(flags: UsageAddFlags) {
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

  // Model
  let model = flags.model
  if (!model) {
    prompted = true
    model = await input({
      message: "Model name (e.g. gpt-4o, claude-3-opus-20240229)",
      validate: validateModelName,
    })
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

  // Pricing lookup
  let pricing: ModelPricingEntry | null = null
  let costCents: number | null = null
  let manualCost = false

  const cache = await ensurePricingCache()
  if (cache) {
    pricing = matchModel(cache, provider, model)
  }
  if (!pricing) {
    pricing = await getModelPricingDirect(model)
  }
  if (pricing) {
    costCents = calculateCostCents(pricing, inputTokens, outputTokens)
  }

  if (costCents === null) {
    // Fallback: ask user for manual cost
    if (!prompted) {
      consola.error(
        `Could not find pricing for "${model}". Run interactively to enter cost manually.`,
      )
      return null
    }
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
  })

  if (entries.length === 0) {
    consola.info("No usage entries found")
    return
  }

  // Build table
  const rows: string[][] = []
  for (const e of entries) {
    rows.push([
      e.provider,
      e.model,
      e.input_tokens.toLocaleString(),
      e.output_tokens.toLocaleString(),
      `$${(e.cost / 100).toFixed(4)}`,
      e.date,
      e.description ?? "",
    ])
  }

  const totalCost = entries.reduce((sum, e) => sum + e.cost, 0)

  // Simple table output
  const header = `${"Provider".padEnd(12)} ${"Model".padEnd(30)} ${"Input".padEnd(12)} ${"Output".padEnd(12)} ${"Cost".padEnd(12)} ${"Date".padEnd(12)} Description`
  consola.log(header)
  consola.log("─".repeat(Math.max(header.length, 60)))
  for (const row of rows) {
    consola.log(
      `${row[0].padEnd(12)} ${row[1].padEnd(30)} ${row[2].padEnd(12)} ${row[3].padEnd(12)} ${row[4].padEnd(12)} ${row[5].padEnd(12)} ${row[6]}`,
    )
  }
  consola.log("")
  consola.log(`Total: $${(totalCost / 100).toFixed(2)} (${entries.length} entr${entries.length === 1 ? "y" : "ies"})`)
}

export async function handleUsageDelete() {
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

export async function handleUsageRefresh() {
  consola.info("Refreshing LiteLLM pricing cache...")
  const result = await refreshPricingCache()
  if (result) {
    consola.success(`Pricing cache updated (${Object.keys(result).length} models)`)
  } else {
    consola.fail("Failed to fetch pricing data")
  }
}
