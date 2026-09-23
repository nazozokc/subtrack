/**
 * ANSI display helpers for the self-contained prompts.
 * Pure string functions — no I/O. CJK/wide characters count as 2 columns.
 */

export const cyan = (s: string) => `\x1b[36m${s}\x1b[39m`
export const green = (s: string) => `\x1b[32m${s}\x1b[39m`
export const bold = (s: string) => `\x1b[1m${s}\x1b[22m`
export const dim = (s: string) => `\x1b[2m${s}\x1b[22m`
export const red = (s: string) => `\x1b[31m${s}\x1b[39m`

export const POINTER = "❯"
export const CHECKED = "◉"
export const UNCHECKED = "◯"

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