#!/usr/bin/env node
import { cli } from "./cli/index.ts"
import { define } from "./cli/types.ts"
import { consola } from "@subtrack/lib/logger"
import { createRequire } from "node:module"
import { markStartup, reportStartup } from "./startup-profile.ts"

markStartup("runtime initialized")

// Single source of truth for the version is package.json
const require = createRequire(import.meta.url)
const pkg = require("../package.json") as { version: string }
const args = process.argv.slice(2)

// Version output does not need command definitions or parsing.
// Keep this fast path limited to the standalone form so `--version` stays
// dependency-free and instant.
if (args.length === 1 && (args[0] === "--version" || args[0] === "-v")) {
  process.stdout.write(`${pkg.version}\n`)
  process.exit(0)
}

const { subCommands } = await import("./commands/index.ts")

const mainCommand = define({
  name: "subtrack",
  description: "Manage subscription services from your terminal",
  run: async () => {
    const { handleMenu } = await import("./menu/index.ts")
    return handleMenu()
  },
})

// Signal handlers for clean shutdown
let exiting = false
const handleSignal = async (signal: string) => {
  if (exiting) return
  exiting = true
  consola.info(`Received ${signal}, saving data...`)
  try { const { saveDb } = await import("./db.ts"); saveDb() } catch { /* best-effort */ }
  process.exit(0)
}
process.on("SIGINT", () => handleSignal("SIGINT"))
process.on("SIGTERM", () => handleSignal("SIGTERM"))

// Restrict file permissions for all created files
process.umask(0o077)

try {
  markStartup("cli start")
  await cli(args, mainCommand, {
    name: "subtrack",
    version: pkg.version,
    subCommands,
    // MCP speaks JSON-RPC on stdout — suppress the header/usage banner so the
    // protocol stream stays pure.
    usageSilent: args[0] === "mcp",
  })
  reportStartup("cli complete")
} catch (error) {
  if (error instanceof Error && error.name === "ExitPromptError") {
    process.exit(0)
  }
  if (error instanceof AggregateError) {
    for (const e of error.errors) { consola.error(String(e)) }
    process.exit(1)
  }
  // Unexpected error: report cleanly and exit non-zero
  consola.error(error instanceof Error ? error.message : String(error))
  process.exit(1)
}
