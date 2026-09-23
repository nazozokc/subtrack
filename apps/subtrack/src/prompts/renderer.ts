/**
 * Prompt screen rendering: in-place redraw of the prompt region and the
 * final answered line shown once a prompt resolves.
 */

import { green } from "./ansi.ts"
import { safeWrite } from "./core.ts"

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

/** Print the final answered line once a prompt resolves (✔ marks completion). */
export function finishPrompt(
  stdout: NodeJS.WriteStream,
  message: string,
  result: string,
): void {
  safeWrite(stdout, `\r${green("✔")} ${message} ${green(result)}\n`)
}