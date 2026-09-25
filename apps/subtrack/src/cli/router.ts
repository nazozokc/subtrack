/**
 * Subcommand resolution — self-contained replacement for gunshi's
 * `resolveCommandTree`. Walks positional tokens through the subcommand map
 * until a leaf command is reached; remaining positionals are command args.
 */

import type { ArgSchema, Command } from "./types.ts"
import { tokenize } from "./parser.ts"

/**
 * Error thrown for an unknown subcommand. `name` stays "CommandNotFoundError"
 * so `String(error)` matches gunshi's rendering.
 */
export class CommandNotFoundError extends Error {
  readonly commandPath: string[]

  constructor(message: string, commandPath: string[] = []) {
    super(message)
    this.name = "CommandNotFoundError"
    this.commandPath = commandPath
  }
}

export interface ResolvedCommand {
  command: Command<any>
  commandName: string | undefined
  path: string[]
  depth: number
  /** True when the resolved command itself has subcommands (drives usage line). */
  omitted: boolean
  /** The unresolved subcommand name when resolution hit an unknown token. */
  unknownName: string | undefined
}

const toKebabName = (name: string): string => name.replace(/[A-Z]/g, (char) => `-${char.toLowerCase()}`)

/**
 * Extract only command-path positionals from raw argv.
 * Option values are skipped using the command tree's schemas, so a value such
 * as `--threshold 0.5` cannot be mistaken for an unknown subcommand.
 */
export function getCommandPositionals(
  argv: string[],
  entry: Command<any>,
  entrySubCommands: Record<string, Command<any>>,
): string[] {
  const valueLongNames = new Set<string>()
  const valueShortNames = new Set<string>()

  const visit = (command: Command<any>): void => {
    const schemas = (command.args ?? {}) as Record<string, ArgSchema>
    for (const [name, schema] of Object.entries(schemas)) {
      const longNames = new Set([name, toKebabName(name)])
      if (schema.type !== "boolean") {
        for (const longName of longNames) valueLongNames.add(longName)
      }
      if (schema.short && schema.type !== "boolean") valueShortNames.add(schema.short)
    }
    for (const subCommand of Object.values(command.subCommands ?? {})) visit(subCommand)
  }

  visit(entry)
  for (const command of Object.values(entrySubCommands)) visit(command)

  const positionals: string[] = []
  let skipNextPositional = false
  for (const token of tokenize(argv)) {
    if (token.kind === "terminator") break
    if (token.kind === "option") {
      const takesValue = token.rawName.startsWith("--")
        ? valueLongNames.has(token.name)
        : valueShortNames.has(token.name)
      skipNextPositional = takesValue && !token.inlineValue
      continue
    }
    if (skipNextPositional) {
      skipNextPositional = false
      continue
    }
    positionals.push(token.value)
  }
  return positionals
}

/**
 * Resolve a command from positional tokens (including the command path).
 *
 * Only tokens that match a subcommand of the current level are consumed;
 * once a leaf command is reached, the remaining positionals are arguments.
 * An unknown token yields `unknownName` with the parent command as fallback
 * (mirroring gunshi's `unresolvedCommandName`).
 */
export function resolveCommand(
  positionals: string[],
  entry: Command<any>,
  entrySubCommands: Record<string, Command<any>>,
): ResolvedCommand {
  const entryHasSubs = Object.keys(entrySubCommands).length > 0

  if (positionals.length === 0) {
    return {
      command: entry,
      commandName: undefined,
      path: [],
      depth: 0,
      omitted: entryHasSubs,
      unknownName: undefined,
    }
  }
  if (!entryHasSubs) {
    // a token was given but the entry exposes no subcommands — the token is
    // unresolved (mirrors gunshi, which reports the first positional as unknown)
    return {
      command: entry,
      commandName: undefined,
      path: [],
      depth: 0,
      omitted: false,
      unknownName: positionals[0],
    }
  }

  let currentSubCommands = entrySubCommands
  let resolvedCommand: Command<any> | undefined
  let resolvedName: string | undefined
  const path: string[] = []

  for (let i = 0; i < positionals.length; i++) {
    const token = positionals[i]
    // Own-property lookup only — inherited Object.prototype members
    // (e.g. "constructor", "toString") must not resolve as commands.
    const cmd = Object.hasOwn(currentSubCommands, token) ? currentSubCommands[token] : undefined
    if (cmd === undefined) {
      // unknown subcommand — parent (or entry) is the target
      return {
        command: resolvedCommand ?? entry,
        commandName: resolvedName,
        path,
        depth: path.length,
        omitted: false,
        unknownName: token,
      }
    }
    resolvedCommand = cmd
    resolvedName = token
    path.push(token)
    const nested = cmd.subCommands
    if (nested && Object.keys(nested).length > 0) {
      currentSubCommands = nested
    } else {
      break
    }
  }

  if (!resolvedCommand) {
    return {
      command: entry,
      commandName: undefined,
      path: [],
      depth: 0,
      omitted: entryHasSubs,
      unknownName: undefined,
    }
  }

  const subCommands = resolvedCommand.subCommands
  const omitted = subCommands != null && Object.keys(subCommands).length > 0
  return {
    command: resolvedCommand,
    commandName: resolvedName,
    path,
    depth: path.length,
    omitted,
    unknownName: undefined,
  }
}

/**
 * Build a CommandNotFoundError for the resolved unknown token.
 * Message matches gunshi: `Command not found: <name>`.
 */
export function createCommandNotFoundError(resolved: ResolvedCommand): CommandNotFoundError {
  return new CommandNotFoundError(`Command not found: ${resolved.unknownName}`, resolved.path)
}