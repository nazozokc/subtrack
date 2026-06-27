import { Box, Text, useInput } from "ink"
import { TextInput, Select } from "@inkjs/ui"
import { useState, useCallback } from "react"
import { useTui } from "../context/app-context.tsx"
import { restoreDb, getBackupFiles, getDbDir } from "../../db.ts"
import { decryptBuffer, isEncrypted, hasEncryptionKey } from "../../crypto.ts"
import { readFileSync, writeFileSync, existsSync } from "node:fs"
import { join } from "node:path"

export function RestoreScreen() {
  const { dispatch } = useTui()
  const [step, setStep] = useState<"choose" | "confirm" | "done">("choose")
  const [source, setSource] = useState("")
  const [result, setResult] = useState<string | null>(null)

  const backups = getBackupFiles(getDbDir())
  const options = backups.map((b) => ({ label: `${b.name} (${b.date})`, value: b.path }))

  useInput((input, key) => {
    if (key.escape) dispatch({ type: "SET_SCREEN", screen: "list" })
  })

  const doRestore = useCallback(() => {
    if (!source || !existsSync(source)) {
      setResult("File not found"); setStep("done"); return
    }
    try {
      if (isEncrypted(readFileSync(source))) {
        if (!hasEncryptionKey()) { setResult("Cannot decrypt: no encryption key configured"); setStep("done"); return }
        const data = readFileSync(source)
        const decrypted = decryptBuffer(data)
        const dest = join(getDbDir(), "subtrack.db")
        writeFileSync(dest, decrypted)
        setResult(`Restored from ${source} (decrypted)`)
      } else {
        restoreDb(source)
        setResult(`Restored from ${source}`)
      }
    } catch (e: unknown) {
      setResult(`Restore failed: ${e instanceof Error ? e.message : String(e)}`)
    }
    setStep("done")
  }, [source])

  return (
    <Box flexDirection="column" flexGrow={1} paddingX={1}>
      <Box marginBottom={1}>
        <Text bold underline>Restore Database</Text>
      </Box>
      {step === "choose" && (
        <Box flexDirection="column" gap={1}>
          <Text dimColor>Select a backup file:</Text>
          <Select
            options={options.length > 0 ? options : [{ label: "No backups found", value: "" }]}
            onChange={(v) => { setSource(v); setStep("confirm") }}
          />
          <TextInput placeholder="Or enter path manually..." defaultValue={source} onChange={setSource} onSubmit={() => setStep("confirm")} />
        </Box>
      )}
      {step === "confirm" && (
        <Box flexDirection="column" gap={1}>
          <Text>Restore from <Text bold>{source}</Text>?</Text>
          <Text dimColor>This will overwrite the current database.</Text>
          <Select options={[{label:"Yes, restore",value:"yes"},{label:"Cancel",value:"no"}]} onChange={(v) => { if (v === "yes") doRestore(); else dispatch({ type: "SET_SCREEN", screen: "list" }) }} />
        </Box>
      )}
      {step === "done" && (
        <Text color={result?.startsWith("Restored") ? "green" : "red"}>{result}</Text>
      )}
    </Box>
  )
}
