/**
 * Shared building blocks for the self-contained prompt implementations.
 * Zero runtime dependencies — only Node built-ins.
 */

export type PromptStreams = {
  stdin?: NodeJS.ReadStream
  stdout?: NodeJS.WriteStream
}

export type Choice<T> = { value: T; name?: string; description?: string }

/** Error raised when a prompt is aborted (Ctrl+C, ESC, EOF, non-TTY). */
export class ExitPromptError extends Error {
  override name = "ExitPromptError"
  constructor(message = "Prompt aborted") {
    super(message)
  }
}

export function resolveStreams(config: PromptStreams) {
  const stdin = config.stdin ?? process.stdin
  const stdout = config.stdout ?? process.stdout
  // An explicitly injected stream is always treated as interactive so tests
  // (and embedded use) can drive prompts with an in-memory stream. The real
  // process stdin is only interactive when it is a TTY.
  const interactive = stdin.isTTY === true || config.stdin !== undefined
  return { stdin, stdout, interactive }
}

/** Resolve a fallback when input is non-interactive, or abort cleanly. */
export function fallback<T>(provided: boolean, value: T): T {
  if (provided) return value
  throw new ExitPromptError()
}

// ── ANSI helpers (self-contained subset) ─────────────────

export const cyan = (s: string) => `\x1b[36m${s}\x1b[39m`
export const green = (s: string) => `\x1b[32m${s}\x1b[39m`
export const dim = (s: string) => `\x1b[2m${s}\x1b[22m`
export const red = (s: string) => `\x1b[31m${s}\x1b[39m`

export const POINTER = "❯"
export const CHECKED = "◉"
export const UNCHECKED = "◯"

const HIDE_CURSOR = "\x1b[?25l"
const SHOW_CURSOR = "\x1b[?25h"

