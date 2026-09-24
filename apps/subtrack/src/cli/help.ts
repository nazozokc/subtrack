/**
 * Usage / help renderer — self-contained replacement for gunshi's
 * `@gunshi/plugin-renderer`. Produces equivalent information:
 * description, USAGE, COMMANDS (+ "for more info" hints), ARGUMENTS, OPTIONS.
 */

import type { Args, ArgSchema, Command } from "./types.ts"

const LEFT_MARGIN = 2
const MIDDLE_MARGIN = 10

const kebabize = (name: string): string => name.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)

interface RenderUsageParams {
  /** CLI name shown in usage lines (e.g. "subtrack") */
  name: string
  /** The resolved command being helped (entry command for the top level) */
  command: Command<any>
  /** Subcommand path of the resolved command (e.g. ["usage"] for `usage --help`, [] for the entry) */
  path: string[]
  /** True when the resolved command has subcommands available */
  omitted: boolean
  /** "subCommand" for nested commands (shows the description), "entry" otherwise */
  callMode: "entry" | "subCommand"
  /** Merged arg schemas (global help/version + command args) */
  args: Args
  /** Subcommands of the resolved command (including an entry copy) */
  subCommands: Record<string, Command<any>>
}

const hasOptionalArgs = (args: Args): boolean =>
  Object.entries(args).some(([, schema]) => schema.type !== "positional")

const hasPositionalArgs = (args: Args): boolean =>
  Object.entries(args).some(([, schema]) => schema.type === "positional")

const makeShortLongOptionPair = (schema: ArgSchema, name: string, toKebab: boolean): string => {
  const key = `--${toKebab ? kebabize(name) : name}`
  return schema.short ? `-${schema.short}, ${key}` : key
}

const isRequiredSinglePositional = (schema: ArgSchema): boolean => {
  if (schema.required === true) return true
  if (schema.required === false) return false
  return true // no `default` support: undecorated positionals are required
}

function generatePositionalSymbols(args: Args): string {
  return Object.entries(args)
    .filter(([, schema]) => schema.type === "positional")
    .map(([name, schema]) => {
      if (schema.array === true) {
        return schema.required === true ? `<${name}> [<${name}> ...]` : `[<${name}> ...]`
      }
      return isRequiredSinglePositional(schema) ? `<${name}>` : `[<${name}>]`
    })
    .join(" ")
}

function generateOptionsSymbols(args: Args): string {
  return hasOptionalArgs(args) ? "<OPTIONS>" : ""
}

export function renderUsage(params: RenderUsageParams): string {
  const { name, command, path, omitted, callMode, args, subCommands } = params
  const messages: string[] = []

  if (callMode === "subCommand") {
    if (command.description) messages.push(command.description, "")
  }

  // ── USAGE ──
  messages.push("USAGE:")
  const optionSymbols = generateOptionsSymbols(args)
  const positionalSymbols = generatePositionalSymbols(args)
  const loaded = loadCommands(command, subCommands)
  const hasCommands = loaded.length > 1

  let usage = name
  if (omitted && hasCommands) usage += " [COMMANDS]"
  else if (callMode === "subCommand") usage += ` ${path.join(" ")}`
  if (optionSymbols) usage += ` ${optionSymbols}`
  if (positionalSymbols) usage += ` ${positionalSymbols}`
  messages.push(usage.padStart(LEFT_MARGIN + usage.length), "")

  // ── COMMANDS + "for more info" hints ──
  if (omitted && hasCommands) {
    messages.push("COMMANDS:")
    const commandMaxLength = Math.max(...loaded.map((cmd) => (cmd.name || "").length))
    const symbolLength = commandMaxLength + optionSymbols.length + positionalSymbols.length
    for (const cmd of loaded) {
      let commandStr = cmd.entry ? `[${cmd.name}]` : cmd.name || ""
      if (optionSymbols) commandStr += ` ${optionSymbols}`
      if (positionalSymbols) commandStr += ` ${positionalSymbols}`
      const desc = cmd.description || ""
      const row = desc ? commandStr.padEnd(symbolLength + MIDDLE_MARGIN) + desc : commandStr
      messages.push(row.padStart(LEFT_MARGIN + row.length))
    }
    messages.push("", "For more info, run any command with the `--help` flag:")
    const basePath = path.length > 0 ? `${name} ${path.join(" ")}` : name
    for (const cmd of loaded) {
      const hint = cmd.entry ? `${basePath} --help` : `${basePath} ${cmd.name} --help`
      messages.push(hint.padStart(LEFT_MARGIN + hint.length))
    }
    messages.push("")
  }

  // ── ARGUMENTS ──
  if (hasPositionalArgs(args)) {
    messages.push("ARGUMENTS:")
    const positionals = Object.entries(args).filter(([, schema]) => schema.type === "positional")
    const argsMaxLength = Math.max(...positionals.map(([n]) => n.length))
    for (const [name, schema] of positionals) {
      const desc = schema.description || ""
      const arg = `${name.padEnd(argsMaxLength + MIDDLE_MARGIN)} ${desc}`
      messages.push(arg.padStart(LEFT_MARGIN + arg.length))
    }
    messages.push("")
  }

  // ── OPTIONS ──
  if (hasOptionalArgs(args)) {
    messages.push("OPTIONS:")
    const options = Object.entries(args)
      .filter(([, schema]) => schema.type !== "positional")
      .map(([name, schema]) => {
        const toKebab = command.toKebab === true
        const key = makeShortLongOptionPair(schema, name, toKebab)
        const display = toKebab ? kebabize(name) : name
        return [name, schema.type !== "boolean" ? `${key} <${display}>` : key] as const
      })
    const optionsMaxLength = Math.max(...options.map(([, key]) => key.length))
    for (const [rawName, key] of options) {
      const schema = args[rawName]
      const desc = schema.description || ""
      const option = key.padEnd(optionsMaxLength + MIDDLE_MARGIN) + desc
      messages.push(option.padStart(LEFT_MARGIN + option.length))
    }
    messages.push("")
  }

  return messages.join("\n")
}

function loadCommands(command: Command<any>, subCommands: Record<string, Command<any>>): LoadedCommand[] {
  const loaded: LoadedCommand[] = [
    { entry: true, name: command.name, description: command.description },
    ...Object.values(subCommands).map((cmd) => ({
      entry: false,
      name: cmd.name,
      description: cmd.description,
    })),
  ]
  return loaded.sort((a, b) => {
    if (a.entry && !b.entry) return -1
    if (!a.entry && b.entry) return 1
    if (a.name && b.name) return a.name.localeCompare(b.name)
    if (a.name && !b.name) return -1
    if (!a.name && b.name) return 1
    return 0
  })
}

interface LoadedCommand {
  entry: boolean
  name: string | undefined
  description: string | undefined
}