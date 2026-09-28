/**
 * Audit logging helper (lightweight).
 *
 * Kept separate from `audit.ts` (audit log display) so write-only callers
 * (config, backup, usage, ...) don't drag the table renderer into their
 * dependency graph.
 */

import { auditRepository } from "./application/index.ts"
import type { AuditAction, AddAuditArgs } from "./types.ts"

/**
 * Convenience function to log an audit entry from command handlers.
 */
export function logAudit(action: AuditAction, args: Omit<AddAuditArgs, "action"> = {}): void {
  auditRepository.record({ action, ...args })
}