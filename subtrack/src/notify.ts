import { consola } from "consola"
import { calcUpcoming } from "./upcoming.ts"
import { formatPrice } from "./price.ts"
import { loadConfig } from "./config.ts"
import type { Currency } from "./types.ts"

export type NotifyOptions = {
  days?: number
  dryRun?: boolean
  json?: boolean
}

export async function handleNotify(options: NotifyOptions = {}): Promise<void> {
  const config = loadConfig()
  const days = options.days ?? config.notifyDays ?? 7

  const entries = calcUpcoming(days)

  if (options.json) {
    const data = entries.map((e) => ({
      name: e.sub.name,
      price: e.sub.price,
      currency: e.sub.currency,
      cycle: e.sub.cycle,
      nextDate: `${e.nextDate.getFullYear()}-${String(e.nextDate.getMonth() + 1).padStart(2, "0")}-${String(e.nextDate.getDate()).padStart(2, "0")}`,
      tags: e.sub.tags,
    }))
    process.stdout.write(JSON.stringify({ days, count: entries.length, entries: data }, null, 2) + "\n")
    return
  }

  if (entries.length === 0) {
    if (!options.dryRun) return // no notification needed
    consola.info(`No upcoming bills in the next ${days} day${days > 1 ? "s" : ""}`)
    return
  }

  if (options.dryRun) {
    consola.info(`Upcoming bills (next ${days} day${days > 1 ? "s" : ""}):`)
    for (const e of entries) {
      const date = `${e.nextDate.getFullYear()}-${String(e.nextDate.getMonth() + 1).padStart(2, "0")}-${String(e.nextDate.getDate()).padStart(2, "0")}`
      consola.log(`  ${date}  ${e.sub.name}  ${formatPrice(e.sub.price, e.sub.currency)}/${e.sub.cycle}`)
    }
    return
  }

  // ── Send OS notification ──
  await sendNotification(entries, days)
}

async function sendNotification(
  entries: { sub: { name: string; price: number; currency: string; cycle: string } }[],
  days: number,
): Promise<void> {
  // Lazy-import to avoid loading the notifier when not needed
  const { default: notifier } = await import("node-notifier")

  const count = entries.length
  let message: string

  if (count <= 5) {
    message = entries
      .map((e) => `${e.sub.name}: ${formatPrice(e.sub.price, e.sub.currency)}/${e.sub.cycle}`)
      .join("\n")
  } else {
    const shown = entries.slice(0, 5)
    message =
      shown
        .map((e) => `${e.sub.name}: ${formatPrice(e.sub.price, e.sub.currency)}/${e.sub.cycle}`)
        .join("\n") + `\n... and ${count - 5} more`
  }

  notifier.notify({
    title: `subtrack: ${count} upcoming bill${count > 1 ? "s" : ""} in ${days} day${days > 1 ? "s" : ""}`,
    message,
    sound: true,
    timeout: 10,
  })
}
