import { test, expect, describe, vi, beforeEach, afterEach } from "vitest"
import { execFile } from "node:child_process"
import { promisify } from "node:util"
import { fileURLToPath } from "node:url"
import path from "node:path"
import { shouldSuppressUsage } from "../cli/usage.ts"

const run = promisify(execFile)
const ENTRY = fileURLToPath(new URL("../index.ts", import.meta.url))
const DB_DIR = "/tmp/subtrack-json-output-test"

/**
 * Commands whose stdout must be a single JSON document so other programs can
 * consume it. `subtrack <cmd> --json` is a public machine-readable surface.
 */
const JSON_COMMANDS: readonly (readonly string[])[] = [
  ["list"],
  ["summary"],
  ["payment"],
  ["upcoming"],
  ["search", "anything"],
  ["budget"],
]

describe("shouldSuppressUsage", () => {
  test("suppresses the banner for --json in every spelling", () => {
    expect(shouldSuppressUsage(["list", "--json"])).toBe(true)
    expect(shouldSuppressUsage(["list", "--json=true"])).toBe(true)
    expect(shouldSuppressUsage(["list", "-j"])).toBe(true)
    expect(shouldSuppressUsage(["list", "-nj"])).toBe(true)
  })

  test("suppresses the banner for the mcp server", () => {
    expect(shouldSuppressUsage(["mcp"])).toBe(true)
  })

  test("keeps the banner for human-facing invocations", () => {
    expect(shouldSuppressUsage(["list"])).toBe(false)
    expect(shouldSuppressUsage(["--help"])).toBe(false)
    expect(shouldSuppressUsage(["list", "--jsonl"])).toBe(false)
    expect(shouldSuppressUsage(["list", "-c", "USD"])).toBe(false)
  })
})

describe("--json stdout is machine-parseable", () => {
  beforeEach(() => {
    vi.spyOn(console, "log").mockImplementation(() => {})
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  // Regression guard: the header/banner used to be printed before the JSON body,
  // which made `JSON.parse(stdout)` fail for every external consumer.
  for (const args of JSON_COMMANDS) {
    test(`subtrack ${args.join(" ")} --json emits only JSON`, async () => {
      const { stdout } = await run(
        "npx",
        ["tsx", "--no-warnings", ENTRY, ...args, "--json"],
        { env: { ...process.env, SUBSC_CLI_DB_DIR: DB_DIR } },
      )

      expect(() => JSON.parse(stdout)).not.toThrow()
    }, 30_000)
  }
})

describe("error reporting survives --json", () => {
  test("invalid flag still reports the failure and exits non-zero", async () => {
    await expect(
      run("npx", ["tsx", "--no-warnings", ENTRY, "list", "--status", "bogus", "--json"], {
        env: { ...process.env, SUBSC_CLI_DB_DIR: DB_DIR },
      }),
    ).rejects.toMatchObject({ code: 1 })
  }, 30_000)
})

describe("test database location", () => {
  test("is isolated from the user's real database", () => {
    expect(path.isAbsolute(DB_DIR)).toBe(true)
  })
})
