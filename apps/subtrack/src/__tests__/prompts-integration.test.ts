/**
 * Integration tests for the self-contained prompts (`src/prompts/`).
 *
 * Drives the real prompts with an injected in-memory stdin/stdout pair so the
 * interactive key-loop (raw mode, validation retry, navigation) is exercised
 * end-to-end, plus the non-TTY fallback path for `process.stdin`.
 */
import { Writable, PassThrough } from "node:stream"
import { describe, it, expect, vi } from "vitest"
import { input, confirm, select } from "../prompts.ts"

class Capture extends Writable {
  chunks: string[] = []
  _write(chunk: Buffer, _enc: BufferEncoding, cb: (error?: Error | null) => void): void {
    this.chunks.push(String(chunk))
    cb()
  }
  toString(): string {
    return this.chunks.join("")
  }
}

type FakeStdin = NodeJS.ReadStream & {
  setRawMode: (mode: boolean) => void
  rawMode: boolean
}

function makeIO() {
  const stdin = new PassThrough() as unknown as FakeStdin
  const setRawMode = vi.fn((mode: boolean) => {
    stdin.rawMode = mode
  })
  stdin.setRawMode = setRawMode
  stdin.rawMode = false
  const stdout = new Capture() as unknown as NodeJS.WriteStream
  return { stdin, stdout, setRawMode }
}

const tick = (): Promise<void> => new Promise((resolve) => setImmediate(resolve))

async function feed(stdin: NodeJS.ReadStream, data: string | Buffer): Promise<void> {
  stdin.write(data)
  await tick()
}

describe("input", () => {
  it("collects typed text and resolves on Enter", async () => {
    const { stdin, stdout, setRawMode } = makeIO()
    const p = input({ message: "name", stdin, stdout })
    await tick()
    await feed(stdin, "abc\r")
    await expect(p).resolves.toBe("abc")
    expect(setRawMode).toHaveBeenCalledWith(true)
    expect(setRawMode).toHaveBeenLastCalledWith(false)
    expect(stdout.toString()).toContain("abc")
  })

  it("re-renders the validation error until input is valid", async () => {
    const { stdin, stdout } = makeIO()
    const p = input({
      message: "name",
      validate: (v) => v.length >= 3 || "Too short",
      stdin,
      stdout,
    })
    await tick()
    await feed(stdin, "ab\r")
    expect(stdout.toString()).toContain("Too short")
    await feed(stdin, "c\r")
    await expect(p).resolves.toBe("abc")
  })

  it("keeps the typed value when EOF arrives before Enter", async () => {
    const { stdin, stdout, setRawMode } = makeIO()
    const p = input({ message: "name", stdin, stdout })
    await tick()
    await feed(stdin, "half")
    stdin.end()
    await tick()
    await expect(p).resolves.toBe("half")
    expect(setRawMode).toHaveBeenLastCalledWith(false)
  })

  it("resolves the default on EOF when empty", async () => {
    const { stdin, stdout } = makeIO()
    const p = input({ message: "name", default: "dflt", stdin, stdout })
    await tick()
    stdin.end()
    await tick()
    await expect(p).resolves.toBe("dflt")
  })

  it("rejects with ExitPromptError on Ctrl+C", async () => {
    const { stdin, stdout, setRawMode } = makeIO()
    const p = input({ message: "name", stdin, stdout })
    p.catch(() => {})
    await tick()
    await feed(stdin, "\x03")
    await expect(p).rejects.toMatchObject({ name: "ExitPromptError" })
    expect(setRawMode).toHaveBeenLastCalledWith(false)
  })

  it("rejects with ExitPromptError on EOF without a default", async () => {
    const { stdin, stdout } = makeIO()
    const p = input({ message: "name", stdin, stdout })
    p.catch(() => {})
    await tick()
    stdin.end()
    await tick()
    await expect(p).rejects.toMatchObject({ name: "ExitPromptError" })
  })
})

