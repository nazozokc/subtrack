import { Box, Text, useInput } from "ink"
import { Select } from "@inkjs/ui"
import { useState, useCallback, useMemo } from "react"
import { useTui } from "../context/app-context.tsx"
import { getSubscriptions } from "../../db.ts"
import { exportCsv, exportJson, exportMd } from "../../export.ts"

const EXPORTERS: Record<string, (subs: any[]) => string> = {
  csv: exportCsv,
  json: exportJson,
  md: exportMd,
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
      const fn = EXPORTERS[fmt]
      if (!fn) { setResult(`Unknown format: ${fmt}`); return }
      const output = fn(subs)
      setResult(`Exported as ${fmt.toUpperCase()}:\n\n${output.slice(0, 1000)}${output.length > 1000 ? "..." : ""}`)
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
