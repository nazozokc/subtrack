import { Box, Text, useInput } from "ink"
import { Select } from "@inkjs/ui"
import { useState, useCallback, useMemo } from "react"
import { useTui } from "../context/app-context.tsx"
import { getSubscriptions } from "../../db.ts"
import { exportCsv, exportJson, exportMd } from "../../export.ts"
import { writeFileSync } from "node:fs"
import { join } from "node:path"
import { cwd } from "node:process"

const EXPORTERS: Record<string, { ext: string; fn: (subs: any[]) => string }> = {
  csv: { ext: "csv", fn: exportCsv },
  json: { ext: "json", fn: exportJson },
  md: { ext: "md", fn: exportMd },
}

export function ExportScreen() {
  const { dispatch } = useTui()
  const [format, setFormat] = useState<string | null>(null)
  const [result, setResult] = useState<string>("")
  const subs = useMemo(() => getSubscriptions(), [])

  useInput((input, key) => {
    if (key.escape) dispatch({ type: "SET_SCREEN", screen: "list" })
  })

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
    <Box flexDirection="column" flexGrow={1} paddingX={1}>
      <Box marginBottom={1}>
        <Text bold underline>Export Subscriptions</Text>
      </Box>
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
        <Text>{result}</Text>
      )}
    </Box>
  )
}
