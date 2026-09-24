/**
 * CLI orchestration — self-contained replacement for gunshi's `cli()`.
 *
 * Flow (mirrors verified gunshi behavior):
 *   1. tokenize argv (parser)
 *   2. resolve the subcommand path (router)
 *   3. resolve args for the target command with `skip = path depth`
 *   4. priority errors (CommandNotFoundError) → render + throw
 *   5. `--version` fast path (no header)
 *   6. header/banner
 *   7. `--help` → render usage
 *   8. validation errors → render + throw
 *   9. run the command
 */

import { resolveArgs, tokenize } from "./parser.ts"
import { resolveCommand, createCommandNotFoundError, CommandNotFoundError } from "./router.ts"
import { renderUsage } from "./help.ts"
import type { Args, Command, CommandContext } from "./types.ts"

export interface CliOptions {
  name?: string
  description?: string
  version?: string
  usageSilent?: boolean
  subCommands?: Record<string, Command<any>>
}

const COMMON_ARGS: Args = {
  help: { type: "boolean", short: "h", description: "Display this help message" },
  version: { type: "boolean", short: "v", description: "Display this version" },
}

const hasPriorityValidationError = (error: AggregateError | undefined): error is AggregateError =>
  error !== undefined && error.errors.some((e) => e instanceof CommandNotFoundError)

const renderValidationErrors = (error: AggregateError): string =>
  error.errors.map((e) => (e instanceof Error ? e.message : String(e))).join("\n")

/**
 * Run the CLI. Prints to stdout like gunshi; when `usageSilent` is set, output
 * is suppressed and rendered strings are returned instead.
 */
export async function cli(
  argv: string[],
  entry: Command<any>,
  options: CliOptions = {},
): Promise<string | undefined> {
  const name = options.name || entry.name || "cli"
  const version = options.version
  const usageSilent = options.usageSilent === true
  const cliSubCommands: Record<string, Command<any>> = options.subCommands ?? {}

  const log = (message: string): void => {
    if (!usageSilent) console.log(message)
  }

  // ── 1/2. resolve the command tree from positional tokens ──
  const positionals = tokenize(argv)
    .filter((t): t is { kind: "positional"; value: string } => t.kind === "positional")
    .map((t) => t.value)
  const resolved = resolveCommand(positionals, entry, cliSubCommands)

  const additionalErrors: Error[] = []
  if (resolved.unknownName !== undefined) {
    additionalErrors.push(createCommandNotFoundError(resolved))
  }

  // For unknown subcommands the target is the parent command (never executed —
  // the priority error throws first), matching gunshi's parent fallback.
  const targetCommand = resolved.command
  const targetPath = resolved.path
  const targetDepth = resolved.depth

  // ── 3. resolve args for the target command ──
  const args: Args = { ...COMMON_ARGS, ...(targetCommand.args ?? {}) }
  const { values, positionals: ctxPositionals, rest, error } = resolveArgs(argv, args, {
    toKebab: targetCommand.toKebab === true,
    skip: targetDepth,
  })

  const errors: Error[] = [...additionalErrors]
  if (error !== undefined) errors.push(...error.errors)
  const validationError = errors.length > 0 ? new AggregateError(errors) : undefined

  // ── 4. priority errors (CommandNotFoundError) — rendered without header ──
  if (hasPriorityValidationError(validationError)) {
    log(renderValidationErrors(validationError))
    throw validationError
  }

  // ── 5. version ──
  if (values.version === true) {
    const v = version || "unknown"
    log(v)
    return v
  }

  // ── 6. header ──
  const title = options.description || name
  const header = title ? `${title} (${name}${version ? ` v${version}` : ""})` : ""
  if (header) {
    log(header)
    log("")
  }

  // ── 7. help ──
  if (values.help === true) {
    // subcommands for the resolved level: its own, or the CLI's for the entry
    const levelSubCommands =
      targetCommand.subCommands ?? (targetDepth === 0 ? cliSubCommands : {})
    const usage = renderUsage({
      name,
      command: targetCommand,
      path: targetPath,
      omitted: resolved.omitted,
      callMode: targetDepth > 0 ? "subCommand" : "entry",
      args,
      subCommands: levelSubCommands,
    })
    if (usage) {
      log(usage)
      return header ? `${header}\n\n${usage}` : usage
    }
    return header || undefined
  }

  // ── 8. validation errors ──
  if (validationError !== undefined) {
    log(renderValidationErrors(validationError))
    throw validationError
  }

  // ── 9. run ──
  const ctx: CommandContext = {
    values,
    positionals: ctxPositionals,
    rest,
    name: targetCommand.name,
    commandName:
      targetPath.length > 0 ? targetPath[targetPath.length - 1] : (targetCommand.name ?? ""),
    commandPath: targetPath.join("/"),
  }
  const result = targetCommand.run ? await targetCommand.run(ctx) : undefined
  return typeof result === "string" ? result : undefined
}