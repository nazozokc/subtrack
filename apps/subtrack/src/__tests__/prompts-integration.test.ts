/**
 * Integration tests for the self-contained prompts (`src/prompts/`).
 *
 * Drives the real prompts with an injected in-memory stdin/stdout pair so the
 * interactive key-loop (raw mode, validation retry, navigation) is exercised
 * end-to-end, plus the non-TTY fallback path for `process.stdin`.
 */
import { Writable, PassThrough } from "node:stream"
import { describe, it, expect, vi } from "vitest"
import { input, confirm, select, checkbox, isValidCycle, validateCycleDays, promptCycle } from "../prompts.ts"
import { Renderer } from "../prompts/renderer.ts"
import { strlen, takeTail } from "../prompts/ansi.ts"

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

  it("shows the value tail with an ellipsis when it overflows the line", async () => {
    const { stdin, stdout } = makeIO()
    ;(stdout as { columns?: number }).columns = 40
    const p = input({ message: "name", stdin, stdout })
    await tick()
    await feed(stdin, "a".repeat(45) + "\r")
    await expect(p).resolves.toBe("a".repeat(45))
    expect(stdout.toString()).toContain("…")
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
    const final = stdout.toString()
    expect(final).toContain("✔")
    expect(final).toContain("\x1b[32myes")
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

  it("highlights the active row and confirms the pick with ✔", async () => {
    const { stdin, stdout } = makeIO()
    const p = select({ message: "pick", choices, stdin, stdout })
    await tick()
    const first = stdout.toString()
    expect(first).toContain("❯")
    // the active row's name is rendered bold-cyan
    expect(first).toContain("\x1b[36m\x1b[1mAlpha")
    await feed(stdin, "\x1b[B\r")
    await expect(p).resolves.toBe("b")
    const final = stdout.toString()
    expect(final).toContain("✔")
    expect(final).toContain("\x1b[32mBeta")
  })
})

describe("checkbox", () => {
  const choices = [
    { name: "Alpha", value: "a" },
    { name: "Beta", value: "b" },
    { name: "Gamma", value: "c" },
  ]

  it("toggles with space, shows a live count, and confirms with glyphs", async () => {
    const { stdin, stdout } = makeIO()
    const p = checkbox({ message: "pick", choices, stdin, stdout })
    await tick()
    await feed(stdin, " \x1b[B \r") // check Alpha, move down, check Beta, enter
    await expect(p).resolves.toEqual(["a", "b"])
    const out = stdout.toString()
    // checked rows render the name bold-cyan
    expect(out).toContain("\x1b[36m\x1b[1mAlpha")
    // live counter footer
    expect(out).toContain("2 selected")
    // final line: ✔ + green ◉-prefixed names
    expect(out).toContain("✔")
    expect(out).toContain("◉ Alpha, ◉ Beta")
  })

  it("shows none when nothing is selected", async () => {
    const { stdin, stdout } = makeIO()
    const p = checkbox({ message: "pick", choices, stdin, stdout })
    await tick()
    await feed(stdin, "\r")
    await expect(p).resolves.toEqual([])
    expect(stdout.toString()).toContain("none")
  })
})

describe("Renderer", () => {
  it("prefixes every line with a carriage return (raw mode has no ONLCR)", () => {
    const { stdout } = makeIO()
    new Renderer(stdout).render("line1\nline2\nline3")
    expect(stdout.toString()).toBe("\rline1\n\rline2\n\rline3")
  })

  it("moves up count - 1 rows when clearing in-place (no drift on re-render)", () => {
    const { stdout } = makeIO()
    const renderer = new Renderer(stdout)
    renderer.render("line1\nline2\nline3")
    stdout.chunks = []
    renderer.clear()
    expect(stdout.toString()).toBe("\r\x1b[2A\x1b[J")
  })

  it("erases a single-line prompt in place (x1b[0A moves up on real terminals)", () => {
    const { stdout } = makeIO()
    const renderer = new Renderer(stdout)
    renderer.render("? name")
    stdout.chunks = []
    renderer.clear()
    expect(stdout.toString()).toBe("\r\x1b[K")
  })

  it("truncates over-width lines so they never wrap (physical rows == count)", () => {
    const { stdout } = makeIO()
    ;(stdout as { columns?: number }).columns = 40
    const renderer = new Renderer(stdout)
    renderer.render("x".repeat(100))
    const out = stdout.toString()
    expect(out.startsWith("\r")).toBe(true)
    expect(out.slice(1)).toHaveLength(38) // columns - 2 margin
  })
})

describe("ansi width helpers", () => {
  it("keeps the tail of a too-long string", () => {
    expect(takeTail("abcdef", 4)).toBe("cdef")
    expect(takeTail("ab", 10)).toBe("ab")
  })

  it("is CJK-width aware", () => {
    expect(takeTail("あいうえお", 4)).toBe("えお")
    expect(strlen("えお")).toBe(4)
  })
})

describe("cycle validation", () => {
  it("accepts named cycles and valid custom day cycles", () => {
    expect(isValidCycle("monthly")).toBe(true)
    expect(isValidCycle("yearly")).toBe(true)
    expect(isValidCycle("weekly")).toBe(true)
    expect(isValidCycle("1d")).toBe(true)
    expect(isValidCycle("3d")).toBe(true)
    expect(isValidCycle("365d")).toBe(true)
  })

  it("rejects invalid day cycles", () => {
    expect(isValidCycle("0d")).toBe(false)
    expect(isValidCycle("366d")).toBe(false)
    expect(isValidCycle("d")).toBe(false)
    expect(isValidCycle("3")).toBe(false)
    expect(isValidCycle("3D")).toBe(false)
    expect(isValidCycle("3 days")).toBe(false)
  })

  it("validates day input between 1 and 365", () => {
    expect(validateCycleDays("3")).toBe(true)
    expect(validateCycleDays("365")).toBe(true)
    expect(validateCycleDays("0")).toBeTruthy()
    expect(validateCycleDays("366")).toBeTruthy()
    expect(validateCycleDays("abc")).toBeTruthy()
  })
})

describe("promptCycle", () => {
  it("accepts a valid flag without prompting", async () => {
    await expect(promptCycle("3d")).resolves.toEqual({ value: "3d", prompted: false })
    await expect(promptCycle("monthly")).resolves.toEqual({ value: "monthly", prompted: false })
  })

  it("rejects an invalid flag", async () => {
    await expect(promptCycle("0d")).resolves.toBeNull()
  })

  it("supports the custom entry and collects days interactively", async () => {
    const { stdin, stdout } = makeIO()
    const p = promptCycle(undefined, "Cycle:", { stdin, stdout })
    await tick()
    await feed(stdin, "\x1b[B\x1b[B\x1b[B\x1b[B\x1b[B\x1b[B\r") // pick "custom (every N days)"
    await tick()
    await feed(stdin, "3\r") // days input
    await expect(p).resolves.toEqual({ value: "3d", prompted: true })
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