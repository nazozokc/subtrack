/**
 * Argument tokenizer + resolver — self-contained replacement for gunshi's
 * `args-tokens` + `resolver`. Behavior mirrors the verified gunshi semantics
 * (short groups, `--x=v`, `-x=v`, `--` terminator, kebabized option names,
 * array accumulation), without external dependencies.
 */

import type { Args, ArgSchema } from "./types.ts"

export type Token = OptionToken | PositionalToken | TerminatorToken

export interface OptionToken {
  kind: "option"
  name: string
  rawName: string
  value?: string
  inlineValue?: boolean
}

export interface PositionalToken {
  kind: "positional"
  value: string
}

export interface TerminatorToken {
  kind: "terminator"
}

export interface ResolveOptions {
  /** Number of command path tokens to skip before assigning positional values */
  skip?: number
  /** Kebabize option names (e.g. `minPrice` → `min-price`) */
  toKebab?: boolean
}

export interface ResolveResult {
  values: Record<string, unknown>
  positionals: string[]
  rest: string[]
  error: AggregateError | undefined
}

const isShortOption = (arg: string): boolean =>
  arg.length === 2 && arg.codePointAt(0) === 0x2d /* - */ && arg.codePointAt(1) !== 0x2d

const isShortOptionGroup = (arg: string): boolean =>
  arg.length > 2 && arg.codePointAt(0) === 0x2d && arg.codePointAt(1) !== 0x2d

const isLongOption = (arg: string): boolean =>
  arg.startsWith("--") && arg.length > 2 && !arg.includes("=", 3)

const isLongOptionAndValue = (arg: string): boolean =>
  arg.startsWith("--") && arg.length > 2 && arg.includes("=", 3)

/**
 * Split argv into tokens. Faithful to the args-tokens behavior:
 * - `--` terminates option parsing; everything after becomes positional (→ rest)
 * - `--x` / `--x=v` long options (value only with `=`)
 * - `-x` single short option
 * - `-abc` short option group: each char is a separate option
 * - `-x=v` / `-abc=v`: chars before `=` are options, the value is a positional
 *   token (consumed by the last pending short if it is a string option)
 */
export function tokenize(argv: string[]): Token[] {
  const tokens: Token[] = []
  const remainings = [...argv]
  let terminated = false

  while (remainings.length > 0) {
    const arg = remainings.shift()
    if (arg === undefined) break

    if (terminated) {
      tokens.push({ kind: "positional", value: arg })
      continue
    }
    if (arg === "--") {
      tokens.push({ kind: "terminator" })
      terminated = true
      continue
    }
    if (isShortOption(arg)) {
      tokens.push({ kind: "option", name: arg.charAt(1), rawName: arg })
      continue
    }
    if (isShortOptionGroup(arg)) {
      const eqIdx = arg.indexOf("=")
      if (eqIdx === 2) {
        // `-x=v` : single short option with an inline value (args-tokens
        // collapses this into one option token; `-x=` yields an empty value
        // which the resolver treats as missing)
        tokens.push({
          kind: "option",
          name: arg.charAt(1),
          rawName: `-${arg.charAt(1)}`,
          value: arg.slice(eqIdx + 1),
          inlineValue: true,
        })
        continue
      }
      const expanded: string[] = []
      let hasSeparator = false
      let shortValue = ""
      for (let i = 1; i < arg.length; i++) {
        const ch = arg.charAt(i)
        if (hasSeparator) shortValue += ch
        else if (ch === "=") hasSeparator = true
        else expanded.push(`-${ch}`)
      }
      if (shortValue) expanded.push(shortValue)
      remainings.unshift(...expanded)
      continue
    }
    if (isLongOption(arg)) {
      // `--` prefix required so `--foo` never matches a short named `-f`.
      // shell-style `--foo=bar` is handled below; a key like `--foo` is stored
      // with `rawName` kept intact for `value` assignment.
      tokens.push({ kind: "option", name: arg.slice(2), rawName: arg })
      continue
    }
    if (isLongOptionAndValue(arg)) {
      const eq = arg.indexOf("=")
      const name = arg.slice(2, eq)
      const value = arg.slice(eq + 1)
      tokens.push({ kind: "option", name, rawName: `--${name}`, value, inlineValue: true })
      continue
    }
    tokens.push({ kind: "positional", value: arg })
  }
  return tokens
}

