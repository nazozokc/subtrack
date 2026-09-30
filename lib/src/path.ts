import { resolve, normalize, isAbsolute, dirname, sep, parse } from "node:path"
import { realpathSync, existsSync } from "node:fs"
import os from "node:os"

/** Default trusted base directories: home and temp. */
function defaultBases(): string[] {
  return [os.homedir(), os.tmpdir()]
}

/**
 * Directories subtrack refuses to treat as its data directory.
 *
 * The bare names are rejected outright. `/etc`, `/proc`, `/sys` and `/dev` are
 * also rejected with descendants, because a subdirectory of those is still
 * system-owned state and nothing legitimate writes there.
 *
 * `/tmp` is deliberately *not* covered by the descendant rule: it is sticky, so
 * a `mkdir`-created `/tmp/subtrack` can only be removed by its owner, and
 * `SUBSC_CLI_DB_DIR=/tmp/subtrack` is a reasonable thing to ask for.
 */
const FORBIDDEN_DIRS = ["/etc", "/dev", "/proc", "/sys", "/tmp"] as const
const FORBIDDEN_DIR_TREES = ["/etc", "/dev", "/proc", "/sys"] as const

/**
 * Validate a directory that subtrack is about to own (database, config, keys).
 *
 * `label` names the setting in the error message so the user knows which knob
 * to change. Returns the normalized path so callers can use it directly.
 *
 * @throws if the path is empty, too long, the filesystem root, a system
 *         directory, or inside a system directory tree.
 */
export function validateAppDir(dir: string, label: string): string {
  if (!dir || typeof dir !== "string") {
    throw new Error(`${label} must be a non-empty string`)
  }
  if (dir.length > 4096) {
    throw new Error(`${label} path too long`)
  }
  const normalized = normalize(resolve(dir))
  // `path.resolve("/")` is a drive root like "D:\" on Windows, so compare
  // against the parsed root rather than a literal "/".
  if (normalized === parse(normalized).root) {
    throw new Error(`${label} cannot be the filesystem root: ${normalized} (a system directory)`)
  }
  if (FORBIDDEN_DIRS.includes(normalized as (typeof FORBIDDEN_DIRS)[number])) {
    throw new Error(`${label} cannot be a system directory: ${normalized}`)
  }
  for (const tree of FORBIDDEN_DIR_TREES) {
    if (normalized === tree || normalized.startsWith(`${tree}${sep}`)) {
      throw new Error(`${label} cannot be inside a system directory: ${normalized}`)
    }
  }
  return normalized
}

/**
 * resolveSafePath with the default home/tmp base directories.
 */
export function safePath(userPath: string): string | null {
  return resolveSafePath(defaultBases(), userPath)
}

/**
 * resolveSafeOutputPath with the default home/tmp base directories.
 */
export function safeOutputPath(targetPath: string): string | null {
  return resolveSafeOutputPath(defaultBases(), targetPath)
}

/** Check if `child` path is within `parent` directory (or equals it), using platform separator. */
function isWithin(child: string, parent: string): boolean {
  if (child === parent) return true
  const prefix = parent.endsWith(sep) ? parent : `${parent}${sep}`
  return child.startsWith(prefix)
}

/**
 * Resolve a base directory through symlinks (if it exists), falling back
 * to normalized form. This ensures macOS /tmp → /private/tmp mapping is
 * handled correctly.
 */
function resolveBase(basePath: string): string {
  try {
    return realpathSync(basePath)
  } catch {
    return normalize(basePath)
  }
}

/**
 * Check that a user-provided path resolves safely within one of the allowed base directories.
 * Prevents path traversal attacks using `..` or symlinks.
 *
 * @param basePaths - Trusted base directories (must be absolute).
 * @param userPath  - The user-provided path to validate. Must exist on the filesystem.
 * @returns The resolved absolute path if safe, or null if the path attempts escape
 *          or does not exist.
 */
export function resolveSafePath(basePaths: string[], userPath: string): string | null {
  for (const basePath of basePaths) {
    if (!isAbsolute(basePath)) continue

    let resolved: string
    try {
      resolved = realpathSync(userPath)
    } catch {
      continue
    }

    const base = resolveBase(basePath)

    // Must be within the base directory
    if (!isWithin(resolved, base)) {
      continue
    }

    return resolved
  }

  return null
}

/**
 * Validate that a file path (which may not exist yet) resolves within one of the
 * allowed base directories. For non-existent paths, traverses up to the nearest
 * existing parent to check symlink safety.
 *
 * @param basePaths - Trusted base directories (must be absolute).
 * @param targetPath - The user-provided path to validate (file or directory).
 * @returns The resolved absolute path if safe, or null if the path escapes all bases.
 */
export function resolveSafeOutputPath(basePaths: string[], targetPath: string): string | null {
  const absolute = resolve(targetPath)

  for (const basePath of basePaths) {
    if (!isAbsolute(basePath)) continue

    const base = resolveBase(basePath)

    // Walk up from the target to find an existing parent for realpath check
    let checkPath = absolute
    while (checkPath && !existsSync(checkPath)) {
      const parent = dirname(checkPath)
      if (parent === checkPath) break // reached filesystem root
      checkPath = parent
    }

    if (!existsSync(checkPath)) {
      // Nothing in the path exists — check the normalized path syntactically
      if (isWithin(absolute, base) || absolute === base) {
        return absolute
      }
      continue
    }

    // Resolve symlinks from the existing portion
    try {
      const resolved = realpathSync(checkPath)
      if (!isWithin(resolved, base)) {
        continue
      }
      // Reconstruct the full path from the resolved base + remaining components
      const remaining = absolute.slice(checkPath.length)
      return remaining ? resolved + remaining : resolved
    } catch {
      continue
    }
  }

  return null
}
