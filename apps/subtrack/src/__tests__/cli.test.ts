import { test, expect, describe, vi } from "vitest"
import { tokenize, resolveArgs, ArgValidationError } from "../cli/parser.ts"
import { resolveCommand, CommandNotFoundError } from "../cli/router.ts"
import { renderUsage } from "../cli/help.ts"
import { cli } from "../cli/index.ts"
import { define } from "../cli/types.ts"
import type { Args, Command } from "../cli/types.ts"

describe("tokenize", () => {
  test("long options with and without values", () => {
    expect(tokenize(["--foo", "--bar=baz", "x"])).toEqual([
      { kind: "option", name: "foo", rawName: "--foo" },
      { kind: "option", name: "bar", rawName: "--bar", value: "baz", inlineValue: true },
      { kind: "positional", value: "x" },
    ])
  })

  test("short option groups expand to separate options", () => {
    expect(tokenize(["-abc"])).toEqual([
      { kind: "option", name: "a", rawName: "-a" },
      { kind: "option", name: "b", rawName: "-b" },
      { kind: "option", name: "c", rawName: "-c" },
    ])
  })

  test("single short option with =value collapses into one option token", () => {
    expect(tokenize(["-c=USD"])).toEqual([
      { kind: "option", name: "c", rawName: "-c", value: "USD", inlineValue: true },
    ])
    expect(tokenize(["-c="])).toEqual([
      { kind: "option", name: "c", rawName: "-c", value: "", inlineValue: true },
    ])
  })

  test("multi-char short group with =value yields options and a positional value", () => {
    expect(tokenize(["-abc=v"])).toEqual([
      { kind: "option", name: "a", rawName: "-a" },
      { kind: "option", name: "b", rawName: "-b" },
      { kind: "option", name: "c", rawName: "-c" },
      { kind: "positional", value: "v" },
    ])
  })

  test("terminator sends the rest to positionals", () => {
    expect(tokenize(["a", "--", "--flag", "-x"])).toEqual([
      { kind: "positional", value: "a" },
      { kind: "terminator" },
      { kind: "positional", value: "--flag" },
      { kind: "positional", value: "-x" },
    ])
  })

  test("lone dash is a positional", () => {
    expect(tokenize(["-"])).toEqual([{ kind: "positional", value: "-" }])
  })
})