const kebabize = (name: string): string => name.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)

const getOptionName = (rawArg: string, schema: ArgSchema, toKebab: boolean): string =>
  toKebab ? kebabize(rawArg) : rawArg

/**
 * Create an option display name like `'--currency' or '-c'`.
 */
const createOptionDisplayName = (option: string, schema: ArgSchema): string =>
  `'--${option}'${schema.short ? ` or '-${schema.short}'` : ""}`

/**
 * Argument validation error. `name` is set to the argument name so that
 * `String(error)` yields `arg: message` (matches gunshi's rendering).
 */
export class ArgValidationError extends Error {
  readonly type: "type" | "required"
  readonly schema: ArgSchema

  constructor(message: string, argName: string, type: "type" | "required", schema: ArgSchema) {
    super(message)
    this.name = argName
    this.type = type
    this.schema = schema
  }
}

const createRequireError = (rawArg: string, option: string, schema: ArgSchema): ArgValidationError =>
  new ArgValidationError(
    schema.type === "positional"
      ? `Positional argument '${option}' is required`
      : `Optional argument ${createOptionDisplayName(option, schema)} is required`,
    option,
    "required",
    schema,
  )

const createTypeError = (rawArg: string, option: string, schema: ArgSchema): ArgValidationError =>
  new ArgValidationError(
    `Optional argument ${createOptionDisplayName(option, schema)} should be '${schema.type}'`,
    option,
    "type",
    schema,
  )

const shouldRequireMissingSinglePositional = (schema: ArgSchema): boolean => {
  if (schema.required === true) return true
  if (schema.required === false) return false
  // no `default` support in subtrack schemas: undecorated positionals are required
  return true
}

/**
 * Count of positionals that later (after `rawArg`) required positionals still
 * need, so optional positionals do not steal their values.
 */
function createRequiredPositionalsAfter(argEntries: [string, ArgSchema][]): Record<string, number> {
  const requiredAfter: Record<string, number> = {}
  let minimumRequired = 0
  for (let i = argEntries.length - 1; i >= 0; i--) {
    const [rawArg, schema] = argEntries[i]
    if (schema.type !== "positional") continue
    requiredAfter[rawArg] = minimumRequired
    minimumRequired += schema.array === true ? (schema.required === true ? 1 : 0) : shouldRequireMissingSinglePositional(schema) ? 1 : 0
  }
  return requiredAfter
}

/**
 * Parse and resolve argv against a command's arg schemas.
 *
 * The returned `positionals` includes the full command path (e.g.
 * `["delete", "1", "2"]`) — callers pass `skip: <path length>` so values
 * assignment starts after the command tokens, mirroring gunshi.
 */
