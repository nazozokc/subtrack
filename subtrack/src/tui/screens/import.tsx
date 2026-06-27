import { Box, Text, useInput } from "ink"
import { TextInput } from "@inkjs/ui"
import { useState, useCallback } from "react"
import { readFileSync } from "node:fs"
import { useTui } from "../context/app-context.tsx"
import { writeSubscription, getDb } from "../../db.ts"
import { parseCsvLine } from "../../import-csv.ts"
import { isValidCurrency, isValidCycle } from "../../prompts.ts"

export function ImportScreen() {
  const { dispatch } = useTui()
  const [filePath, setFilePath] = useState("")
  const [result, setResult] = useState<string | null>(null)

  useInput((input, key) => {
    if (key.escape) dispatch({ type: "SET_SCREEN", screen: "list" })
  })

  const doImport = useCallback(() => {
    if (!filePath.trim()) { setResult("Please enter a file path"); return }
    try {
      const content = readFileSync(filePath.trim(), "utf-8")
      const clean = content.charCodeAt(0) === 0xfeff ? content.slice(1) : content
      const lines = clean.split("\n").map((l) => l.trim()).filter(Boolean)

      let success = 0
      let failed = 0
      const errors: string[] = []
      getDb().exec("BEGIN TRANSACTION")
      try {
        for (let i = 1; i < lines.length; i++) {
          const fields = parseCsvLine(lines[i])
          if (fields.length < 5) { failed++; continue }
          if (!isValidCurrency(fields[4]) || !isValidCycle(fields[1])) { failed++; continue }
          const price = Number(fields[3])
          if (isNaN(price) || price < 0 || !Number.isInteger(price)) { failed++; errors.push(`Line ${i + 1}: invalid price "${fields[3]}"`); continue }
          writeSubscription({
            name: fields[0].trim(),
            cycle: fields[1],
            tags: fields[2].split(";").map((t) => t.trim()).filter(Boolean),
            price,
            currency: fields[4],
          })
          success++
        }
        getDb().exec("COMMIT")
      } catch (e) {
        getDb().exec("ROLLBACK")
        throw e
      }
      const msg = `Imported ${success} subscription${success !== 1 ? "s" : ""} from ${filePath}${failed > 0 ? ` (${failed} failed)` : ""}`
      setResult(errors.length > 0 ? `${msg}\n${errors.join("\n")}` : msg)
    } catch (e: unknown) {
      setResult(`Error: ${e instanceof Error ? e.message : String(e)}`)
    }
  }, [filePath])

  return (
    <Box flexDirection="column" flexGrow={1} paddingX={1}>
      <Box marginBottom={1}>
        <Text bold underline>Import from CSV</Text>
      </Box>
      {result ? (
        <Text color="green">{result}</Text>
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
