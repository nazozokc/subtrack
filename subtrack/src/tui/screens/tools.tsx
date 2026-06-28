import { Box, Text } from "ink"
import { TextInput, Select } from "@inkjs/ui"
import { useState, useCallback, useMemo, useEffect } from "react"
import { useTui, useSetFormActive } from "../context/app-context.tsx"
import { TOOLS_TAB_LABELS, TOOLS_TABS } from "../types.ts"
import type { ToolsTab } from "../types.ts"
import {
  getSubscriptions,
  getLlmUsage,
  getDb,
  getDbDir,
  saveDb,
  getDefaultBackupDir,
  getBackupFiles,
  restoreDb,
} from "../../db.ts"
import type { SharedArgs } from "../../types.ts"
import { exportCsv, exportJson, exportMd } from "../../export.ts"
import { parseCsvLine } from "../../import-csv.ts"
import { isValidCurrency, isValidCycle } from "../../prompts.ts"
import { encryptBuffer, hasEncryptionKey } from "../../crypto.ts"
import { writeFileSync, readFileSync, copyFileSync, mkdirSync, existsSync } from "node:fs"
import { join } from "node:path"
import { cwd } from "node:process"

// ── Tab bar ───────────────────────────────────────────

function TabBar({ activeTab }: { activeTab: ToolsTab }) {
  const barWidth = TOOLS_TABS.length * 16

  return (
    <Box flexDirection="column" marginBottom={1}>
      <Box width={barWidth}>
        {TOOLS_TABS.map((tab, i) => {
          const isActive = tab === activeTab
          const label = `${TOOLS_TAB_LABELS[tab]}`
          const paddedLabel = ` ${label.padEnd(12)} `
          const separator = i < TOOLS_TABS.length - 1 ? (
            <Text dimColor>│</Text>
          ) : null

          return (
            <Box key={tab}>
              {isActive ? (
                <Text bold color="cyan" inverse>{paddedLabel}</Text>
              ) : (
                <Text dimColor>{paddedLabel}</Text>
              )}
              {separator}
            </Box>
          )
        })}
      </Box>
      <Text dimColor>
        {"─".repeat(barWidth)}
      </Text>
    </Box>
  )
}

// ── Export tab ────────────────────────────────────────

const EXPORTERS: Record<string, { ext: string; fn: (subs: SharedArgs[]) => string }> = {
  csv: { ext: "csv", fn: exportCsv },
  json: { ext: "json", fn: exportJson },
  md: { ext: "md", fn: exportMd },
}

function ExportTab() {
  const [format, setFormat] = useState<string | null>(null)
  const [result, setResult] = useState<string>("")
  const subs = useMemo(() => getSubscriptions(), [])
  const setFormActive = useSetFormActive()

  useEffect(() => {
    setFormActive(!format)
  }, [format, setFormActive])

  const doExport = useCallback((fmt: string) => {
    try {
      const entry = EXPORTERS[fmt]
      if (!entry) { setResult(`Unknown format: ${fmt}`); return }
      const output = entry.fn(subs)
      const filename = `subtrack-export.${entry.ext}`
      const filePath = join(cwd(), filename)
      writeFileSync(filePath, output, "utf-8")
      setResult(`Exported as ${fmt.toUpperCase()} to ${filePath}`)
    } catch (e: unknown) {
      setResult(`Error: ${e instanceof Error ? e.message : String(e)}`)
    }
  }, [subs])

  return (
    <Box flexDirection="column">
      <Box marginBottom={1}><Text bold underline>Export Subscriptions</Text></Box>
      {!format ? (
        <Select
          options={[
            { label: "CSV", value: "csv" },
            { label: "JSON", value: "json" },
            { label: "Markdown", value: "md" },
          ]}
          onChange={(v) => { setFormat(v); doExport(v) }}
        />
      ) : (
        <Text color={result.startsWith("Error") ? "red" : "green"}>{result}</Text>
      )}
    </Box>
  )
}

// ── Import tab ────────────────────────────────────────

