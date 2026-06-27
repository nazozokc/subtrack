import { useEffect, useRef, useState } from "react"

export interface MouseState {
  /** Column (1-based) */
  x: number
  /** Row (1-based) */
  y: number
  /** 0=left, 1=middle, 2=right, 3=release */
  button: number
  /** True on press, false on release */
  pressed: boolean
}

/**
 * Enable SGR mouse tracking (mode 1006) and return click events.
 *
 * Must be called inside an Ink `<App>` — relies on `process.stdin`
 * already being in raw mode (set by Ink).
 *
 * Coordinates are 1‑based terminal rows/cols.
 * Callers should heuristically map these to rendered components.
 */
export function useMouse(): MouseState | null {
  const [click, setClick] = useState<MouseState | null>(null)
  const buf = useRef("")

  useEffect(() => {
    const stdin = process.stdin
    if (!stdin || !stdin.isTTY) return

    // Enable SGR mouse mode
    // 1000 = basic tracking, 1002 = button-event tracking, 1006 = SGR coords
    const enable = "\x1b[?1000h\x1b[?1002h\x1b[?1006h"
    stdin.write(enable)

    const onData = (chunk: Buffer) => {
      buf.current += chunk.toString()

      // SGR mouse format: ESC [ <button> ; <col> ; <row> M / m
      const re = /\x1b\[<(\d+);(\d+);(\d+)([Mm])/g
      let match: RegExpExecArray | null = null
      let last: RegExpExecArray | null = null
      while ((match = re.exec(buf.current)) !== null) {
        last = match
      }

      if (last) {
        const [, rawBtn, rawX, rawY, kind] = last
        const btn = parseInt(rawBtn, 10)
        setClick({
          x: parseInt(rawX, 10),
          y: parseInt(rawY, 10),
          button: btn & 3,       // 0=left, 1=middle, 2=right
          pressed: kind === "M", // M=press, m=release
        })
        // Consume processed data up to the end of the last match
        buf.current = buf.current.slice(re.lastIndex)
      }
    }

    stdin.on("data", onData)

    return () => {
      const disable = "\x1b[?1006l\x1b[?1002l\x1b[?1000l"
      stdin.write(disable)
      stdin.off("data", onData)
    }
  }, [])

  return click
}
