/**
 * Shared types and lifecycle helpers for the self-contained prompt
 * implementations. Zero runtime dependencies — only Node built-ins.
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

/** Write to stdout, swallowing terminal write errors (EPIPE, closed stream). */
export function safeWrite(stdout: NodeJS.WriteStream, s: string): void {
  try {
    stdout.write(s)
  } catch {
    // EPIPE or closed stream — nothing sensible to do.
  }
}