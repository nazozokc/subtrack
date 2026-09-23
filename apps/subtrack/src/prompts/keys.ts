/**
 * Raw-mode key input machinery: byte/sequence parsing and the interactive
 * key reader used by the prompts, plus the raw-mode lifecycle guard that
 * guarantees the terminal is restored on every exit path.
 */

import { safeWrite } from "./core.ts"

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

const HIDE_CURSOR = "\x1b[?25l"
const SHOW_CURSOR = "\x1b[?25h"

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