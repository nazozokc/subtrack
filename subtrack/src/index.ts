#!/usr/bin/env node
import { cli, define } from "gunshi"
import { consola } from "consola"
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
  handleImport,
  handleSummary,
  handleBackup,
  handlePayment,
} from "./commands.ts"
import type { Cycle } from "./db.ts"

// ── Command definitions ──────────────────────────────────

const listCommand = define({
  name: "list",
  description: "List all subscriptions",
  args: {
    currency: { type: "string", short: "c", description: "Convert all prices to target currency" },
    sort: { type: "string", description: "Sort field: name, price, currency, cycle" },
    desc: { type: "boolean", short: "d", description: "Sort descending" },
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
  },
  run: (ctx) => handleAdd(ctx.values),
})

const editCommand = define({
  name: "edit",
  description: "Edit a subscription",
  args: {
    id: { type: "positional", description: "Subscription ID (omit for interactive selection)" },
    name: { type: "string", description: "Subscription name" },
    price: { type: "string", description: "Monthly payment amount" },
    currency: { type: "string", description: "Currency" },
    cycle: { type: "string", description: "Billing cycle" },
    tags: { type: "string", description: "Comma-separated tags" },
  },
  run: (ctx) => handleEdit(ctx.values.id, ctx.values),
})

const deleteCommand = define({
  name: "delete",
  description: "Delete subscriptions (interactive)",
  run: () => handleDelete(),
})

const tagsCommand = define({
  name: "tags",
  description: "Filter subscriptions by tags (AND logic)",
  run: (ctx) => {
    if (ctx.positionals.length === 0) {
      consola.error("Please specify at least one tag")
      return
    }
    handleTags(ctx.positionals)
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
  },
  run: (ctx) => handleExport(ctx.values.format, { currency: ctx.values.currency, tags: ctx.values.tags }),
})

const importCommand = define({
  name: "import",
  description: "Import subscriptions from CSV",
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
  description: "Backup database",
  args: {
    destination: { type: "positional", description: "Backup destination directory" },
  },
  run: (ctx) => handleBackup(ctx.values.destination),
})

const paymentCommand = define({
  name: "payment",
  description: "Show payment totals",
  args: {
    period: { type: "positional", description: "Billing period (default: monthly)" },
    currency: { type: "string", short: "c", description: "Convert all prices to target currency" },
  },
  run: (ctx) => {
    const period = (ctx.values.period || "monthly") as Cycle
    handlePayment(period, { currency: ctx.values.currency })
  },
})

const mainCommand = define({
  name: "subtrack",
  description: "Manage subscription services from your terminal",
  run: () => consola.info('Run "subtrack --help" for available commands'),
})

await cli(process.argv.slice(2), mainCommand, {
  name: "subtrack",
  version: "2.2.0",
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
    payment: paymentCommand,
  },
})
