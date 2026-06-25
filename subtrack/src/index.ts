#!/usr/bin/env node
import { cli, define } from "gunshi"
import { consola } from "consola"
import { saveDb } from "./db.ts"
import {
  handleList,
  handleAdd,
  handleEdit,
  handleDelete,
  handleTags,
  handleTagList,
  handleTagRename,
  handleTagDelete,
  handleTagPrune,
  handleExport,
  handleSummary,
  handleBackup,
  handleRestore,
  handlePayment,
  handleUpcoming,
  handleAnalytics,
  handleConfigList,
  handleConfigGet,
  handleConfigSet,
  handleConfigReset,
} from "./commands.ts"
import {
  handleUsageAdd,
  handleUsageList,
  handleUsageDelete,
  handleUsageImport,
  handleUsageRefresh,
} from "./usage.ts"
import { handleImport } from "./import-csv.ts"
import type { Cycle, UsageRefreshFlags } from "./types.ts"

// ── Command definitions ──────────────────────────────────

const listCommand = define({
  name: "list",
  description: "List all subscriptions",
  args: {
    currency: { type: "string", short: "c", description: "Convert all prices to target currency" },
    sort: { type: "string", description: "Sort field: name, price, currency, cycle" },
    desc: { type: "boolean", short: "d", description: "Sort descending" },
    api: { type: "boolean", short: "a", description: "Include LLM API usage for current month" },
  },
  run: (ctx) => handleList(ctx.values),
})

const addCommand = define({
  name: "add",
  description: "Add a subscription",
  args: {
    name: { type: "string", description: "Subscription name" },
    price: { type: "string", description: "Monthly payment amount" },
    currency: { type: "string", description: "Currency" },
    cycle: { type: "string", description: "Billing cycle" },
    tags: { type: "string", description: "Comma-separated tags" },
    billingDay: { type: "string", description: "Billing day of month (1-31)" },
    status: { type: "string", description: "Status: active, paused, cancelled (default: active)" },
  },
  run: (ctx) => handleAdd(ctx.values),
})

const editCommand = define({
  name: "edit",
  description: "Edit a subscription",
  args: {
    id: { type: "positional", description: "Subscription ID (omit for interactive selection)", required: false },
    name: { type: "string", description: "Subscription name" },
    price: { type: "string", description: "Monthly payment amount" },
    currency: { type: "string", description: "Currency" },
    cycle: { type: "string", description: "Billing cycle" },
    tags: { type: "string", description: "Comma-separated tags" },
    status: { type: "string", description: "Status: active, paused, cancelled" },
    billingDay: { type: "string", description: "Billing day of month (1-31)" },
  },
  run: (ctx) => handleEdit(ctx.values.id ? Number(ctx.values.id) : undefined, ctx.values),
})

const deleteCommand = define({
  name: "delete",
  description: "Delete subscriptions",
  args: {
    id: { type: "positional", array: true, description: "Subscription ID(s) to delete (omit for interactive selection)", required: false },
  },
  run: (ctx) => {
    const ids = ctx.positionals.slice(1).map(Number).filter((n) => !isNaN(n))
    handleDelete(ids.length > 0 ? ids : undefined)
  },
})

const tagsCommand = define({
  name: "tags",
  description: "Filter subscriptions by tags (AND logic)",
  args: {
    names: { type: "positional", array: true, description: "Tag names", required: false },
  },
  run: (ctx) => {
    const tagNames = ctx.positionals.slice(1) as string[]
    if (tagNames.length === 0) {
      consola.error("Please specify at least one tag")
      return
    }
    handleTags(tagNames)
  },
})

const tagListCmd = define({
  name: "list",
  description: "List all tags with usage count",
  run: () => handleTagList(),
})

const tagRenameCmd = define({
  name: "rename",
  description: "Rename a tag",
  args: {
    old: { type: "positional", description: "Current tag name" },
    "new": { type: "positional", description: "New tag name" },
  },
  run: (ctx) => handleTagRename(ctx.values.old, ctx.values["new"]),
})

const tagDeleteCmd = define({
  name: "delete",
  description: "Delete a tag and its associations",
  args: {
    name: { type: "positional", description: "Tag name to delete" },
  },
  run: (ctx) => handleTagDelete(ctx.values.name),
})

const tagPruneCmd = define({
  name: "prune",
  description: "Remove orphaned tags",
  run: () => handleTagPrune(),
})

