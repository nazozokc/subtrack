/**
 * MCP server setup: wires the minimal stdio transport to the tool registry
 * and keeps stdout free of non-protocol output while the server is running.
 */

import { consola } from "@subtrack/lib/logger"
import type { LogFunction, LogType } from "@subtrack/lib/logger"

import { startTransport } from "./transport.ts"

const LOG_TYPES: readonly LogType[] = ["log", "info", "success", "warn", "error", "fail"]

function writeToStderr(...args: unknown[]): void {
  process.stderr.write(args.length === 0 ? "\n" : `${args.join(" ")}\n`)
}

/**
 * The logger writes via console.log (stdout). During MCP mode every byte on
 * stdout must be a JSON-RPC frame, so redirect all logger levels to stderr and
 * restore the originals afterwards.
 */
function redirectLoggerToStderr(): () => void {
  const originals = new Map<LogType, LogFunction>()
  for (const type of LOG_TYPES) {
    originals.set(type, consola[type])
    consola[type] = writeToStderr
  }
  return () => {
    for (const type of LOG_TYPES) consola[type] = originals.get(type) as LogFunction
  }
}

export async function startMcpServer(): Promise<void> {
  const restoreLogger = redirectLoggerToStderr()
  try {
    await startTransport(process.stdin, process.stdout)
  } finally {
    restoreLogger()
  }
}
