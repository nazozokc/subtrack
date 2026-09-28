import { test, expect, beforeEach, afterEach } from "vitest"
import { execFileSync } from "node:child_process"
import { mkdtempSync, rmSync, writeFileSync, mkdirSync } from "node:fs"
import { join, resolve, dirname } from "node:path"
import { tmpdir } from "node:os"
import { fileURLToPath } from "node:url"

// The checker is the thing that keeps the layer rules enforced, so its own
// blind spots matter: a specifier it fails to see is a boundary a future
// change can walk straight through. These cases pin the shapes it must catch.
//
// The fixture tree needs the imported modules to actually exist, because the
// checker resolves a specifier to a real file before judging it.
const here = resolve(fileURLToPath(import.meta.url), "../../..")
const script = join(here, "scripts/arch-check.mjs")

let fixture: string

/** Write a file into the fixture tree, creating parent directories. */
function write(relative: string, source: string): void {
  const target = join(fixture, relative)
  mkdirSync(dirname(target), { recursive: true })
  writeFileSync(target, source)
}

/** The persistence modules a handler might be reaching for. */
function writeDbLayer(): void {
  write("db.ts", "export const getDb = () => ({})\n")
  write("db/connection.ts", "export const getDb = () => ({})\nexport const saveDb = () => {}\n")
}

function run(): { status: number; output: string } {
  let raw: string
  let status = 0
  try {
    raw = execFileSync(process.execPath, [script, "--report"], {
      env: { ...process.env, ARCH_CHECK_SRC: fixture },
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    })
  } catch (error) {
    const e = error as { status: number; stdout: string; stderr: string }
    status = e.status
    raw = `${e.stdout ?? ""}${e.stderr ?? ""}`
  }
  // The report contains real paths, so Windows separators are normalised before
  // the assertions rather than duplicated per platform.
  return { status, output: raw.replace(/\\/g, "/") }
}

beforeEach(() => {
  fixture = mkdtempSync(join(tmpdir(), "subtrack-arch-"))
})

afterEach(() => {
  rmSync(fixture, { recursive: true, force: true })
})

test("a static db import is reported", () => {
  writeDbLayer()
  write("handler.ts", `import { getDb } from "./db.ts"\nexport const h = () => getDb()\n`)

  const { status, output } = run()

  expect(status).toBe(0) // --report reports and exits 0
  expect(output).toContain("handler.ts")
  expect(output).toContain("no-direct-db")
  expect(output).toContain("db.ts")
})

test("a dynamic db import is reported too", () => {
  // `await import("../db.ts")` crosses the same boundary as a static import.
  // If the scanner misses it, the ratchet has a hole in it.
  writeDbLayer()
  write("handlers/h.ts", `export const h = async () => (await import("../db.ts")).getDb()\n`)

  const { output } = run()

  expect(output).toContain("handlers/h.ts")
  expect(output).toContain("no-direct-db")
})

test("a dynamic import of a db submodule is reported", () => {
  writeDbLayer()
  write("handlers/h.ts", `export const h = async () => import("../db/connection.ts")\n`)

  const { output } = run()

  expect(output).toContain("no-direct-db")
  expect(output).toContain("db/connection.ts")
})

test("a dynamic import through a promise chain is reported", () => {
  writeDbLayer()
  write("handlers/h.ts", `export const h = () => import (\n  "../db.ts"\n)\n`)

  const { output } = run()

  expect(output).toContain("no-direct-db")
})

test("a dynamic import of an application module is fine", () => {
  write("application/index.ts", "export const repo = {}\n")
  write("handlers/h.ts", `export const h = async () => import("../application/index.ts")\n`)

  const { output } = run()

  expect(output).not.toContain("no-direct-db")
})

test("a string that merely looks like an import is not a specifier", () => {
  writeDbLayer()
  write("handlers/h.ts", `const label = 'import("../db.ts")'\nexport const h = () => label\n`)

  const { output } = run()

  expect(output).not.toContain("no-direct-db")
})

test("the db layer may import the db layer", () => {
  writeDbLayer()
  write("db/handler.ts", `import { getDb } from "./connection.ts"\nexport const h = () => getDb()\n`)

  const { output } = run()

  expect(output).not.toContain("no-direct-db")
})
