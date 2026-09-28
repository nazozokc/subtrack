// ── Backup/Restore commands ────────────────────────────
import { define } from "../cli/types.ts"

export const backupCommand = define({
  name: "backup",
  description: "Backup database (gzip compressed)",
  args: {
    destination: { type: "positional", description: "Backup destination directory (default: ~/.config/subtrack/backups/)", required: false },
    encrypt: { type: "boolean", short: "e", description: "Encrypt the backup with your database key" },
  },
  run: async (ctx) => {
    const { handleBackup } = await import("../backup.ts")
    return handleBackup(ctx.values.destination, { encrypt: ctx.values.encrypt })
  },
})

export const restoreCommand = define({
  name: "restore",
  description: "Restore database from a backup",
  args: {
    file: { type: "positional", description: "Backup file to restore (omit for interactive selection)", required: false },
    force: { type: "boolean", short: "f", description: "Skip confirmation" },
    dir: { type: "string", description: "Directory to scan for backup files" },
  },
  run: async (ctx) => {
    const { handleRestore } = await import("../backup.ts")
    return handleRestore(ctx.values.file, { force: ctx.values.force, dir: ctx.values.dir })
  },
})