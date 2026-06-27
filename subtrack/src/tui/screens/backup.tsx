import { Box, Text, useInput } from "ink"
import { TextInput, Select } from "@inkjs/ui"
import { useState, useCallback } from "react"
import { useTui } from "../context/app-context.tsx"
import { getDefaultBackupDir, saveDb, getDbDir } from "../../db.ts"
import { encryptBuffer, hasEncryptionKey } from "../../crypto.ts"
import { mkdirSync, writeFileSync, copyFileSync, readFileSync } from "node:fs"
import { join } from "node:path"

export function BackupScreen() {
  const { dispatch } = useTui()
  const [step, setStep] = useState<"path" | "encrypt" | "done">("path")
  const [dest, setDest] = useState(getDefaultBackupDir())
  const [encrypt, setEncrypt] = useState(false)
  const [result, setResult] = useState<string | null>(null)

  useInput((input, key) => {
    if (key.escape) dispatch({ type: "SET_SCREEN", screen: "list" })
  })

  const doBackup = useCallback(() => {
    try {
      saveDb()
      mkdirSync(dest, { recursive: true })
      const ts = new Date().toISOString().replace(/[:.]/g, "-")
      const destPath = join(dest, `subtrack-${ts}.db`)

      if (encrypt && hasEncryptionKey()) {
        const dbPath = join(getDbDir(), "subtrack.db")
        const data = readFileSync(dbPath)
        const encrypted = encryptBuffer(data)
        writeFileSync(destPath, encrypted)
      } else {
        const dbPath = join(getDbDir(), "subtrack.db")
        copyFileSync(dbPath, destPath)
      }
      setResult(`Backup saved to ${destPath}`)
    } catch (e: unknown) {
      setResult(`Backup failed: ${e instanceof Error ? e.message : String(e)}`)
    }
    setStep("done")
  }, [dest, encrypt])

  return (
    <Box flexDirection="column" flexGrow={1} paddingX={1}>
      <Box marginBottom={1}>
        <Text bold underline>Backup Database</Text>
      </Box>
      {step === "path" && (
        <TextInput placeholder="Backup destination directory..." defaultValue={dest} onChange={setDest} onSubmit={() => setStep("encrypt")} />
      )}
      {step === "encrypt" && (
        <Box flexDirection="column" gap={1}>
          <Text>Encrypt backup?</Text>
          <Select options={[{label:"Yes, encrypt",value:"yes"},{label:"No, plain copy",value:"no"}]} onChange={(v) => { setEncrypt(v === "yes"); doBackup() }} />
        </Box>
      )}
      {step === "done" && (
        <Text color={result?.startsWith("Backup") ? "green" : "red"}>{result}</Text>
      )}
    </Box>
  )
}