function ImportTab() {
  const [filePath, setFilePath] = useState("")
  const [result, setResult] = useState<string | null>(null)
  const setFormActive = useSetFormActive()

  useEffect(() => {
    setFormActive(!result)
  }, [result, setFormActive])

  const doImport = useCallback(() => {
    if (!filePath.trim()) { setResult("Please enter a file path"); return }
    try {
      const content = readFileSync(filePath.trim(), "utf-8")
      const clean = content.charCodeAt(0) === 0xfeff ? content.slice(1) : content
      const lines = clean.split("\n").map((l) => l.trim()).filter(Boolean)

      let success = 0
      let failed = 0
      const errors: string[] = []
      const db = getDb()
      db.run("BEGIN TRANSACTION")
      try {
        for (let i = 1; i < lines.length; i++) {
          const fields = parseCsvLine(lines[i])
          if (fields.length < 5) { failed++; continue }
          if (!isValidCurrency(fields[4]) || !isValidCycle(fields[1])) { failed++; continue }
          const price = Number(fields[3])
          if (isNaN(price) || price < 0 || !Number.isInteger(price)) { failed++; errors.push(`Line ${i + 1}: invalid price "${fields[3]}"`); continue }
          // Direct insert to avoid nested transaction from writeSubscription
          db.run(
            "INSERT INTO subscriptions (name, price, currency, cycle, status, created_at) VALUES (?, ?, ?, ?, 'active', date('now'))",
            [fields[0].trim(), price, fields[4], fields[1]],
          )
          const idRow = db.exec("SELECT last_insert_rowid() AS id")
          if (idRow.length > 0 && idRow[0].values.length > 0) {
            const subId = Number(idRow[0].values[0][0])
            const tags = fields[2].split(";").map((t) => t.trim()).filter(Boolean)
            for (const t of tags) {
              db.run("INSERT OR IGNORE INTO tags (name) VALUES (?)", [t])
              const tagRow = db.exec("SELECT id FROM tags WHERE name = ?", [t])
              if (tagRow.length > 0 && tagRow[0].values.length > 0) {
                db.run("INSERT INTO subscription_tags (subscription_id, tag_id) VALUES (?, ?)", [subId, Number(tagRow[0].values[0][0])])
              }
            }
          }
          success++
        }
        db.run("COMMIT")
        saveDb()
      } catch (e) {
        db.run("ROLLBACK")
        throw e
      }
      const msg = `Imported ${success} subscription${success !== 1 ? "s" : ""} from ${filePath}${failed > 0 ? ` (${failed} failed)` : ""}`
      setResult(errors.length > 0 ? `${msg}\n${errors.join("\n")}` : msg)
    } catch (e: unknown) {
      setResult(`Error: ${e instanceof Error ? e.message : String(e)}`)
    }
  }, [filePath])

  return (
    <Box flexDirection="column">
      <Box marginBottom={1}><Text bold underline>Import from CSV</Text></Box>
      {result ? (
        <Text color={result.startsWith("Error") ? "red" : "green"}>{result}</Text>
      ) : (
        <TextInput
          placeholder="Path to CSV file..."
          defaultValue={filePath}
          onChange={setFilePath}
          onSubmit={doImport}
        />
      )}
    </Box>
  )
}

// ── Backup tab ────────────────────────────────────────

function BackupTab() {
  const [step, setStep] = useState<"path" | "encrypt" | "done">("path")
  const [dest, setDest] = useState(getDefaultBackupDir())
  const [result, setResult] = useState<string | null>(null)
  const setFormActive = useSetFormActive()

  useEffect(() => {
    setFormActive(step !== "done")
  }, [step, setFormActive])

  const doBackup = useCallback((encrypt: boolean) => {
    try {
      saveDb()
      mkdirSync(dest, { recursive: true })
      const ts = new Date().toISOString().replace(/[:.]/g, "-")
      const destPath = join(dest, `subtrack-${ts}.db`)
      const dbPath = join(getDbDir(), "subtrack.db")

      if (encrypt) {
        if (!hasEncryptionKey()) {
          setResult("Cannot encrypt: no encryption key configured. Run 'subtrack backup' in CLI to set up encryption.")
          setStep("done")
          return
        }
        const data = readFileSync(dbPath)
        const encrypted = encryptBuffer(data)
        writeFileSync(destPath, encrypted)
      } else {
        copyFileSync(dbPath, destPath)
      }
      setResult(`Backup saved to ${destPath}`)
    } catch (e: unknown) {
      setResult(`Backup failed: ${e instanceof Error ? e.message : String(e)}`)
    }
    setStep("done")
  }, [dest])

  return (
    <Box flexDirection="column">
      <Box marginBottom={1}><Text bold underline>Backup Database</Text></Box>
      {step === "path" && (
        <TextInput
          placeholder="Backup destination directory..."
          defaultValue={dest}
          onChange={setDest}
          onSubmit={() => setStep("encrypt")}
        />
      )}
      {step === "encrypt" && (
        <Box flexDirection="column" gap={1}>
          <Text dimColor>Encrypt backup?</Text>
          <Select
            options={[
              { label: "Yes, encrypt", value: "yes" },
              { label: "No, plain copy", value: "no" },
            ]}
            onChange={(v) => { doBackup(v === "yes") }}
          />
        </Box>
      )}
      {step === "done" && (
        <Text color={result?.startsWith("Backup") ? "green" : "red"}>{result}</Text>
      )}
    </Box>
  )
}