const ANSI_RE = /\u001b\[[0-9;]*m/g

export function stripAnsi(s: string): string {
  return s.replace(ANSI_RE, "")
}

const WIDE_RE =
  /[\u1100-\u115f\u2e80-\u303e\u3041-\u33ff\u3400-\u4dbf\u4e00-\u9fff\uac00-\ud7a3\uf900-\ufaff\ufe30-\ufe4f\uff00-\uff60\uffe0-\uffe6]/

/** Visible width: ANSI stripped, wide (CJK) chars count 2. */
export function strlen(s: string): number {
  let width = 0
  for (const ch of stripAnsi(s)) {
    width += WIDE_RE.test(ch) ? 2 : 1
  }
  return width
}

function safeWrite(stdout: NodeJS.WriteStream, s: string): void {
  try {
    stdout.write(s)
  } catch {
    // EPIPE or closed stream — nothing sensible to do.
  }
}

// ── Rendering helpers ────────────────────────────────────

/** Tracks the lines it rendered so the next render can redraw in-place. */
export class Renderer {
  private count = 0

  constructor(private readonly stdout: NodeJS.WriteStream) {}

  render(text: string): void {
    this.clear()
    if (text === "") return
    safeWrite(this.stdout, text)
    this.count = text.split("\n").length
  }

  clear(): void {
    if (this.count === 0) return
    safeWrite(this.stdout, `\r\x1b[${this.count}A\x1b[J`)
    this.count = 0
  }
}

/** Truncate to a visible width (ANSI codes stripped; wide chars count 2). */
export function truncate(s: string, maxWidth: number): string {
  if (maxWidth <= 0 || strlen(s) <= maxWidth) return s
  const plain = stripAnsi(s)
  let out = ""
  let width = 0
  for (const ch of plain) {
    const w = WIDE_RE.test(ch) ? 2 : 1
    if (width + w > maxWidth) break
    out += ch
    width += w
  }
  return out
}

// ── Raw-mode key input ───────────────────────────────────

export type Key = {
  name: string
  ctrl?: boolean
  str?: string
}

const CSI_NAMES: Record<string, string> = {
  A: "up",
  B: "down",
  C: "right",
  D: "left",
  H: "home",
  F: "end",
}

/**
 * Parse as many complete keys as possible from `buf`, leaving any dangling
 * bytes (a trailing ESC that may be the start of a sequence) in `rest`.
 */
export function parseKeys(buf: string): { keys: Key[]; rest: string } {
  const keys: Key[] = []
  let i = 0
  while (i < buf.length) {
    const ch = buf[i]!
    const code = ch.charCodeAt(0)
    if (ch === "\x1b") {
      const next = buf[i + 1]
      if (next === "[") {
        // CSI sequence — end byte is in @–~ (0x40–0x7e)
        let j = i + 2
        while (j < buf.length) {
          const c = buf.charCodeAt(j)!
          if (c >= 0x40 && c <= 0x7e) break
          j++
        }
        if (j >= buf.length) break // wait for more bytes
        const final = buf[j]!
        const mid = buf.slice(i + 2, j)
        const name = CSI_NAMES[final] ?? (/;\d/.test(mid) ? CSI_NAMES[final] ?? "" : "")
        if (!name && final !== "") {
          // Unknown CSI sequence — consume and ignore.
          i = j + 1
          continue
        }
        keys.push({ name })
        i = j + 1
      } else if (next === "O") {
        const final = buf[i + 2]
        if (final === undefined) break
        const name = { P: "f1", Q: "f2", R: "f3", S: "f4" }[final] ?? ""
        if (name) keys.push({ name })
        i += 3
      } else if (next === undefined) {
        break // lone trailing ESC — wait for possible sequence bytes
      } else {
        // Alt+key — treat as the escaped key.
        keys.push({ name: next })
        i += 2
      }
    } else if (ch === "\r" || ch === "\n" || code === 13 || code === 10) {
      keys.push({ name: "return" })
      i++
    } else if (code === 3) {
      keys.push({ name: "ctrl-c", ctrl: true })
      i++
    } else if (ch === "\t") {
      keys.push({ name: "tab" })
      i++
    } else if (code === 127 || code === 8) {
      keys.push({ name: "backspace" })
      i++
    } else if (code === 0) {
      i++ // ignore null bytes
    } else {
      const char = String.fromCodePoint(code)
      keys.push({ name: char, str: char })
      i++
    }
  }
  return { keys, rest: buf.slice(i) }
}

const ESC_DEBOUNCE_MS = 50

/** Buffers stdin data and hands out parsed keys as promises. */
export function createKeyReader(stdin: NodeJS.ReadStream) {
  let buffer = ""
  const queued: Key[] = []
  const waiters: ((k: Key) => void)[] = []
  let escTimer: ReturnType<typeof setTimeout> | null = null
  let ended = false

  const cancelEscTimer = () => {
    if (escTimer !== null) {
      clearTimeout(escTimer)
      escTimer = null
    }
  }

  const push = (k: Key) => {
    const waiter = waiters.shift()
    if (waiter) waiter(k)
    else queued.push(k)
  }

  const flush = () => {
    cancelEscTimer()
    if (buffer === "") return
    const { keys, rest } = parseKeys(buffer)
    buffer = rest
    for (const k of keys) push(k)
    if (buffer === "\x1b" && !ended) {
      // A lone ESC may be the prefix of an arrow sequence arriving in the
      // next chunk. Wait briefly before committing to the escape key.
      escTimer = setTimeout(() => {
        escTimer = null
        buffer = ""
        push({ name: "escape" })
      }, ESC_DEBOUNCE_MS)
    }
  }

  const onData = (chunk: string | Buffer) => {
    if (ended) return
    buffer += typeof chunk === "string" ? chunk : chunk.toString("utf8")
    flush()
  }

  const onEnd = () => {
    if (ended) return
    ended = true
    cancelEscTimer()
    push({ name: "eof" })
  }

  const onPause = () => {
    // Some streams pause after flushing piped data; nothing to do here.
  }

  stdin.on("data", onData)
  stdin.on("end", onEnd)
  stdin.on("pause", onPause)

  function readKey(): Promise<Key> {
    const queuedKey = queued.shift()
    if (queuedKey) return Promise.resolve(queuedKey)
    if (ended) return Promise.resolve({ name: "eof" })
    return new Promise((resolve) => waiters.push(resolve))
  }

  function close() {
    cancelEscTimer()
    stdin.removeListener("data", onData)
    stdin.removeListener("end", onEnd)
    stdin.removeListener("pause", onPause)
  }

  return { readKey, close }
}

/** Run `impl` with stdin in raw mode, guaranteeing the terminal is restored. */
export async function withRawMode<T>(
  stdin: NodeJS.ReadStream,
  stdout: NodeJS.WriteStream,
  impl: (io: { readKey: () => Promise<Key>; print: (s: string) => void }) => Promise<T>,
): Promise<T> {
  stdin.setRawMode?.(true)
  stdin.resume()
  safeWrite(stdout, HIDE_CURSOR)
  const reader = createKeyReader(stdin)
  try {
    return await impl({
      readKey: () => reader.readKey(),
      print: (s) => safeWrite(stdout, s),
    })
  } finally {
    reader.close()
    stdin.setRawMode?.(false)
    stdin.pause()
    safeWrite(stdout, SHOW_CURSOR)
  }
}

/** Print the final answered line once a prompt resolves. */
export function finishPrompt(
  stdout: NodeJS.WriteStream,
  message: string,
  result: string,
): void {
  safeWrite(stdout, `\r${cyan("?")} ${message} ${green(result)}\n`)
}