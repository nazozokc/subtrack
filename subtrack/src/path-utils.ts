import { resolve, normalize } from "node:path"

/**
 * Check that a user-provided path resolves safely within an expected base directory.
 * Prevents path traversal attacks using `..` or symlinks.
 *
 * @param basePath - The trusted base directory (must be absolute, already resolved).
 * @param userPath - The user-provided path to validate.
 * @returns The resolved absolute path if safe, or null if the path attempts escape.
 */
export function resolveSafePath(basePath: string, userPath: string): string | null {
  const resolved = resolve(userPath)
  const normalized = normalize(resolved)
  const base = normalize(basePath)

  // Must be within the base directory
  if (!normalized.startsWith(base.endsWith("/") ? base : `${base}/`)) {
    return null
  }

  return normalized
}
