import { test, expect, describe } from "vitest"
import { subCommands } from "../commands/index.ts"
import { commandLoaders } from "../commands/lazy.ts"

describe("lazy command loaders", () => {
  test("cover exactly the same command names as the barrel", () => {
    expect(Object.keys(commandLoaders).sort()).toEqual(Object.keys(subCommands).sort())
  })

  test("resolve to the very same definition objects as the barrel", async () => {
    for (const [name, expected] of Object.entries(subCommands)) {
      const loaded = await commandLoaders[name]()
      expect(loaded, `loader for "${name}"`).toBe(expected)
    }
  })

  test("each loader is reusable and memoizable by the module system", async () => {
    // Loading the same command twice must yield the identical definition
    // (dynamic imports go through the module cache).
    const first = await commandLoaders["list"]()
    const second = await commandLoaders["list"]()
    expect(first).toBe(second)
  })
})