describe("resolveArgs", () => {
  const args: Args = {
    limit: { type: "string" },
    json: { type: "boolean", short: "j" },
    currency: { type: "string", short: "c" },
    desc: { type: "boolean", short: "d" },
    id: { type: "positional", array: true, required: false },
    set: { type: "string", required: true },
  }

  test("boolean flags from long and short", () => {
    const r = resolveArgs(["--json", "-d"], { json: { type: "boolean" }, desc: { type: "boolean", short: "d" } })
    expect(r.values).toEqual({ json: true, desc: true })
    expect(r.error).toBeUndefined()
  })

  test("string value with = and spaced forms", () => {
    expect(resolveArgs(["--limit=5"], args).values.limit).toBe("5")
    expect(resolveArgs(["--limit", "5"], args).values.limit).toBe("5")
    expect(resolveArgs(["-c", "USD"], args).values.currency).toBe("USD")
    expect(resolveArgs(["-c=USD"], args).values.currency).toBe("USD")
  })

  test("unknown options are ignored (non-strict)", () => {
    const r = resolveArgs(["--unknown", "--limit=3"], {
      limit: { type: "string" },
      json: { type: "boolean" },
    })
    expect(r.error).toBeUndefined()
    expect(r.values.limit).toBe("3")
  })

  test("string flag without value is a type error", () => {
    const r = resolveArgs(["--limit"], args)
    expect(r.error).toBeInstanceOf(AggregateError)
    const e = (r.error as AggregateError).errors[0]
    expect(e).toBeInstanceOf(ArgValidationError)
    expect(String(e)).toBe("limit: Optional argument '--limit' should be 'string'")
  })

  test("boolean pending followed by another flag does not consume it as value", () => {
    const r = resolveArgs(["-c", "-d"], args)
    expect(r.error).toBeInstanceOf(AggregateError)
    const e = (r.error as AggregateError).errors[0]
    expect(String(e)).toBe("currency: Optional argument '--currency' or '-c' should be 'string'")
    expect(r.values.desc).toBe(true)
  })

  test("required option missing", () => {
    const r = resolveArgs([], args)
    const e = (r.error as AggregateError).errors[0]
    expect(String(e)).toBe("set: Optional argument '--set' is required")
  })

  test("required positional missing", () => {
    const r = resolveArgs([], { id: { type: "positional", required: true } })
    const e = (r.error as AggregateError).errors[0]
    expect(String(e)).toBe("id: Positional argument 'id' is required")
  })

  test("positionals include command path tokens; skip starts values after them", () => {
    const r = resolveArgs(["delete", "1", "2"], { id: { type: "positional", array: true, required: false } }, { skip: 1 })
    expect(r.positionals).toEqual(["delete", "1", "2"])
    expect(r.values.id).toEqual(["1", "2"])
  })

  test("array positional consumes remaining positionals", () => {
    const r = resolveArgs(["1", "2", "3"], { ids: { type: "positional", array: true, required: false } })
    expect(r.values.ids).toEqual(["1", "2", "3"])
  })

  test("array positional requires only when required: true", () => {
    const r = resolveArgs([], { ids: { type: "positional", array: true, required: false } })
    expect(r.error).toBeUndefined()
    expect(r.values.ids).toBeUndefined()
    const r2 = resolveArgs([], { ids: { type: "positional", array: true, required: true } })
    expect((r2.error as AggregateError).errors[0]).toBeInstanceOf(ArgValidationError)
  })

  test("repeated string flags accumulate when array: true", () => {
    const r = resolveArgs(["--tag", "a", "--tag", "b"], { tag: { type: "string", array: true } })
    expect(r.values.tag).toEqual(["a", "b"])
  })

  test("last occurrence wins for non-array string", () => {
    const r = resolveArgs(["--limit", "1", "--limit", "2"], args)
    expect(r.values.limit).toBe("2")
  })

  test("terminator sends positionals to rest", () => {
    const r = resolveArgs(["--", "--limit"], { limit: { type: "string" }, json: { type: "boolean" } })
    expect(r.rest).toEqual(["--limit"])
    expect(r.positionals).toEqual([])
    expect(r.error).toBeUndefined()
  })

  test("kebabized option names match camelCase schemas when toKebab", () => {
    const r = resolveArgs(["--input-tokens=5"], { inputTokens: { type: "string" } }, { toKebab: true })
    expect(r.values.inputTokens).toBe("5")
  })

  test("skip assignment with nested path (depth 2)", () => {
    const r = resolveArgs(
      ["usage", "delete", "5"],
      { id: { type: "positional", array: true, required: false } },
      { skip: 2 },
    )
    expect(r.positionals).toEqual(["usage", "delete", "5"])
    expect(r.values.id).toEqual(["5"])
  })
})

describe("resolveCommand", () => {
  const entry: Command = { name: "subtrack", run: () => undefined }
  const leaf: Command = { name: "delete", run: () => undefined }
  const parent: Command = {
    name: "tag",
    run: () => undefined,
    subCommands: { list: { name: "list", run: () => undefined }, rename: { name: "rename", run: () => undefined } },
  }
  const subs = { tag: parent, delete: leaf }

  test("empty positionals resolves the entry", () => {
    const r = resolveCommand([], entry, subs)
    expect(r.command).toBe(entry)
    expect(r.depth).toBe(0)
    expect(r.omitted).toBe(true)
    expect(r.unknownName).toBeUndefined()
  })

  test("top-level command resolves and stops at leaf (remaining positionals are args)", () => {
    const r = resolveCommand(["delete", "1", "2"], entry, subs)
    expect(r.command).toBe(leaf)
    expect(r.path).toEqual(["delete"])
    expect(r.depth).toBe(1)
    expect(r.omitted).toBe(false)
    expect(r.unknownName).toBeUndefined()
  })

  test("nested subcommand resolves", () => {
    const r = resolveCommand(["tag", "rename"], entry, subs)
    expect(r.path).toEqual(["tag", "rename"])
    expect(r.depth).toBe(2)
    expect(r.unknownName).toBeUndefined()
  })

  test("unknown top-level command yields parent + unknownName", () => {
    const r = resolveCommand(["bogus"], entry, subs)
    expect(r.unknownName).toBe("bogus")
    expect(r.command).toBe(entry)
    expect(r.depth).toBe(0)
  })

  test("token with no subcommands available is unknown", () => {
    const r = resolveCommand(["bogus"], entry, {})
    expect(r.unknownName).toBe("bogus")
    expect(r.command).toBe(entry)
    expect(r.depth).toBe(0)
  })

  test("unknown nested command yields the parent command", () => {
    const r = resolveCommand(["tag", "bogus"], entry, subs)
    expect(r.unknownName).toBe("bogus")
    expect(r.command).toBe(parent)
    expect(r.path).toEqual(["tag"])
  })

  test("CommandNotFoundError message matches gunshi", () => {
    const e = new CommandNotFoundError("Command not found: bogus")
    expect(String(e)).toBe("CommandNotFoundError: Command not found: bogus")
  })
})

