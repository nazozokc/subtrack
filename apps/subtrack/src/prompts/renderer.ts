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
    // Raw mode disables output post-processing (\n stays line-feed-only, no
    // carriage return), so every line must reset the column itself. A leading
    // \r on the first line too is harmless.
    const body = text
      .split("\n")
      .map((line) => `\r${line}`)
      .join("\n")
    safeWrite(this.stdout, body)
    this.count = text.split("\n").length
  }

  clear(): void {
    if (this.count === 0) return
    // After write() the cursor rests on the last rendered line, so returning
    // to the block start needs count - 1 rows up (count would overshoot by
    // one and walk the whole menu up the screen on every re-render).
    safeWrite(this.stdout, `\r\x1b[${this.count - 1}A\x1b[J`)
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