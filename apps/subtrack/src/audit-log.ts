/**
 * Audit logging helper (lightweight).
 *
 * Kept separate from `audit.ts` (audit log display) so write-only callers
 * (config, backup, usage, ...) don't drag the table renderer into their
 * dependency graph.
 */

import { addAuditLog } from "./db/audit.ts"
import type { AuditAction, AddAuditArgs } from "./db/audit.ts"

/**
 * Convenience function to log an audit entry from command handlers.
 */
export function logAudit(action: AuditAction, args: Omit<AddAuditArgs, "action"> = {}): void {
  addAuditLog({ action, ...args })
}