describe("renderUsage", () => {
  const args: Args = {
    id: { type: "positional", array: true, required: false, description: "Subscription ID(s) to delete (omit for interactive selection)" },
    json: { type: "boolean", short: "j", description: "Output as JSON" },
    limit: { type: "string", description: "Max rows" },
  }
  const help: Args = {
    help: { type: "boolean", short: "h", description: "Display this help message" },
    version: { type: "boolean", short: "v", description: "Display this version" },
  }

  test("leaf command usage line with optional array positional", () => {
    const out = renderUsage({
      name: "subtrack",
      command: { name: "delete", description: "Delete subscriptions" },
      path: ["delete"],
      omitted: false,
      callMode: "subCommand",
      args: { ...help, ...args },
      subCommands: {},
    })
    expect(out).toContain("Delete subscriptions")
    expect(out).toContain("USAGE:\n  subtrack delete <OPTIONS> [<id> ...]")
    expect(out).toContain("ARGUMENTS:")
    expect(out).toContain("  id           Subscription ID(s) to delete (omit for interactive selection)")
    // options order: common flags first
    const optionsAt = out.indexOf("OPTIONS:")
    expect(optionsAt).toBeGreaterThan(out.indexOf("ARGUMENTS:"))
    expect(out.slice(optionsAt)).toContain("-h, --help")
    expect(out.slice(optionsAt)).toContain("-v, --version")
    expect(out.slice(optionsAt)).toContain("-j, --json")
    expect(out.slice(optionsAt)).toContain("--limit <limit>")
  })

  test("entry command renders COMMANDS and for-more hints", () => {
    const out = renderUsage({
      name: "subtrack",
      command: { name: "subtrack", description: "Manage subscription services from your terminal" },
      path: [],
      omitted: true,
      callMode: "entry",
      args: help,
      subCommands: {
        tag: { name: "tag", description: "Manage tags" },
        add: { name: "add", description: "Add a subscription" },
      },
    })
    expect(out).toContain("USAGE:\n  subtrack [COMMANDS] <OPTIONS>")
    expect(out).toContain("[subtrack] <OPTIONS>")
    expect(out).toContain("add <OPTIONS>")
    expect(out).toContain("tag <OPTIONS>")
    expect(out).toContain("For more info, run any command with the `--help` flag:")
    expect(out).toContain("  subtrack --help")
    expect(out).toContain("  subtrack add --help")
    // no description for the entry
    expect(out.startsWith("USAGE:")).toBe(true)
  })

  test("nested usage line uses command path", () => {
    const out = renderUsage({
      name: "subtrack",
      command: { name: "delete", description: "Delete LLM API usage entries" },
      path: ["usage", "delete"],
      omitted: false,
      callMode: "subCommand",
      args: { ...help, ...args },
      subCommands: {},
    })
    expect(out).toContain("USAGE:\n  subtrack usage delete <OPTIONS> [<id> ...]")
  })

  test("required positional is bracketless", () => {
    const out = renderUsage({
      name: "subtrack",
      command: { name: "export" },
      path: ["export"],
      omitted: false,
      callMode: "subCommand",
      args: {
        ...help,
        format: { type: "positional", required: true, description: "Export format" },
      },
      subCommands: {},
    })
    expect(out).toContain("USAGE:\n  subtrack export <OPTIONS> <format>")
  })
})