export function resolveArgs(argv: string[], args: Args, options: ResolveOptions = {}): ResolveResult {
  const { skip = 0, toKebab = false } = options
  const tokens = tokenize(argv)

  // ── analyze phase: separate positionals, long and short options ──
  const optionTokens: OptionToken[] = []
  const positionalTokens: PositionalToken[] = []
  const rest: string[] = []

  const booleanLongOptionNames = new Set<string>()
  const shortToSchema = new Map<string, ArgSchema>()
  for (const [rawArg, schema] of Object.entries(args)) {
    if (schema.short) shortToSchema.set(schema.short, schema)
    if (schema.type !== "boolean") continue
    booleanLongOptionNames.add(getOptionName(rawArg, schema, toKebab))
  }

  const flushLong = (value?: string): void => {
    if (!pendingLong) return
    optionTokens.push({ ...pendingLong, value: value === undefined ? pendingLong.value : value })
    pendingLong = null
  }
  const flushShort = (value?: string): void => {
    if (!pendingShort) return
    optionTokens.push({ ...pendingShort, value: value === undefined ? pendingShort.value : value })
    pendingShort = null
  }
  let pendingLong: OptionToken | null = null
  let pendingShort: OptionToken | null = null
  let terminated = false

  for (const token of tokens) {
    if (token.kind === "terminator") {
      terminated = true
      flushLong()
      flushShort()
      continue
    }
    if (token.kind === "positional") {
      if (terminated && token.value) {
        rest.push(token.value)
        continue
      }
      if (pendingShort) {
        const schema = shortToSchema.get(pendingShort.name)
        if (schema?.type === "boolean") {
          positionalTokens.push(token)
          flushShort()
        } else {
          flushShort(token.value)
        }
      } else if (pendingLong) {
        if (booleanLongOptionNames.has(pendingLong.name)) {
          positionalTokens.push(token)
          flushLong()
        } else {
          flushLong(token.value)
        }
      } else {
        positionalTokens.push(token)
      }
      continue
    }
    // option token
    if (hasLongPrefix(token.rawName)) {
      flushLong()
      if (token.inlineValue) optionTokens.push({ ...token })
      else pendingLong = { ...token }
      flushShort()
    } else if (isShortOption(token.rawName)) {
      if (pendingShort) flushShort()
      if (token.inlineValue) optionTokens.push({ ...token })
      else pendingShort = { ...token }
      flushLong()
    } else {
      flushLong()
    }
  }
  flushLong()
  flushShort()

  // ── resolve phase ──
  const values: Record<string, unknown> = {}
  const errors: ArgValidationError[] = []
  const argEntries = Object.entries(args)
  const requiredPositionalsAfter = createRequiredPositionalsAfter(argEntries)

  let positionalsCount = skip
  for (const [rawArg, schema] of argEntries) {
    const arg = getOptionName(rawArg, schema, toKebab)

    if (schema.type === "positional") {
      if (schema.array === true) {
        const available = Math.max(positionalTokens.length - positionalsCount, 0)
        if (available > 0) {
          const requiredAfter = requiredPositionalsAfter[rawArg] ?? 0
          const toConsume = Math.max(available - requiredAfter, 0)
          if (toConsume > 0) {
            const end = positionalsCount + toConsume
            values[rawArg] = positionalTokens.slice(positionalsCount, end).map((t) => t.value)
            positionalsCount = end
          } else if (schema.required === true) {
            errors.push(createRequireError(rawArg, arg, schema))
          }
        } else if (schema.required === true) {
          errors.push(createRequireError(rawArg, arg, schema))
        }
      } else if (shouldRequireMissingSinglePositional(schema)) {
        const positional = positionalTokens[positionalsCount]
        if (positional) {
          values[rawArg] = positional.value
          positionalsCount++
        } else {
          errors.push(createRequireError(rawArg, arg, schema))
        }
      } else {
        const positional = positionalTokens[positionalsCount]
        const requiredAfter = requiredPositionalsAfter[rawArg] ?? 0
        if (positional && Math.max(positionalTokens.length - positionalsCount, 0) > requiredAfter) {
          values[rawArg] = positional.value
          positionalsCount++
        }
      }
      continue
    }

    // option
    const matches = optionTokens.filter((token) =>
      (schema.short !== undefined && token.name === schema.short && isShortOption(token.rawName)) ||
      (token.rawName.startsWith("--") && token.name === arg),
    )
    if (schema.required === true && matches.length === 0) {
      errors.push(createRequireError(rawArg, arg, schema))
      continue
    }
    for (const token of matches) {
      if (schema.required === true && schema.type !== "boolean" && !token.value) {
        errors.push(createRequireError(rawArg, arg, schema))
        continue
      }
      if (schema.type === "boolean") {
        if (schema.array === true) {
          values[rawArg] ??= []
          ;(values[rawArg] as unknown[]).push(true)
        } else {
          values[rawArg] = true
        }
      } else if (typeof token.value === "string") {
        const value = token.value || undefined
        if (schema.array === true) {
          values[rawArg] ??= []
          ;(values[rawArg] as unknown[]).push(value)
        } else {
          values[rawArg] = value
        }
      } else {
        errors.push(createTypeError(rawArg, arg, schema))
      }
    }
  }

  return {
    values,
    positionals: positionalTokens.map((t) => t.value),
    rest,
    error: errors.length > 0 ? new AggregateError(errors) : undefined,
  }
}

const hasLongPrefix = (rawName: string): boolean => rawName.startsWith("--")