const tagCommand = define({
  name: "tag",
  description: "Manage tags",
  subCommands: {
    list: tagListCmd,
    rename: tagRenameCmd,
    delete: tagDeleteCmd,
    prune: tagPruneCmd,
  },
  run: () => consola.info("Usage: subtrack tag list|rename|delete|prune"),
})

const exportCommand = define({
  name: "export",
  description: "Export subscriptions",
  args: {
    format: { type: "positional", description: "Export format: csv, json, md" },
    currency: { type: "string", short: "c", description: "Convert all prices to target currency" },
    tags: { type: "string", description: "Filter by comma-separated tags" },
    output: { type: "string", short: "o", description: "Output file path (default: stdout)" },
  },
  run: (ctx) => handleExport(ctx.values.format, { currency: ctx.values.currency, tags: ctx.values.tags, output: ctx.values.output }),
})

const importCommand = define({
  name: "import",
  description: "Import subscriptions from CSV",
  toKebab: true,
  args: {
    file: { type: "positional", description: "CSV file to import" },
    dryRun: { type: "boolean", description: "Validate without importing" },
  },
  run: (ctx) => handleImport(ctx.values.file, { dryRun: ctx.values.dryRun }),
})

const summaryCommand = define({
  name: "summary",
  description: "Show subscription summary statistics",
  run: () => handleSummary(),
})

const backupCommand = define({
  name: "backup",
  description: "Backup database (gzip compressed)",
  args: {
    destination: { type: "positional", description: "Backup destination directory (default: ~/.config/subtrack/backups/)", required: false },
    encrypt: { type: "boolean", short: "e", description: "Encrypt the backup with your database key" },
  },
  run: (ctx) => {
    handleBackup(ctx.values.destination, { encrypt: ctx.values.encrypt })
  },
})

const restoreCommand = define({
  name: "restore",
  description: "Restore database from a backup",
  args: {
    file: { type: "positional", description: "Backup file to restore (omit for interactive selection)", required: false },
    force: { type: "boolean", short: "f", description: "Skip confirmation" },
    dir: { type: "string", description: "Directory to scan for backup files" },
  },
  run: (ctx) => handleRestore(ctx.values.file, { force: ctx.values.force, dir: ctx.values.dir }),
})

const paymentCommand = define({
  name: "payment",
  description: "Show payment totals",
  args: {
    period: { type: "positional", description: "Billing period (default: monthly)", required: false },
    currency: { type: "string", short: "c", description: "Convert all prices to target currency" },
    api: { type: "boolean", short: "a", description: "Include LLM API usage costs" },
  },
  run: (ctx) => {
    const period = (ctx.values.period || "monthly") as Cycle
    handlePayment(period, { currency: ctx.values.currency, api: ctx.values.api })
  },
})

// ── Upcoming ──────────────────────────────────────────────

const upcomingCommand = define({
  name: "upcoming",
  description: "Show upcoming bills within a number of days",
  args: {
    days: { type: "positional", description: "Number of days (default: 7)", required: false },
  },
  run: (ctx) => {
    const days = ctx.values.days ? Number(ctx.values.days) : undefined
    handleUpcoming(days)
  },
})

// ── Analytics ─────────────────────────────────────────────

const analyticsCommand = define({
  name: "analytics",
  description: "Show detailed subscription analytics",
  run: () => handleAnalytics(),
})

// ── Config ────────────────────────────────────────────────

const configListCmd = define({
  name: "list",
  description: "List all config values",
  run: () => handleConfigList(),
})

const configGetCmd = define({
  name: "get",
  description: "Get a config value",
  args: {
    key: { type: "positional", description: "Config key" },
  },
  run: (ctx) => handleConfigGet(ctx.values.key),
})

const configSetCmd = define({
  name: "set",
  description: "Set a config value",
  args: {
    key: { type: "positional", description: "Config key" },
    value: { type: "positional", description: "Config value" },
  },
  run: (ctx) => handleConfigSet(ctx.values.key, ctx.values.value),
})

const configResetCmd = define({
  name: "reset",
  description: "Reset config to defaults",
  run: () => handleConfigReset(),
})

const configCommand = define({
  name: "config",
  description: "Manage configuration",
  subCommands: {
    list: configListCmd,
    get: configGetCmd,
    set: configSetCmd,
    reset: configResetCmd,
  },
  run: () => consola.info("Usage: subtrack config list|get|set|reset"),
})

// ── Usage commands ───────────────────────────────────────

