/**
 * Pre-command hooks shared by interactive commands.
 */

/**
 * Run non-blocking pre-command hooks for interactive output:
 * Show the pending notification banner.
 * Skipped entirely for JSON output.
 */
export async function runPreCommandHooks(options: { json?: boolean } = {}): Promise<void> {
  if (options.json) return
  const { showNotificationBanner } = await import("./notifications/banner.ts")
  showNotificationBanner()
}