describe("confirm", () => {
  it("resolves true on y", async () => {
    const { stdin, stdout, setRawMode } = makeIO()
    const p = confirm({ message: "continue?", stdin, stdout })
    await tick()
    await feed(stdin, "y\r")
    await expect(p).resolves.toBe(true)
    expect(setRawMode).toHaveBeenLastCalledWith(false)
  })

  it("resolves false on n", async () => {
    const { stdin, stdout } = makeIO()
    const p = confirm({ message: "continue?", stdin, stdout })
    await tick()
    await feed(stdin, "n\r")
    await expect(p).resolves.toBe(false)
  })

  it("resolves the default on bare Enter and shows the default hint", async () => {
    const { stdin, stdout } = makeIO()
    const p = confirm({ message: "continue?", default: false, stdin, stdout })
    await tick()
    expect(stdout.toString()).toContain("(y/N)")
    await feed(stdin, "\r")
    await expect(p).resolves.toBe(false)
  })

  it("shows (Y/n) for a true default", async () => {
    const { stdin, stdout } = makeIO()
    const p = confirm({ message: "continue?", default: true, stdin, stdout })
    await tick()
    expect(stdout.toString()).toContain("(Y/n)")
    await feed(stdin, "\r")
    await expect(p).resolves.toBe(true)
  })
})

describe("select", () => {
  const choices = [
    { name: "Alpha", value: "a" },
    { name: "Beta", value: "b" },
    { name: "Gamma", value: "c" },
  ]

  it("resolves the current choice on Enter", async () => {
    const { stdin, stdout, setRawMode } = makeIO()
    const p = select({ message: "pick", choices, stdin, stdout })
    await tick()
    await feed(stdin, "\r")
    await expect(p).resolves.toBe("a")
    expect(setRawMode).toHaveBeenLastCalledWith(false)
  })

  it("moves with arrow keys", async () => {
    const { stdin, stdout } = makeIO()
    const p = select({ message: "pick", choices, stdin, stdout })
    await tick()
    await feed(stdin, "\x1b[B\x1b[B\r")
    await expect(p).resolves.toBe("c")
  })

  it("wraps around when loop is enabled", async () => {
    const { stdin, stdout } = makeIO()
    const p = select({ message: "pick", choices, loop: true, stdin, stdout })
    await tick()
    await feed(stdin, "\x1b[A\r")
    await expect(p).resolves.toBe("c")
  })

  it("filters by typeahead", async () => {
    const { stdin, stdout } = makeIO()
    const p = select({ message: "pick", choices, stdin, stdout })
    await tick()
    await feed(stdin, "bet\r")
    await expect(p).resolves.toBe("b")
  })

  it("resolves the default on EOF when empty", async () => {
    const { stdin, stdout } = makeIO()
    const p = select({ message: "pick", choices, default: "b", stdin, stdout })
    await tick()
    stdin.end()
    await tick()
    await expect(p).resolves.toBe("b")
  })

  it("rejects with ExitPromptError on EOF without a default", async () => {
    const { stdin, stdout } = makeIO()
    const p = select({ message: "pick", choices, stdin, stdout })
    p.catch(() => {})
    await tick()
    stdin.end()
    await tick()
    await expect(p).rejects.toMatchObject({ name: "ExitPromptError" })
  })
})

describe("non-TTY fallback (default process.stdin)", () => {
  it("falls back to defaults or rejects cleanly", async () => {
    const orig = Reflect.getOwnPropertyDescriptor(process.stdin, "isTTY")
    Object.defineProperty(process.stdin, "isTTY", { value: false, configurable: true })
    try {
      await expect(input({ message: "name", default: "dflt" })).resolves.toBe("dflt")
      await expect(input({ message: "name" })).rejects.toMatchObject({ name: "ExitPromptError" })
      await expect(confirm({ message: "ok", default: false })).resolves.toBe(false)
      await expect(confirm({ message: "ok" })).rejects.toMatchObject({ name: "ExitPromptError" })
      await expect(select({ message: "pick", choices: ["a", "b"], default: "b" })).resolves.toBe("b")
      await expect(select({ message: "pick", choices: ["a", "b"] })).rejects.toMatchObject({
        name: "ExitPromptError",
      })
    } finally {
      if (orig) Object.defineProperty(process.stdin, "isTTY", orig)
      else delete (process.stdin as NodeJS.ReadStream & { isTTY?: boolean }).isTTY
    }
  })
})