const usageAddCommand = define({
  name: "add",
  description: "Add an LLM API usage entry",
  toKebab: true,
  args: {
    provider: { type: "string", description: "Provider name (openai, anthropic, ...)" },
    model: { type: "string", description: "Model name (e.g. gpt-4o)" },
    inputTokens: { type: "string", description: "Input tokens used" },
    outputTokens: { type: "string", description: "Output tokens used" },
    date: { type: "string", description: "Date (YYYY-MM-DD, default: today)" },
    description: { type: "string", description: "Optional description" },
    cost: { type: "string", description: "Total cost in USD (e.g. 0.50 for 50 cents; overrides auto-pricing)" },
  },
  run: (ctx) => handleUsageAdd(ctx.values),
})

const usageListCommand = define({
  name: "list",
  description: "List LLM API usage entries",
  args: {
    provider: { type: "string", description: "Filter by provider" },
    from: { type: "string", description: "Start date (YYYY-MM-DD)" },
    to: { type: "string", description: "End date (YYYY-MM-DD)" },
  },
  run: (ctx) => handleUsageList(ctx.values),
})

const usageDeleteCommand = define({
  name: "delete",
  description: "Delete LLM API usage entries",
  args: {
    id: { type: "positional", array: true, description: "Entry ID(s) to delete (omit for interactive selection)", required: false },
  },
  run: (ctx) => {
    const ids = ctx.positionals.slice(1).map(Number).filter((n) => !isNaN(n))
    handleUsageDelete(ids.length > 0 ? ids : undefined)
  },
})

const usageImportCommand = define({
  name: "import",
  description: "Import LLM API usage from JSONL/JSON response log files",
  toKebab: true,
  args: {
    file: { type: "positional", description: "JSONL/JSON file to import (use - for stdin)" },
    dryRun: { type: "boolean", description: "Validate without importing" },
  },
  run: (ctx) => handleUsageImport(ctx.values),
})

const usageRefreshCommand = define({
  name: "refresh",
  description: "Auto-scan known sources (OpenCode DB, Claude Code, Codex CLI, Cursor, Copilot, Windsurf) and import usage data — defaults to current month",
  args: {
    from: { type: "string", description: "Start date (YYYY-MM-DD)" },
    to: { type: "string", description: "End date (YYYY-MM-DD)" },
    all: { type: "boolean", description: "Scan all historical data (ignore date range)" },
  },
  run: (ctx) => handleUsageRefresh(ctx.values as UsageRefreshFlags),
})

const usageCommand = define({
  name: "usage",
  description: "Track LLM API usage costs",
  subCommands: {
    add: usageAddCommand,
    list: usageListCommand,
    delete: usageDeleteCommand,
    import: usageImportCommand,
    refresh: usageRefreshCommand,
  },
  run: () => consola.info("Usage: subtrack usage add|list|delete|import|refresh"),
})

const mainCommand = define({
  name: "subtrack",
  description: "Manage subscription services from your terminal",
  run: () => consola.info('Run "subtrack --help" for available commands'),
})

// Signal handlers for clean shutdown
let exiting = false
const handleSignal = (signal: string) => {
  if (exiting) return
  exiting = true
  consola.info(`Received ${signal}, saving data...`)
  try { saveDb() } catch { /* best-effort */ }
  process.exit(0)
}
process.on("SIGINT", () => handleSignal("SIGINT"))
process.on("SIGTERM", () => handleSignal("SIGTERM"))

// Restrict file permissions for all created files
process.umask(0o077)

try {
  await cli(process.argv.slice(2), mainCommand, {
    name: "subtrack",
    version: "6.0.0",
    subCommands: {
      list: listCommand,
      add: addCommand,
      edit: editCommand,
      delete: deleteCommand,
      tags: tagsCommand,
      tag: tagCommand,
      export: exportCommand,
      import: importCommand,
      summary: summaryCommand,
      backup: backupCommand,
      restore: restoreCommand,
      payment: paymentCommand,
      upcoming: upcomingCommand,
      analytics: analyticsCommand,
      config: configCommand,
      usage: usageCommand,
    },
  })
} catch (error) {
  if (error instanceof Error && error.name === "ExitPromptError") {
    // User cancelled the prompt (Ctrl+C/D) — exit gracefully
    process.exit(0)
  }
  if (error instanceof AggregateError) {
    for (const e of error.errors) {
      consola.error(String(e))
    }
    process.exit(1)
  }
  throw error
}
