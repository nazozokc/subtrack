import {
  confirm, select, isValidCurrency, isValidCycle, validatePrice, validateDateString, validatePlanTier,
} from "./prompts.ts"
import { consola } from "@subtrack/lib/logger"
import { fail } from "./error.ts"
import { logAudit } from "./audit-log.ts"
import { subscriptionRepository, priceHistoryRepository, withBatch } from "./application/index.ts"
import type { AddSharedArgs, SharedArgs, Status } from "./types.ts"

async function selectIds(ids: number[] | undefined): Promise<number[]> {
  if (ids?.length) return ids
  const choices = subscriptionRepository.list().map((s) => ({ name: `#${s.id} ${s.name} (${s.status})`, value: s.id }))
  if (!choices.length) return []
  return [await select({ message: "select subscription", choices, pageSize: 15 })]
}

/** Pause the selected subscriptions. */
export async function handlePause(ids?: number[], force = false): Promise<void> { await changeStatus(ids, "paused", force) }
/** Resume the selected subscriptions. */
export async function handleResume(ids?: number[], force = false): Promise<void> { await changeStatus(ids, "active", force) }

async function changeStatus(ids: number[] | undefined, target: Status, force: boolean): Promise<void> {
  const selected = await selectIds(ids)
  const subs = selected.map((id) => subscriptionRepository.get(id)).filter((s): s is SharedArgs => !!s)
  const pending = subs.filter((s) => s.status !== target && s.status !== "archived")
  if (!pending.length) { consola.info("No subscriptions need changing"); return }
  if (!force && !(await confirm({ message: `Change ${pending.length} subscription(s) to ${target}?`, default: false }))) { consola.info("Cancelled"); return }
  for (const sub of pending) {
    if (!subscriptionRepository.update(sub.id, { status: target })) {
      fail(`Subscription with id ${sub.id} not found`)
      return
    }
    logAudit(target === "active" ? "subscription.resume" : "subscription.pause", { targetType: "subscription", targetId: sub.id, details: sub.name })
  }
  consola.success(`Updated ${pending.length} subscription(s)`)
}

/** Renew a subscription and record any price change. */
export async function handleRenew(id: number, flags: { price?: string; currency?: string; cycle?: string; contractEnd?: string; planTier?: string; autoRenewal?: boolean }): Promise<void> {
  const sub = subscriptionRepository.get(id)
  if (!sub) { fail(`Subscription with id ${id} not found`); return }
  const fields: Partial<AddSharedArgs> = { status: "active" }
  if (flags.price !== undefined) {
    const validation = validatePrice(flags.price)
    if (validation !== true) {
      fail(`Invalid price: ${validation}`)
      return
    }
    fields.price = Number(flags.price)
  }
  if (flags.currency !== undefined) {
    if (!isValidCurrency(flags.currency)) {
      fail(`Invalid currency: "${flags.currency}"`)
      return
    }
    fields.currency = flags.currency
  }
  if (flags.cycle !== undefined) {
    if (!isValidCycle(flags.cycle)) {
      fail(`Invalid cycle: "${flags.cycle}"`)
      return
    }
    fields.cycle = flags.cycle as AddSharedArgs["cycle"]
  }
  if (flags.contractEnd !== undefined) {
    const contractEnd = flags.contractEnd.trim()
    if (contractEnd) {
      const validation = validateDateString(contractEnd)
      if (validation !== true) {
        fail(`Invalid contract end: ${validation}`)
        return
      }
      fields.contractEnd = contractEnd
    } else {
      fields.contractEnd = null
    }
  }
  if (flags.planTier !== undefined) {
    const planTier = flags.planTier.trim()
    if (planTier) {
      const validation = validatePlanTier(planTier)
      if (validation !== true) {
        fail(`Invalid plan tier: ${validation}`)
        return
      }
      fields.planTier = planTier
    } else {
      fields.planTier = null
    }
  }
  if (flags.autoRenewal !== undefined) fields.autoRenewal = flags.autoRenewal
  if (!subscriptionRepository.update(id, fields)) {
    fail(`Subscription with id ${id} not found`)
    return
  }
  withBatch(() =>
    priceHistoryRepository.record(
      id,
      sub.price,
      fields.price ?? sub.price,
      sub.currency,
      fields.currency ?? sub.currency,
    ),
  )
  logAudit("subscription.renew", { targetType: "subscription", targetId: id, details: `${sub.name} renewed` })
  consola.success(`Renewed: ${sub.name}`)
}
