/**
 * Self-contained table renderer, drop-in replacement for the subset of
 * `cli-table3` used by subtrack. Zero runtime dependencies.
 *
 * Supported options: chars, style (border/head/padding/compact), colWidths,
 * head, colAligns, wordWrap, wrapOnWordBoundary, truncate.
 *
 * Rendering follows the behavior subtrack relied on from cli-table3:
 * - compact: false — a border line is drawn before every row (mid-style for
 *   rows below the first)
 * - cell content is truncated with "…" when it exceeds the column width
 * - wordWrap wraps on word boundaries (ANSI-aware)
 * - East Asian wide characters count as width 2
 */

// ── string width ──────────────────────────────────────────

const ANSI_RE = /\u001b\[[0-9;]*m/g

const WIDE_RE =
  /[\u1100-\u115f\u2e80-\u303e\u3041-\u33ff\u3400-\u4dbf\u4e00-\u9fff\uac00-\ud7a3\uf900-\ufaff\ufe30-\ufe4f\uff00-\uff60\uffe0-\uffe6]/

/** Visible width of a string: ANSI codes stripped, wide (CJK) chars count 2. */
export function strlen(str: string): number {
  let width = 0
  for (const ch of str.replace(ANSI_RE, "")) {
    width += WIDE_RE.test(ch) ? 2 : 1
  }
  return width
}

function repeat(str: string, times: number): string {
  return Array(times + 1).join(str)
}

function pad(str: string, len: number, padChar: string, dir: "left" | "right" | "center"): string {
  const length = strlen(str)
  if (len + 1 >= length) {
    const padLen = len - length
    switch (dir) {
      case "right":
        str = repeat(padChar, padLen) + str
        break
      case "center": {
        const right = Math.ceil(padLen / 2)
        const left = padLen - right
        str = repeat(padChar, left) + str + repeat(padChar, right)
        break
      }
      default:
        str = str + repeat(padChar, padLen)
    }
  }
  return str
}

// ── ANSI state tracking ───────────────────────────────────

const FG_CODES = new Set<number>([
  30, 31, 32, 33, 34, 35, 36, 37, 90, 91, 92, 93, 94, 95, 96, 97, 38,
])
const BG_CODES = new Set<number>([
  40, 41, 42, 43, 44, 45, 46, 47, 100, 101, 102, 103, 104, 105, 106, 107, 48,
])
const ATTR_OFF: Record<number, number> = { 1: 22, 2: 22, 3: 23, 4: 24, 7: 27, 9: 29 }

function applySgr(state: Set<number>, code: number): void {
  if (code === 0) {
    state.clear()
    return
  }
  if (code === 39) {
    for (const c of state) if (FG_CODES.has(c)) state.delete(c)
    return
  }
  if (code === 49) {
    for (const c of state) if (BG_CODES.has(c)) state.delete(c)
    return
  }
  if (ATTR_OFF[code] !== undefined) {
    state.delete(ATTR_OFF[code])
    return
  }
  if (FG_CODES.has(code)) {
    for (const c of state) if (FG_CODES.has(c)) state.delete(c)
    state.add(code)
    return
  }
  if (BG_CODES.has(code)) {
    for (const c of state) if (BG_CODES.has(c)) state.delete(c)
    state.add(code)
    return
  }
  if (code in ATTR_OFF) state.add(code)
}

/** Read the SGR state at the end of a string. */
function readSgr(str: string): Set<number> {
  const state = new Set<number>()
  const re = /\u001b\[([0-9;]*)m/g
  for (const m of str.matchAll(re)) {
    const raw = m[1]
    const codes = raw === "" ? [0] : raw.split(";").map((c) => parseInt(c, 10))
    // Bare 38/48 (16-color) or 38;5;n / 38;2;r;g;b — treat as fg/bg set
    applySgr(state, codes[0])
  }
  return state
}

/** Open codes for a state (attributes → bg → fg). */
function openCodes(state: Set<number>): string {
  let out = ""
  const fg: number[] = []
  const bg: number[] = []
  for (const c of state) {
    if (FG_CODES.has(c)) fg.push(c)
    else if (BG_CODES.has(c)) bg.push(c)
    else out += `\u001b[${c}m`
  }
  for (const c of bg) out += `\u001b[${c}m`
  for (const c of fg) out += `\u001b[${c}m`
  return out
}

/** Close codes for a state (attributes → bg → fg). */
function closeCodes(state: Set<number>): string {
  let out = ""
  for (const c of state) {
    if (ATTR_OFF[c] !== undefined) out += `\u001b[${ATTR_OFF[c]}m`
  }
  if ([...state].some((c) => BG_CODES.has(c))) out += "\u001b[49m"
  if ([...state].some((c) => FG_CODES.has(c))) out += "\u001b[39m"
  return out
}

/**
 * Keep ANSI color state consistent across wrapped lines: each line carries
 * the open styles from the previous line and closes them at its end.
 */
export function colorizeLines(lines: string[]): string[] {
  let state = new Set<number>()
  return lines.map((line) => {
    const prefixed = openCodes(state) + line
    state = readSgr(prefixed)
    return prefixed + closeCodes(state)
  })
}

// ── truncate ──────────────────────────────────────────────

function truncateWidthAnsi(str: string, maxWidth: number): { text: string; state: Set<number> } {
  let out = ""
  let width = 0
  const state = new Set<number>()
  const re = /\u001b\[([0-9;]*)m|([\s\S])/g
  for (const m of str.matchAll(re)) {
    if (m[1] !== undefined) {
      out += m[0]
      const raw = m[1]
      const code = raw === "" ? 0 : parseInt(raw.split(";")[0]!, 10)
      applySgr(state, code)
      continue
    }
    const ch = m[0]!
    const w = WIDE_RE.test(ch) ? 2 : 1
    if (width + w > maxWidth) break
    out += ch
    width += w
  }
  return { text: out, state }
}

/** Truncate to a visible width, appending "…" when content is cut. */
export function truncate(str: string, desiredLength: number, truncateChar = "…"): string {
  if (strlen(str) <= desiredLength) return str
  const { text, state } = truncateWidthAnsi(str, desiredLength - strlen(truncateChar))
  return text + closeCodes(state) + truncateChar
}

// ── word wrap ─────────────────────────────────────────────

function wrapOnWords(maxLength: number, input: string): string[] {
  const lines: string[] = []
  const split = input.split(/(\s+)/g)
  let line: string[] = []
  let lineLength = 0
  let whitespace: string | undefined
  for (let i = 0; i < split.length; i += 2) {
    const word = split[i]!
    let newLength = lineLength + strlen(word)
    if (lineLength > 0 && whitespace) newLength += whitespace.length
    if (newLength > maxLength) {
      if (lineLength !== 0) lines.push(line.join(""))
      line = [word]
      lineLength = strlen(word)
    } else {
      line.push(whitespace || "", word)
      lineLength = newLength
    }
    whitespace = split[i + 1]
  }
  if (lineLength) lines.push(line.join(""))
  return lines
}

/** Wrap on every character (ignoring word boundaries) — used when wrapOnWordBoundary is false. */
function wrapChars(maxLength: number, input: string): string[] {
  const lines: string[] = []
  let line = ""
  const push = (str: string, ws?: string) => {
    if (line.length && ws) line += ws
    line += str
    while (strlen(line) > maxLength) {
      lines.push(line.slice(0, line.length - (strlen(line) - maxLength)))
      line = line.slice(line.length - (strlen(line) - maxLength))
    }
  }
  const split = input.split(/(\s+)/g)
  for (let i = 0; i < split.length; i += 2) {
    push(split[i]!, i && split[i - 1] ? split[i - 1] : undefined)
  }
  if (line.length) lines.push(line)
  return lines
}

/** Split on "\n", then wrap each resulting line. ANSI-aware. */
function multiLineWrap(maxLength: number, input: string, onWordBoundary: boolean): string[] {
  const handler = onWordBoundary ? wrapOnWords : wrapChars
  const out: string[] = []
  for (const part of input.split("\n")) {
    out.push(...handler(maxLength, part))
  }
  return out
}

// ── table ─────────────────────────────────────────────────

export type CellValue = string | number | { content: string; colSpan?: number; rowSpan?: number }

export type TableOptions = {
  chars?: Record<string, string>
  style?: {
    border?: readonly string[]
    head?: readonly string[]
    "padding-left"?: number
    "padding-right"?: number
  }
  colWidths?: readonly number[]
  head?: readonly string[]
  colAligns?: readonly ("left" | "right" | "center")[]
  wordWrap?: boolean
  wrapOnWordBoundary?: boolean
  truncate?: string
}

const DEFAULT_CHARS = {
  top: "─",
  "top-mid": "┬",
  "top-left": "┌",
  "top-right": "┐",
  bottom: "─",
  "bottom-mid": "┴",
  "bottom-left": "└",
  "bottom-right": "┘",
  left: "│",
  "left-mid": "├",
  mid: "─",
  "mid-mid": "┼",
  right: "│",
  "right-mid": "┤",
  middle: "│",
}

type CellInfo = {
  content: string
  colSpan: number
  x: number
  y: number
  /** True for the placeholder cells added after a colSpan cell. */
  isSpanPlaceholder: boolean
}

export class CliTable3 {
  private readonly chars: Record<string, string>
  private readonly style: NonNullable<TableOptions["style"]>
  private readonly colWidths: readonly number[]
  private readonly head: readonly string[]
  private readonly colAligns: readonly ("left" | "right" | "center")[]
  private readonly wordWrap: boolean
  private readonly wrapOnWordBoundary: boolean
  private readonly truncateChar: string
  private readonly rows: CellValue[][] = []

  constructor(options: TableOptions = {}) {
    this.chars = { ...DEFAULT_CHARS, ...options.chars }
    this.style = options.style ?? {}
    this.colWidths = options.colWidths ?? []
    this.head = options.head ?? []
    this.colAligns = options.colAligns ?? []
    this.wordWrap = options.wordWrap ?? false
    this.wrapOnWordBoundary = options.wrapOnWordBoundary ?? true
    this.truncateChar = options.truncate ?? "…"
  }

  push(row: CellValue[]): void {
    this.rows.push(row)
  }

  toString(): string {
    const allRows: CellValue[][] = this.head.length
      ? [this.head as CellValue[], ...this.rows]
      : this.rows

    // Assign x (column) coordinates; colSpan cells consume extra columns.
    const cells: CellInfo[][] = allRows.map((row, y) => {
      let x = 0
      return row.map((value) => {
        const cell = typeof value === "object" && value !== null
          ? { content: String(value.content), colSpan: value.colSpan ?? 1 }
          : { content: String(value), colSpan: 1 }
        const info: CellInfo = { content: cell.content, colSpan: cell.colSpan, x, y, isSpanPlaceholder: false }
        x += cell.colSpan
        return info
      })
    })

    const colCount = Math.max(0, ...cells.flat().map((c) => c.x + c.colSpan))

    // Compute content width per column (fixed colWidths win, else desired width).
    const paddingLeft = this.style["padding-left"] ?? 1
    const paddingRight = this.style["padding-right"] ?? 1
    const widths: number[] = []
    for (let x = 0; x < colCount; x++) {
      const fixed = this.colWidths[x]
      if (typeof fixed === "number") {
        widths.push(fixed)
        continue
      }
      let desired = 1
      for (const row of cells) {
        for (const cell of row) {
          if (cell.x === x && cell.colSpan === 1) {
            desired = Math.max(desired, strlen(cell.content) + paddingLeft + paddingRight)
          }
        }
      }
      widths.push(desired)
    }

    // Pre-compute the wrapped lines of every cell.
    const wordWrap = this.wordWrap
    const onWordBoundary = this.wrapOnWordBoundary
    const cellLines: Map<CellInfo, string[]> = new Map()
    for (const row of cells) {
      for (const cell of row) {
        if (cell.isSpanPlaceholder) {
          cellLines.set(cell, [])
          continue
        }
        const spanWidth = cell.colSpan - 1
        let width = 0
        for (let i = 0; i < cell.colSpan; i++) {
          width += widths[cell.x + i]!
        }
        width += spanWidth
        const contentWidth = width - paddingLeft - paddingRight
        let lines: string[]
        if (wordWrap) {
          lines = multiLineWrap(Math.max(1, contentWidth), cell.content, onWordBoundary)
        } else {
          lines = cell.content.split("\n")
        }
        cellLines.set(cell, colorizeLines(lines))
      }
    }

    // Row height = max line count across the row's cells.
    const heights = cells.map((row) =>
      Math.max(1, ...row.map((c) => cellLines.get(c)!.length)),
    )

    if (cells.length === 0) return ""

    const out: string[] = []
    for (let y = 0; y < cells.length; y++) {
      out.push(this.drawBorder(cells, widths, y, "top"))
      for (let lineNum = 0; lineNum < heights[y]!; lineNum++) {
        out.push(this.drawLine(cells, widths, cellLines, y, lineNum))
      }
    }
    out.push(this.drawBorder(cells, widths, cells.length - 1, "bottom"))
    return out.join("\n")
  }

  private drawBorder(cells: CellInfo[][], widths: number[], rowIndex: number, kind: "top" | "bottom"): string {
    const bottom = kind === "bottom"
    const topRow = kind === "top" && rowIndex === 0
    const row = cells[rowIndex]!
    const lineChar = bottom ? "bottom" : topRow ? "top" : "mid"

    // Columns that start a real cell (vs. the interior of a colSpan cell).
    const starts = new Set<number>()
    for (const cell of row) {
      if (!cell.isSpanPlaceholder) starts.add(cell.x)
    }

    let out = ""
    for (let x = 0; x < widths.length; x++) {
      const isStart = starts.has(x)
      if (bottom) {
        out += x === 0 ? this.chars["bottom-left"] : isStart ? this.chars["bottom-mid"] : this.chars.bottom
      } else if (isStart) {
        if (x === 0) out += topRow ? this.chars["top-left"] : this.chars["left-mid"]
        else out += topRow ? this.chars["top-mid"] : this.chars["mid-mid"]
      } else {
        // Interior column of a colSpan cell: the border passes straight through
        // with ┴ so the spanning cell reads as a single band.
        out += topRow ? this.chars.top : this.chars["bottom-mid"]
      }
      out += repeat(this.chars[lineChar], widths[x]!)
    }
    if (bottom) out += this.chars["bottom-right"]
    else out += topRow ? this.chars["top-right"] : this.chars["right-mid"]

    const borderStyle = this.style.border
    if (borderStyle && borderStyle[0]) {
      return `${borderStyle[0]}${out}${borderStyle[1] ?? ""}`
    }
    return out
  }

  private drawLine(
    cells: CellInfo[][],
    widths: number[],
    cellLines: Map<CellInfo, string[]>,
    y: number,
    lineNum: number,
  ): string {
    const paddingLeft = this.style["padding-left"] ?? 1
    const paddingRight = this.style["padding-right"] ?? 1
    const isHead = y === 0 && this.head.length > 0
    const row = cells[y]!

    let out = ""
    for (let x = 0; x < widths.length; x++) {
      const cell = row.find((c) => c.x === x)
      if (!cell) continue // interior column of a colSpan cell

      let width = 0
      for (let i = 0; i < cell.colSpan; i++) width += widths[cell.x + i]!
      width += cell.colSpan - 1

      const lines = cellLines.get(cell)!
      const contentWidth = width - paddingLeft - paddingRight
      const line = lineNum < lines.length ? lines[lineNum]! : ""
      const content = pad(
        truncate(line, contentWidth, this.truncateChar),
        contentWidth,
        " ",
        this.colAligns[cell.x] ?? "left",
      )

      const borderStyle = this.style.border
      const colorize = (s: string) =>
        borderStyle && borderStyle[0] ? `${borderStyle[0]}${s}${borderStyle[1] ?? ""}` : s
      const left = colorize(this.chars[cell.x === 0 ? "left" : "middle"])
      const padding = " ".repeat(paddingLeft)
      const rightPadding = " ".repeat(paddingRight)
      const right = cell.x + cell.colSpan === widths.length
        ? colorize(this.chars.right)
        : ""

      let formatted = padding + content + rightPadding
      if (isHead && this.style.head && this.style.head[0]) {
        formatted = `${this.style.head[0]}${formatted}${this.style.head[1] ?? ""}`
      }
      out += left + formatted + right
    }
    return out
  }
}

export default CliTable3