/**
 * Pre-command hooks shared by interactive commands.
 */

/**
 * Run non-blocking pre-command hooks for interactive output:
 * Show the pending notification banner.
 * Skipped entirely for JSON output and when stdout is piped/redirected —
 * the banner would pollute the output stream, and its extra DB queries
 * (suggestion count + full subscription scan) would run for nothing.
 */
export async function runPreCommandHooks(options: { json?: boolean } = {}): Promise<void> {
  if (options.json) return
  if (!process.stdout.isTTY) return
  const { showNotificationBanner } = await import("./notifications/banner.ts")
  showNotificationBanner()
}
