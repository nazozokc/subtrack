/**
 * Delete subscription handler.
 * Split out of core.ts so display-only commands (list, summary, tags, ...)
 * never load the interactive prompt machinery.
 */

import { checkbox, confirm } from "../prompts.ts"
import { consola } from "@subtrack/lib/logger"
import { fail } from "../error.ts"
import { subscriptionRepository } from "../application/repositories.ts"
import { formatPrice } from "../price.ts"
import { logAudit } from "../audit-log.ts"

export async function handleDelete(ids?: number[]) {
  if (ids && ids.length > 0) {
    for (const id of ids) {
      const sub = subscriptionRepository.get(id)
      if (!sub) {
        fail(`Subscription with id ${id} not found`)
        continue
      }
      subscriptionRepository.remove(id)
      logAudit("subscription.delete", {
        targetType: "subscription",
        targetId: id,
        details: sub.name,
      })
      consola.success(`Deleted: ${sub.name}`)
    }
    return
  }

  const all = subscriptionRepository.list()

  if (all.length === 0) {
    consola.info("No subscriptions found — try `subtrack add`")
    return
  }

  const selected = await checkbox({
    message: "select subscriptions to delete",
    choices: all.map((sub) => ({
      name: `${sub.name} — ${formatPrice(sub.price, sub.currency)}/${sub.cycle}${sub.tags.length > 0 ? ` [${sub.tags.join(", ")}]` : ""}`,
      value: sub,
    })),
  })

  if (selected.length === 0) {
    consola.info("Cancelled")
    return
  }

  const names = selected.map((s) => s.name).join(", ")
  const ok = await confirm({
    message: `Delete ${selected.length} subscription${selected.length > 1 ? "s" : ""}? (${names})`,
    default: false,
  })

  if (!ok) {
    consola.info("Cancelled")
    return
  }

  for (const sub of selected) {
    subscriptionRepository.remove(sub.id)
    logAudit("subscription.delete", {
      targetType: "subscription",
      targetId: sub.id,
      details: sub.name,
    })
    consola.success(`Deleted: ${sub.name}`)
  }
}