describe("cli", () => {
  const makeEntry = (run?: (ctx: any) => unknown) =>
    define({ name: "subtrack", description: "Manage subscription services from your terminal", run })

  const captureLogs = () => {
    const lines: string[] = []
    const spy = vi.spyOn(console, "log").mockImplementation((m?: unknown) => {
      lines.push(String(m))
    })
    return { lines, restore: () => spy.mockRestore() }
  }

  test("--version prints version without header", async () => {
    const { lines, restore } = captureLogs()
    try {
      const r = await cli(["--version"], makeEntry(), { name: "subtrack", version: "1.2.3" })
      expect(r).toBe("1.2.3")
      expect(lines).toEqual(["1.2.3"])
    } finally {
      restore()
    }
  })

  test("validation error renders header then message", async () => {
    const { lines, restore } = captureLogs()
    try {
      await expect(cli(["renew"], makeEntry(), { name: "subtrack", version: "1.2.3", subCommands: {
        renew: define({ name: "renew", args: { id: { type: "positional", required: true } }, run: () => undefined }),
      } })).rejects.toThrow(AggregateError)
      expect(lines[0]).toBe("subtrack (subtrack v1.2.3)")
      expect(lines[1]).toBe("")
      expect(lines[2]).toBe("Positional argument 'id' is required")
    } finally {
      restore()
    }
  })

  test("unknown command throws AggregateError with CommandNotFoundError, no header", async () => {
    const { lines, restore } = captureLogs()
    try {
      const err = await cli(["bogus"], makeEntry(), { name: "subtrack", version: "1.2.3", subCommands: {} }).catch(
        (e: unknown) => e,
      )
      expect(err).toBeInstanceOf(AggregateError)
      expect((err as AggregateError).errors[0]).toBeInstanceOf(CommandNotFoundError)
      expect(lines).toEqual(["Command not found: bogus"])
    } finally {
      restore()
    }
  })

  test("-v prints version", async () => {
    const { lines, restore } = captureLogs()
    try {
      await cli(["-v"], makeEntry(), { name: "subtrack", version: "9.9.9" })
      expect(lines).toEqual(["9.9.9"])
    } finally {
      restore()
    }
  })

  test("-h renders header and usage", async () => {
    const { lines, restore } = captureLogs()
    try {
      await cli(["list", "-h"], makeEntry(), {
        name: "subtrack",
        version: "1.0.0",
        subCommands: { list: define({ name: "list", description: "List all subscriptions", run: () => undefined }) },
      })
      expect(lines[0]).toBe("subtrack (subtrack v1.0.0)")
      expect(lines[1]).toBe("")
      expect(lines[2]).toContain("List all subscriptions")
      expect(lines[2]).toContain("USAGE:\n  subtrack list <OPTIONS>")
    } finally {
      restore()
    }
  })

  test("usageSilent suppresses output and returns usage string", async () => {
    const { lines, restore } = captureLogs()
    try {
      const r = await cli(["list", "--help"], makeEntry(), {
        name: "subtrack",
        version: "1.0.0",
        usageSilent: true,
        subCommands: { list: define({ name: "list", description: "List all subscriptions", run: () => undefined }) },
      })
      expect(lines).toEqual([])
      expect(r).toContain("subtrack (subtrack v1.0.0)")
      expect(r).toContain("subtrack list <OPTIONS>")
    } finally {
      restore()
    }
  })

  test("runs the command with resolved ctx", async () => {
    const { restore } = captureLogs()
    try {
      const seen: { values: unknown; positionals: string[] }[] = []
      const r = await cli(
        ["edit", "42", "--json"],
        makeEntry(),
        {
          name: "subtrack",
          version: "1.0.0",
          subCommands: {
            edit: define({
              name: "edit",
              args: { id: { type: "positional", required: true }, json: { type: "boolean" } },
              run: (ctx) => {
                seen.push({ values: ctx.values, positionals: ctx.positionals })
                return "EDITED"
              },
            }),
          },
        },
      )
      expect(r).toBe("EDITED")
      expect(seen[0].values).toEqual({ id: "42", json: true, help: undefined, version: undefined })
      expect(seen[0].positionals).toEqual(["edit", "42"])
    } finally {
      restore()
    }
  })

  test("bare entry runs without validation errors", async () => {
    const { lines, restore } = captureLogs()
    try {
      const r = await cli([], makeEntry(() => "MENU"), { name: "subtrack", version: "1.0.0", subCommands: {} })
      expect(r).toBe("MENU")
      expect(lines[0]).toBe("subtrack (subtrack v1.0.0)")
    } finally {
      restore()
    }
  })
})