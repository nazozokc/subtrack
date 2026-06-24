import { isAbsolute, normalize } from "node:path"
import { realpathSync } from "node:fs"

/**
 * Check that a user-provided path resolves safely within an expected base directory.
 * Prevents path traversal attacks using `..` or symlinks.
 *
 * @param basePath - The trusted base directory (must be absolute, already resolved).
 * @param userPath - The user-provided path to validate. Must exist on the filesystem.
 * @returns The resolved absolute path if safe, or null if the path attempts escape
 *          or does not exist.
 */
export function resolveSafePath(basePath: string, userPath: string): string | null {
  if (!isAbsolute(basePath)) {
    return null
  }

  let resolved: string
  try {
    // realpathSync resolves all symlinks so a symlink pointing outside basePath
    // cannot pass the prefix check
    resolved = realpathSync(userPath)
  } catch {
    return null
  }

  const base = normalize(basePath)

  // Must be within the base directory
  if (!resolved.startsWith(base.endsWith("/") ? base : `${base}/`)) {
    return null
  }

  return resolved
}
