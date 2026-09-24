/**
 * Interactive suggestion review flow.
 *
 * Shows each suggestion and lets the user:
 * - [a]dd — add as a new subscription (with optional edit)
 * - [e]dit — pre-fill and edit before adding
 * - [s]kip — skip this suggestion
 * - [q]uit — stop reviewing
 */

import { confirm, input, select } from "../prompts.ts"
import { consola } from "@subtrack/lib/logger"
import pc from "@subtrack/lib/ansi"
import { writeSubscription, markSuggestionAsAdded, dismissSuggestion } from "../db.ts"
import { formatPrice } from "../price.ts"
import type { Suggestion } from "./types.ts"
import { findMatches, hasPriceConflict } from "./matcher.ts"

/**
 * Strip control characters (incl. ESC) from email-derived strings before
 * printing, preventing terminal escape-sequence injection via the terminal.
 */
function displaySafe(value: string): string {
  return value.replace(/[\x00-\x1F\x7F]/g, "")
}

/**
 * Review all pending suggestions interactively.
 */
export async function reviewSuggestions(suggestions: Suggestion[]): Promise<void> {
  if (suggestions.length === 0) {
    consola.info("No pending suggestions.")
    return
  }

  for (const suggestion of suggestions) {
    const shouldContinue = await reviewOne(suggestion)
    if (!shouldContinue) break
  }
}

/**
 * Review a single suggestion. Returns false if user wants to quit.
 */
async function reviewOne(suggestion: Suggestion): Promise<boolean> {
  // Sanitize email-derived strings before printing to the terminal
  const safeName = displaySafe(suggestion.name)
  const safeSource = suggestion.sourceDetail ? displaySafe(suggestion.sourceDetail) : null

  // Display suggestion info
  const priceStr = suggestion.price !== null && suggestion.currency
    ? formatPrice(suggestion.price, suggestion.currency)
    : "N/A"
  const cycleStr = suggestion.cycle ?? "unknown cycle"

  consola.log("")
  consola.log(pc.bold(pc.cyan(`── Suggestion #${suggestion.id} ──`)))
  consola.log(`  ${pc.bold("Name:")}     ${safeName}`)
  consola.log(`  ${pc.bold("Price:")}    ${priceStr}/${cycleStr}`)
  if (safeSource) {
    consola.log(`  ${pc.bold("Source:")}   ${safeSource}`)
  }
  consola.log(`  ${pc.dim(`Confidence: ${Math.round(suggestion.confidence * 100)}%`)}`)
  consola.log("")

  // Check for matches
  const { matches, exactMatch } = findMatches(suggestion)
  if (exactMatch) {
    consola.warn(`⚠ Already exists: "${displaySafe(matches[0].name)}" (${formatPrice(matches[0].price, matches[0].currency)}/${matches[0].cycle})`)
    const action = await select({
      message: "What to do?",
      choices: [
        { name: "Skip (already exists)", value: "skip" },
        { name: "Add as duplicate anyway", value: "add" },
        { name: "Quit reviewing", value: "quit" },
      ],
    })
    switch (action) {
      case "skip": return true
      case "quit": return false
      case "add": break
    }
  } else if (matches.length > 0) {
    consola.info(`Similar existing subscriptions:`)
    for (const m of matches) {
      const conflict = hasPriceConflict(suggestion, m) ? pc.red(" (price differs)") : ""
      consola.log(`  · ${displaySafe(m.name)} — ${formatPrice(m.price, m.currency)}/${m.cycle}${conflict}`)
    }
    consola.log("")
  }

  // Ask what to do
  const action = await select({
    message: `Add "${safeName}" as a subscription?`,
    choices: [
      { name: `${pc.green("Add")}       — add with extracted values`, value: "add" },
      { name: `${pc.yellow("Edit")}      — review and edit before adding`, value: "edit" },
      { name: `${pc.dim("Skip")}       — dismiss this suggestion`, value: "skip" },
      { name: "Quit reviewing", value: "quit" },
    ],
  })

  switch (action) {
    case "add": {
      // Determine cycle — ask if not detected
      let cycle = suggestion.cycle
      if (!cycle) {
        cycle = await select({
          message: "Select billing cycle:",
          choices: [
            { name: "Monthly", value: "monthly" },
            { name: "Yearly", value: "yearly" },
            { name: "Weekly", value: "weekly" },
            { name: "Quarterly", value: "quarterly" },
          ],
        })
      }

      // Determine billing day from email date
      let billingDay: number | null = null
      if (suggestion.emailDate) {
        const d = new Date(suggestion.emailDate)
        if (!isNaN(d.getTime())) {
          billingDay = d.getDate()
        }
      }

      const result = writeSubscription({
        name: suggestion.name,
        price: suggestion.price ?? 0,
        currency: suggestion.currency ?? "USD",
        cycle: cycle as import("../types.ts").Cycle,
        tags: [],
        billingDay,
        vendorName: suggestion.vendorName,
        planTier: suggestion.planTier,
        paymentMethod: suggestion.paymentMethod,
      })

      markSuggestionAsAdded(suggestion.id, result)
      consola.success(`Added: "${safeName}" (${priceStr}/${cycle})`)
      return true
    }

    case "edit": {
      // Let user edit fields interactively
      const edited = await editSuggestion(suggestion)
      if (edited) {
        markSuggestionAsAdded(suggestion.id, edited.id)
        consola.success(`Added: "${displaySafe(edited.name)}"`)
      }
      return true
    }

    case "skip": {
      dismissSuggestion(suggestion.id)
      consola.info("Suggestion dismissed.")
      return true
    }

    case "quit":
      return false
  }
}

