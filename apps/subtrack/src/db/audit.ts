import type { AddAuditArgs, AuditEntry } from "../types.ts"
/**
 * Audit log database operations.
 *
 * Tracks all mutating operations (add, edit, delete, restore, import, bulk)
 * for security diagnostics and change history.
 */

import type { SQLInputValue } from "node:sqlite"
import { getDb, execObjs, saveDb } from "./connection.ts"

export type { AddAuditArgs, AuditAction, AuditEntry } from "../types.ts"

/** Insert an audit log entry. */
export function addAuditLog(args: AddAuditArgs): void {
  const db = getDb()
  try {
    db.prepare(
      `INSERT INTO audit_log (action, target_type, target_id, details) VALUES (?, ?, ?, ?)`,
    ).run(args.action, args.targetType ?? null, args.targetId ?? null, args.details ?? null)
    saveDb()
  } catch {
    // Silently ignore if table doesn't exist (test environments, first-run edge cases)
  }
}

/** Query audit log entries with optional filters. */
export function getAuditLogs(options: {
  action?: string
  targetId?: number
  limit?: number
  offset?: number
  from?: string
  to?: string
} = {}): AuditEntry[] {
  const conditions: string[] = []
  const params: SQLInputValue[] = []

  if (options.action) {
    conditions.push("action = ?")
    params.push(options.action)
  }
  if (options.targetId !== undefined) {
    conditions.push("target_id = ?")
    params.push(options.targetId)
  }
  if (options.from) {
    conditions.push("created_at >= ?")
    params.push(options.from)
  }
  if (options.to) {
    conditions.push("created_at <= ?")
    params.push(options.to + "T23:59:59")
  }

  const where = conditions.length > 0 ? `WHERE ${conditions.join(" AND ")}` : ""
  const limit = options.limit ?? 50
  const offset = options.offset ?? 0

  return execObjs<AuditEntry>(
    getDb(),
    `SELECT * FROM audit_log ${where} ORDER BY id DESC LIMIT ? OFFSET ?`,
    [...params, limit, offset],
  )
}

/** Get the count of audit log entries (with optional filters). */
export function getAuditLogCount(options: { action?: string; from?: string; to?: string } = {}): number {
  const conditions: string[] = []
  const params: SQLInputValue[] = []

  if (options.action) {
    conditions.push("action = ?")
    params.push(options.action)
  }
  if (options.from) {
    conditions.push("created_at >= ?")
    params.push(options.from)
  }
  if (options.to) {
    conditions.push("created_at <= ?")
    params.push(options.to + "T23:59:59")
  }

  const where = conditions.length > 0 ? `WHERE ${conditions.join(" AND ")}` : ""
  const result = getDb().prepare(`SELECT COUNT(*) AS count FROM audit_log ${where}`).get(...params) as
    | { count: number }
    | undefined
  return result ? Number(result.count) : 0
}

/** Prune audit log entries older than a given date. */
export function pruneAuditLogs(before: string): number {
  const db = getDb()
  const { changes } = db.prepare("DELETE FROM audit_log WHERE created_at < ?").run(before)
  const count = Number(changes)
  if (count > 0) saveDb()
  return count
}
