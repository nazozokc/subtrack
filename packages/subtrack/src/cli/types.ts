/**
 * CLI command definition types — self-contained replacement for gunshi's
 * `define` / `cli` type layer. No external dependencies.
 */

export type ArgType = "string" | "boolean" | "positional"

export interface ArgSchema {
  type: ArgType
  short?: string
  description?: string
  /** Only meaningful for positional (consumes all remaining tokens) and string (accumulates repeated flags) */
  array?: boolean
  required?: boolean
}

export type Args = Record<string, ArgSchema>

type IsRequiredKey<Name extends string, Schema extends ArgSchema> = Schema["type"] extends "positional"
  ? [Schema["array"]] extends [true]
    ? // multiple positionals are optional unless explicitly required
      Schema["required"] extends true
      ? Name
      : never
    : // single positionals are required unless explicitly optional
      Schema["required"] extends false
      ? never
      : Name
  : // options are optional unless explicitly required
    Schema["required"] extends true
    ? Name
    : never

export type ArgValue<Schema extends ArgSchema> = Schema["type"] extends "boolean"
  ? boolean
  : Schema["type"] extends "positional"
    ? [Schema["array"]] extends [true]
      ? string[]
      : string
    : [Schema["array"]] extends [true]
      ? string[]
      : string

export type ArgValues<A extends Args> = Prettify<
  Intersect<
    {
      [P in keyof A as IsRequiredKey<P & string, A[P]>]: ArgValue<A[P]>
    },
    {
      [P in keyof A as Exclude<P, IsRequiredKey<P & string, A[P]>>]?: ArgValue<A[P]>
    }
  >
>

export interface CommandContext<A extends Args = {}> {
  /** Generated fields per arg schema, keyed by the raw arg name. */
  values: ArgValues<A>
  /** All positional arguments including the command path (e.g. ["delete", "1", "2"]). */
  positionals: string[]
  /** Positional arguments after `--`. */
  rest: string[]
  /** Command name in array form like subCommands (e.g. ["tag", "rename"]) or undefined for the entry. */
  name: string | undefined
  /** The name of the invoked subcommand, or the entry command name when run as entry. */
  commandName: string
  /** Slash-separated subcommand path (e.g. "tag/rename") for API-style subcommands. */
  commandPath: string
  /** The subcommands available at the resolved command level. */
  subCommands?: Record<string, Command<any>>
}

export type Command<A extends Args = Args> = {
  name?: string
  description?: string
  args?: A
  toKebab?: boolean
  subCommands?: Record<string, Command<any>>
  run?: (ctx: CommandContext<A>) => unknown
}

/**
 * Define a command with contextual argument typing.
 * `array: true` literals survive contextual typing only while `ArgSchema`
 * declares the `array` property — do not narrow `Args` to inline schemas.
 */
export function define<A extends Args = Args>(definition: Command<A>): Command<A> {
  return Object.assign({ args: {} as A }, definition)
}

// --- helpers (ported from type-fest-style utils, kept local) ---

type Intersect<A, B> = A extends unknown ? (B extends unknown ? A & B : never) : never

type Prettify<T> = { [K in keyof T]: T[K] } & {}