// ── Restore tab ──────────────────────────────────────

function RestoreTab() {
  const [step, setStep] = useState<"choose" | "confirm" | "done">("choose")
  const [source, setSource] = useState("")
  const [result, setResult] = useState<string | null>(null)
  const setFormActive = useSetFormActive()

  useEffect(() => {
    setFormActive(step !== "done")
  }, [step, setFormActive])

  const backups = getBackupFiles(getDbDir())
  const options = backups.map((b) => ({
    label: `${b.name} (${b.mtime.toISOString().slice(0, 10)})`,
    value: b.path,
  }))

  const doRestore = useCallback(() => {
    if (!source || !existsSync(source)) {
      setResult("File not found")
      setStep("done")
      return
    }
    try {
      restoreDb(source)
      setResult(`Restored from ${source}`)
    } catch (e: unknown) {
      setResult(`Restore failed: ${e instanceof Error ? e.message : String(e)}`)
    }
    setStep("done")
  }, [source])

  return (
    <Box flexDirection="column">
      <Box marginBottom={1}><Text bold underline>Restore Database</Text></Box>
      {step === "choose" && (
        <Box flexDirection="column" gap={1}>
          <Text dimColor>Select a backup file:</Text>
          {options.length > 0 ? (
            <Select
              options={options}
              onChange={(v) => { setSource(v); setStep("confirm") }}
            />
          ) : (
            <Text dimColor>No backups found</Text>
          )}
          <TextInput
            placeholder="Or enter path manually..."
            defaultValue={source}
            onChange={setSource}
            onSubmit={() => setStep("confirm")}
          />
        </Box>
      )}
      {step === "confirm" && (
        <Box flexDirection="column" gap={1}>
          <Text>
            Restore from <Text bold>{source}</Text>?
          </Text>
          <Text dimColor>This will overwrite the current database.</Text>
          <Select
            options={[
              { label: "Yes, restore", value: "yes" },
              { label: "Cancel", value: "no" },
            ]}
            onChange={(v) => {
              if (v === "yes") doRestore()
              else setStep("choose")
            }}
          />
        </Box>
      )}
      {step === "done" && (
        <Text color={result?.startsWith("Restored") ? "green" : "red"}>
          {result}
        </Text>
      )}
    </Box>
  )
}

// ── Usage tab ─────────────────────────────────────────

function UsageTab() {
  const { state } = useTui()
  const entries = useMemo(() => getLlmUsage({ limit: 50 }), [state.refreshKey])
  const total = entries.reduce((s, e) => s + e.cost, 0)

  return (
    <Box flexDirection="column">
      <Box marginBottom={1}>
        <Text bold underline>LLM API Usage</Text>
        <Text dimColor>  ({entries.length} entries)</Text>
      </Box>
      {entries.length === 0 ? (
        <Text dimColor>No usage entries</Text>
      ) : (
        <>
          {entries.map((e) => (
            <Box key={e.id}>
              <Box width={14}>
                <Text bold wrap="truncate-end">{e.provider}</Text>
              </Box>
              <Box width={20}>
                <Text dimColor wrap="truncate-end">{e.model}</Text>
              </Box>
              <Box width={12}>
                <Text>{e.date}</Text>
              </Box>
              <Box width={10}>
                <Text>${(e.cost / 100).toFixed(4)}</Text>
              </Box>
              {e.description && (
                <Text dimColor wrap="truncate-end">{e.description}</Text>
              )}
            </Box>
          ))}
          <Box marginTop={1}>
            <Text bold color="yellow">
              Total: ${(total / 100).toFixed(2)}
            </Text>
          </Box>
        </>
      )}
    </Box>
  )
}

// ── Main component ────────────────────────────────────

export function ToolsScreen() {
  const { state } = useTui()

  return (
    <Box flexDirection="column" flexGrow={1} paddingX={1} paddingY={1}>
      <Box marginBottom={1} flexDirection="column">
        <Box>
          <Text bold inverse color="cyan">
            {" Tools "}
          </Text>
        </Box>
      </Box>

      <TabBar activeTab={state.toolsTab} />

      {state.toolsTab === "export" && <ExportTab />}
      {state.toolsTab === "import" && <ImportTab />}
      {state.toolsTab === "backup" && <BackupTab />}
      {state.toolsTab === "restore" && <RestoreTab />}
      {state.toolsTab === "usage" && <UsageTab />}
    </Box>
  )
}
