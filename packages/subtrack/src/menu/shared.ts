/**
 * Shared helpers for the interactive sub-menus:
 * subscription/tag/period pickers and the Back constant.
 */

import { tagRepository } from "../application/index.ts"
import { subscriptionRepository } from "./../application/index.ts"
import { select } from "../prompts.ts"
import { CYCLE_CHOICES } from "../prompts.ts"
import { consola } from "@subtrack/lib/logger"
import { formatPrice } from "../price.ts"
import { formatCycle } from "@subtrack/lib/date"
import type { NamedCycle } from "@subtrack/lib/date"
import type { Status } from "../types.ts"

export const BACK = { name: "← Back", value: "back" } as const

export async function pickSubscription(message: string, status?: Status): Promise<number | null> {
  const subs = subscriptionRepository.list({ includeArchived: true })
    .filter((s) => status === undefined || s.status === status)
  if (subs.length === 0) {
    consola.info("No subscriptions found — choose Add from the menu")
    return null
  }
  return select({
    message,
    pageSize: 10,
    loop: false,
    choices: subs.map((s) => ({
      name: `#${s.id} ${s.name} — ${formatPrice(s.price, s.currency)}/${formatCycle(s.cycle)}${s.status !== "active" ? ` (${s.status})` : ""}`,
      value: s.id,
    })),
  })
}

export async function pickTag(message: string): Promise<string | null> {
  const tags = tagRepository.list()
  if (tags.length === 0) {
    consola.info("No tags found")
    return null
  }
  return select({
    message,
    pageSize: 10,
    choices: tags.map((t) => ({ name: t, value: t })),
  })
}

export async function pickPeriod(message = "select period"): Promise<NamedCycle> {
  return select<NamedCycle>({ message, choices: CYCLE_CHOICES })
}