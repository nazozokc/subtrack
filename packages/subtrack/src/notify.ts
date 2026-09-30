import { consola } from "@subtrack/lib/logger"
import { calcUpcoming } from "./upcoming.ts"
import { formatPrice } from "./price.ts"
import { loadConfig } from "./config.ts"
import type { NotifyChannel } from "./types.ts"
import { formatDate, formatShortDate } from "@subtrack/lib/date"
import { spawnSync } from "node:child_process"

export type NotifyOptions = {
  days?: number
  dryRun?: boolean
  json?: boolean
  channel?: NotifyChannel
}

const WEBHOOK_TIMEOUT_MS = 10_000
const NOTIFY_COMMAND_TIMEOUT_MS = 10_000

/**
 * A webhook target must be https at the moment it is used, not only when it was
 * written. `config.json` is a plain file the user can edit, sync from a dotfiles
 * repo, or restore from a backup, and the payload carries subscription names,
 * prices, and renewal dates — data that should never leave over plaintext or
 * be aimed at an internal host.
 */
function requireHttps(url: string | undefined, field: string): string | null {
  if (!url) return null
  let parsed: URL
  try {
    parsed = new URL(url)
  } catch {
    return null
  }
  if (parsed.protocol !== "https:") {
    consola.warn(`${field} must be an https:// URL — notification skipped`)
    return null
  }
  return parsed.toString()
}

/** POST JSON to a webhook with a hard timeout so a hung endpoint cannot hang the CLI. */
async function postJson(url: string, payload: unknown): Promise<Response> {
  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), WEBHOOK_TIMEOUT_MS)
  try {
    return await globalThis.fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
      signal: controller.signal,
    })
  } finally {
    clearTimeout(timer)
  }
}

/**
 * Drain and discard a response body.
 *
 * An unconsumed body keeps the connection checked out of the pool. `notify` is
 * wired to run on a schedule, so leaking one socket per run adds up.
 */
async function discardBody(res: Response): Promise<void> {
  try {
    await res.arrayBuffer()
  } catch {
    // A body that cannot be read is not worth reporting; the status was the
    // part the caller cared about.
  }
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
      nextDate: formatDate(e.nextDate),
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
    const fmt = loadConfig().dateFormat === "short" ? formatShortDate : formatDate
    for (const e of entries) {
      const date = fmt(e.nextDate)
      consola.log(`  ${date}  ${e.sub.name}  ${formatPrice(e.sub.price, e.sub.currency)}/${e.sub.cycle}`)
    }
    return
  }

  // ── Determine channels ──
  const channels: NotifyChannel[] = options.channel
    ? [options.channel]
    : (config.notifyChannels?.length ? config.notifyChannels : ["os"])

  for (const channel of channels) {
    switch (channel) {
      case "os":
        sendOsNotification(entries, days)
        break
      case "slack":
        await sendSlackNotification(entries, days, config.slackWebhook)
        break
      case "webhook":
        await sendWebhookNotification(entries, days, config.webhookUrl)
        break
    }
  }
}

// ── OS Notification ─────────────────────────────────────
// Uses the platform's native notification command so no runtime
// dependency is needed. Missing commands fail silently.

/** Escape a string for embedding in an AppleScript string literal. */
function asEscape(s: string): string {
  return s.replace(/\\/g, "\\\\").replace(/"/g, '\\"')
}

function sendOsNotification(
  entries: { sub: { name: string; price: number; currency: string; cycle: string } }[],
  days: number,
): void {
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

  const title = `subtrack: ${count} upcoming bill${count > 1 ? "s" : ""} in ${days} day${days > 1 ? "s" : ""}`

  // `timeout` because spawnSync blocks: a hung osascript / MessageBox would
  // wedge the whole process with no output and no way to interrupt it.
  const spawnOpts = { timeout: NOTIFY_COMMAND_TIMEOUT_MS, stdio: "ignore" } as const

  if (process.platform === "darwin") {
    spawnSync("osascript", [
      "-e",
      `display notification "${asEscape(message)}" with title "${asEscape(title)}" sound name "default"`,
    ], spawnOpts)
  } else if (process.platform === "win32") {
    spawnSync("powershell.exe", [
      "-NoProfile",
      "-Command",
      `Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.MessageBox]::Show('${message.replace(/'/g, "''")}', '${title.replace(/'/g, "''")}')`,
    ], spawnOpts)
  } else {
    spawnSync("notify-send", [title, message, "-t", "10000"], spawnOpts)
  }
}

// ── Slack Webhook ───────────────────────────────────────

async function sendSlackNotification(
  entries: { sub: { name: string; price: number; currency: string; cycle: string } }[],
  days: number,
  webhookUrl?: string,
): Promise<void> {
  const target = requireHttps(webhookUrl, "slackWebhook")
  if (!target) {
    if (webhookUrl) return // already warned: present but not https
    consola.warn("Slack notification configured but no slackWebhook set. Use: subtrack config set slackWebhook https://hooks.slack.com/services/...")
    return
  }

  const attachments = entries.map((e) => ({
    color: "#36a64f",
    text: `${e.sub.name}: ${formatPrice(e.sub.price, e.sub.currency)}/${e.sub.cycle}`,
  }))

  try {
    const response = await postJson(target, {
      text: `subtrack: *${entries.length} upcoming bill${entries.length > 1 ? "s" : ""}* in ${days} day${days > 1 ? "s" : ""}`,
      attachments,
    })
    await discardBody(response)
    if (response.ok) {
      consola.success("Slack notification sent")
    } else {
      consola.warn(`Slack webhook returned ${response.status}: ${response.statusText}`)
    }
  } catch (err) {
    consola.warn(`Failed to send Slack notification: ${err instanceof Error ? err.message : String(err)}`)
  }
}

// ── Generic Webhook ─────────────────────────────────────

async function sendWebhookNotification(
  entries: { sub: { name: string; price: number; currency: string; cycle: string } }[],
  days: number,
  webhookUrl?: string,
): Promise<void> {
  const target = requireHttps(webhookUrl, "webhookUrl")
  if (!target) {
    if (webhookUrl) return // already warned: present but not https
    consola.warn("Webhook configured but no webhookUrl set. Use: subtrack config set webhookUrl https://example.com/hook")
    return
  }

  try {
    const response = await postJson(target, {
      event: "upcoming_bills",
      days,
      count: entries.length,
      entries: entries.map((e) => ({
        name: e.sub.name,
        price: e.sub.price,
        currency: e.sub.currency,
        cycle: e.sub.cycle,
      })),
    })
    await discardBody(response)
    if (response.ok) {
      consola.success("Webhook notification sent")
    } else {
      consola.warn(`Webhook returned ${response.status}: ${response.statusText}`)
    }
  } catch (err) {
    consola.warn(`Failed to send webhook notification: ${err instanceof Error ? err.message : String(err)}`)
  }
}
