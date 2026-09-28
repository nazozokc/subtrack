const profileEnabled = process.env.SUBTRACK_STARTUP_PROFILE === "1"
const startedAt = globalThis.performance.now()
const marks = new Map<string, number>()

/** Record a named startup checkpoint when startup profiling is enabled. */
export function markStartup(name: string): void {
  if (!profileEnabled) return
  marks.set(name, globalThis.performance.now())
}

/** Write the elapsed time since the process entry point to stderr. */
export function reportStartup(name: string): void {
  if (!profileEnabled) return
  const now = globalThis.performance.now()
  const previous = marks.get(name)
  const sinceStart = now - startedAt
  const sincePrevious = previous === undefined ? undefined : now - previous
  const suffix = sincePrevious === undefined ? "" : ` (+${sincePrevious.toFixed(2)} ms)`
  process.stderr.write(
    `[startup] ${name}: ${sinceStart.toFixed(2)} ms${suffix}\n`,
  )
}
