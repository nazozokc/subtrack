import { checkbox, confirm } from "@inquirer/prompts"
import { consola } from "consola"
import type { LlmUsageEntry } from "./types.ts"
import { getLlmUsage, deleteLlmUsage } from "./db.ts"
import { renderUsageTable } from "./display.ts"

export { handleUsageAdd } from "./usage-add.ts"
export { handleUsageImport } from "./usage-import.ts"
export { handleUsageRefresh } from "./usage-refresh.ts"

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

  // Model
  let model = flags.model
  if (!model) {
    prompted = true
    model = await input({
      message: "Model name (e.g. gpt-4o, claude-3-opus-20240229)",
      validate: validateModelName,
    })
  }

  // Input tokens (from flags only — no interactive prompt)
  let inputTokens = 0
  if (flags.inputTokens !== undefined) {
    const result = validateTokens(flags.inputTokens)
    if (result !== true) { consola.error(result); return null }
    inputTokens = Number(flags.inputTokens)
  }

  // Output tokens (from flags only — no interactive prompt)
  let outputTokens = 0
  if (flags.outputTokens !== undefined) {
    const result = validateTokens(flags.outputTokens)
    if (result !== true) { consola.error(result); return null }
    outputTokens = Number(flags.outputTokens)
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

  // Cost resolution
  let costCents: number | null = null
  let manualCost = false

  if (manualCostCents !== null) {
    // --cost flag wins
    costCents = manualCostCents
    manualCost = true
  } else {
    // Try pricing lookup
    const cache = await ensurePricingCache()
    let pricing: ModelPricingEntry | null = null
    if (cache) {
      pricing = matchModel(cache, provider, model)
    }
    if (!pricing) {
      pricing = await getModelPricingDirect(model)
    }

    if (pricing && flags.inputTokens !== undefined && flags.outputTokens !== undefined) {
      // Both tokens provided via flags + pricing available → auto-calculate
      costCents = calculateCostCents(pricing, inputTokens, outputTokens)
    } else if (prompted) {
      // Interactive: prompt for cost directly
      consola.info(
        `Cost will be entered manually${pricing ? "" : " (no pricing data for this model)"}`,
      )
      const costStr = await input({
        message: "Total cost in USD (e.g. 0.50 for 50 cents)",
        validate: (v) =>
          !isNaN(Number(v)) && Number(v) >= 0 ? true : "Enter a valid non-negative number",
      })
      costCents = Math.round(Number(costStr) * 100)
      manualCost = true
    } else {
      // Non-interactive: can't determine cost
      consola.error(
        "Cost cannot be determined. Provide --cost (e.g. --cost 0.50 for 50 cents), or --input-tokens and --output-tokens together for auto-calculation.",
      )
      return null
    }
  }

  // Confirm (only when interactive)
  if (prompted) {
    const costDisplay = manualCost
      ? `$${((costCents ?? 0) / 100).toFixed(2)} (manual)`
      : `$${((costCents ?? 0) / 100).toFixed(4)}`
    consola.info(
      `Cost: ${costDisplay}${inputTokens > 0 || outputTokens > 0 ? ` (${inputTokens.toLocaleString()} in / ${outputTokens.toLocaleString()} out)` : ""}`,
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

// ── Delete ──────────────────────────────────────────────

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
    choices: all.map((e: LlmUsageEntry) => ({
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