/**
 * Interactive edit flow for a suggestion before adding.
 */
async function editSuggestion(suggestion: Suggestion): Promise<{ id: number; name: string } | null> {
  const { validateName, validatePrice } = await import("../prompts.ts")

  const name = await input({
    message: "Name:",
    default: suggestion.name,
    validate: validateName,
  })
  if (!name?.trim()) return null

  const defaultPrice = suggestion.price?.toString() ?? ""
  const priceStr = await input({
    message: "Monthly payment amount:",
    default: defaultPrice,
    validate: validatePrice,
  })
  const price = Number(priceStr)
  if (isNaN(price) || price <= 0) return null

  const currency = await select<"USD" | "JPY" | "EUR" | "GBP">({
    message: "Currency:",
    choices: [
      { name: "USD ($)", value: "USD" },
      { name: "JPY (¥)", value: "JPY" },
      { name: "EUR (€)", value: "EUR" },
      { name: "GBP (£)", value: "GBP" },
    ],
    default: (suggestion.currency as "USD" | "JPY" | "EUR" | "GBP") ?? "USD",
  })

  const cycle = await select<import("../types.ts").Cycle>({
    message: "Cycle:",
    choices: [
      { name: "Monthly", value: "monthly" },
      { name: "Yearly", value: "yearly" },
      { name: "Weekly", value: "weekly" },
      { name: "Quarterly", value: "quarterly" },
      { name: "Bi-weekly", value: "bi-weekly" },
      { name: "Semi-annual", value: "semi-annual" },
    ],
    default: (suggestion.cycle ?? "monthly") as import("../types.ts").Cycle,
  })

  const ok = await confirm({
    message: `Save "${name}" (${formatPrice(price, currency)}, ${cycle})?`,
    default: true,
  })
  if (!ok) {
    consola.info("Cancelled.")
    return null
  }

  const id = writeSubscription({
    name,
    price,
    currency,
    cycle: cycle as import("../types.ts").Cycle,
    tags: [],
    billingDay: suggestion.emailDate ? (() => {
      const d = new Date(suggestion.emailDate)
      return isNaN(d.getTime()) ? null : d.getDate()
    })() : null,
    vendorName: suggestion.vendorName,
    planTier: suggestion.planTier,
    paymentMethod: suggestion.paymentMethod,
  })

  return { id, name }
}
