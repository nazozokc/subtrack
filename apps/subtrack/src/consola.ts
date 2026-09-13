/**
 * Self-contained console logger, drop-in replacement for the subset of
 * `consola` used by subtrack. Zero runtime dependencies.
 *
 * Supported methods: log, info, success, warn, error, fail plus `mockTypes`
 * (used heavily by the test suite to capture output).
 */

export type LogType = "log" | "info" | "success" | "warn" | "error" | "fail"
export type LogFunction = (...args: unknown[]) => void

const TYPES: readonly LogType[] = ["log", "info", "success", "warn", "error", "fail"]

function emit(icon: string, color: string, args: unknown[]): void {
  if (args.length === 0) {
    console.log()
    return
  }
  console.log(`\x1b[${color}m${icon}\x1b[39m`, ...args)
}

/** Real output implementation per level. Log is plain; the rest carry an icon. */
export function makeDefault(type: LogType): LogFunction {
  switch (type) {
    case "log":
      return (...args) => console.log(...args)
    case "info":
      return (...args) => emit("ℹ", "36", args)
    case "success":
      return (...args) => emit("✔", "32", args)
    case "warn":
      return (...args) => emit("⚠", "33", args)
    case "error":
    case "fail":
      return (...args) => emit("✖", "31", args)
  }
}

/** Mutable logger instance — tests swap methods via `mockTypes` or direct assignment. */
export const consola: Record<LogType, LogFunction> & { mockTypes(factory?: (type: string, defaults: unknown) => LogFunction): void } = {
  log: makeDefault("log"),
  info: makeDefault("info"),
  success: makeDefault("success"),
  warn: makeDefault("warn"),
  error: makeDefault("error"),
  fail: makeDefault("fail"),
  mockTypes(factory?: (type: string, defaults: unknown) => LogFunction): void {
    for (const type of TYPES) {
      const defaults = makeDefault(type)
      consola[type] = factory ? factory(type, defaults) : defaults
    }
  },
}

export default consola