import { resolve, normalize, isAbsolute, dirname } from "node:path"
import { realpathSync, existsSync } from "node:fs"

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
      // realpathSync resolves all symlinks so a symlink pointing outside basePath
      // cannot pass the prefix check
      resolved = realpathSync(userPath)
    } catch {
      continue
    }

    const base = normalize(basePath)

    // Must be within the base directory
    if (!resolved.startsWith(base.endsWith("/") ? base : `${base}/`)) {
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

    const base = normalize(basePath)

    // Walk up from the target to find an existing parent for realpath check
    let checkPath = absolute
    while (checkPath && !existsSync(checkPath)) {
      const parent = dirname(checkPath)
      if (parent === checkPath) break // reached filesystem root
      checkPath = parent
    }

    if (!existsSync(checkPath)) {
      // Nothing in the path exists — check the normalized path syntactically
      if (absolute.startsWith(base.endsWith("/") ? base : `${base}/`) || absolute === base) {
        return absolute
      }
      continue
    }

    // Resolve symlinks from the existing portion
    try {
      const resolved = realpathSync(checkPath)
      if (!resolved.startsWith(base.endsWith("/") ? base : `${base}/`)) {
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
