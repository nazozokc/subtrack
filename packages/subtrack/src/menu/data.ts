/**
 * "Data" sub-menu (export / backup / restore / maintenance / audit / LLM usage).
 */

import { select, input, confirm } from "../prompts.ts"
import { BACK } from "./shared.ts"
import { handleExport } from "../export.ts"
import { handleBackup, handleRestore } from "../backup.ts"
import { handleMaintenance } from "../maintenance.ts"
import { handleAuditList, handleAuditPrune } from "../audit.ts"
import { handleUsageAdd } from "../usage-add.ts"
import { handleUsageDelete, handleUsageList } from "../usage.ts"
import { handleUsageImport } from "../usage-import.ts"
import { handleUsageRefresh } from "../usage-refresh.ts"
import { handleUsageTotal } from "../usage-total.ts"

export async function runDataMenu(): Promise<void> {
  while (true) {
    const action = await select({
      message: "data",
      pageSize: 10,
      choices: [
        { name: "Export", description: "Export subscriptions (csv/json/md/excel/ics)", value: "export" },
        { name: "Backup", description: "Back up the database", value: "backup" },
        { name: "Restore", description: "Restore database from a backup file", value: "restore" },
        { name: "Maintenance", description: "VACUUM and integrity check", value: "maintenance" },
        { name: "Audit", description: "Audit log list / prune", value: "audit" },
        { name: "LLM usage", description: "Track LLM API usage costs", value: "usage" },
        BACK,
      ],
    })

    switch (action) {
      case "export": {
        const format = await select({
          message: "select export format",
          choices: [
            { name: "csv", value: "csv" },
            { name: "json", value: "json" },
            { name: "md", value: "md" },
            { name: "excel", value: "excel" },
            { name: "ics", value: "ics" },
          ],
        })
        await handleExport(format, {})
        break
      }
      case "backup": await handleBackup(); break
      case "restore": {
        const file = await input({ message: "backup file path:", validate: (v) => v.trim().length > 0 || "Path required" })
        await handleRestore(file)
        break
      }
      case "maintenance": handleMaintenance(); break
      case "audit": await runAuditMenu(); break
      case "usage": await runUsageMenu(); break
      case "back": return
    }
  }
}

async function runAuditMenu(): Promise<void> {
  while (true) {
    const action = await select({
      message: "audit log",
      pageSize: 10,
      choices: [
        { name: "List", description: "View the audit log", value: "list" },
        { name: "Prune", description: "Delete entries older than 90 days", value: "prune" },
        BACK,
      ],
    })

    switch (action) {
      case "list": handleAuditList({}); break
      case "prune": {
        const ok = await confirm({ message: "Delete audit log entries older than 90 days?", default: false })
        if (ok) handleAuditPrune({ force: true })
        break
      }
      case "back": return
    }
  }
}

async function runUsageMenu(): Promise<void> {
  while (true) {
    const action = await select({
      message: "LLM usage",
      pageSize: 10,
      choices: [
        { name: "Add", description: "Record LLM API usage", value: "add" },
        { name: "List", description: "List usage entries", value: "list" },
        { name: "Delete", description: "Delete usage entries", value: "delete" },
        { name: "Import", description: "Import from JSONL/JSON logs", value: "import" },
        { name: "Refresh", description: "Auto-scan AI tool logs", value: "refresh" },
        { name: "Total", description: "Aggregated cost summary", value: "total" },
        BACK,
      ],
    })

    switch (action) {
      case "add": await handleUsageAdd({}); break
      case "list": await handleUsageList({}); break
      case "delete": await handleUsageDelete(); break
      case "import": {
        const file = await input({ message: "log file path:", validate: (v) => v.trim().length > 0 || "Path required" })
        await handleUsageImport({ file })
        break
      }
      case "refresh": await handleUsageRefresh(); break
      case "total": handleUsageTotal(); break
      case "back": return
    }
  }
}