import { test, expect, describe, afterAll } from "vitest"
import { mkdtempSync, writeFileSync, mkdirSync, symlinkSync, existsSync, rmSync } from "node:fs"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { resolveSafePath, resolveSafeOutputPath } from "@subtrack/lib/path"

/**
 * Path-safety helpers must accept files directly inside a trusted base
 * directory (e.g. `~/backup.db.gz`), not only files inside nested
 * subdirectories — regression test for the `isWithin` equality bug.
 */
describe("resolveSafeOutputPath", () => {
  const base = mkdtempSync(join(tmpdir(), "subtrack-path-base-"))
  const nested = join(base, "nested")
  mkdirSync(nested)

  afterAll(() => {
    rmSync(base, { recursive: true, force: true })
  })

  test("accepts a new file directly inside the base directory", () => {
    const target = join(base, "new-file.db.gz")
    expect(resolveSafeOutputPath([base], target)).toBe(target)
  })

  test("accepts a new file inside an existing nested directory", () => {
    const target = join(nested, "new-file.json")
    expect(resolveSafeOutputPath([base], target)).toBe(target)
  })

  test("accepts the base directory itself", () => {
    expect(resolveSafeOutputPath([base], base)).toBe(base)
  })

  test("rejects a path that escapes the base directory", () => {
    expect(resolveSafeOutputPath([base], join(base, "..", "etc", "evil.conf"))).toBeNull()
  })
})

describe("resolveSafePath", () => {
  const base = mkdtempSync(join(tmpdir(), "subtrack-path-existing-"))
  const file = join(base, "existing.db")
  writeFileSync(file, "data")

  afterAll(() => {
    rmSync(base, { recursive: true, force: true })
  })

  test("accepts an existing file directly inside the base directory", () => {
    expect(resolveSafePath([base], file)).toBe(file)
  })

  test("rejects a non-existent file", () => {
    expect(resolveSafePath([base], join(base, "missing.db"))).toBeNull()
  })

  test("rejects a symlink escaping the base directory", () => {
    const outside = mkdtempSync(join(tmpdir(), "subtrack-path-outside-"))
    const escapeTarget = join(outside, "secret.txt")
    writeFileSync(escapeTarget, "secret")
    const link = join(base, "escape-link")
    symlinkSync(escapeTarget, link)
    try {
      expect(resolveSafePath([base], link)).toBeNull()
    } finally {
      rmSync(outside, { recursive: true, force: true })
    }
  })
})