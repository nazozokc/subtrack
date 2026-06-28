import { Box, Text } from "ink"
import Spinner from "ink-spinner"
import { Select } from "@inkjs/ui"
import { useState, useCallback, useMemo, useEffect } from "react"
import { useSetFormActive } from "../../context/app-context.tsx"
import { getSubscriptions } from "../../../db.ts"
import { exportCsv, exportJson, exportMd } from "../../../export.ts"
import { writeFileSync } from "node:fs"
import { join } from "node:path"
import { cwd } from "node:process"

const EXPORTERS: Record<string, { ext: string; fn: (subs: any[]) => string }> = {
  csv: { ext: "csv", fn: exportCsv },
  json: { ext: "json", fn: exportJson },
  md: { ext: "md", fn: exportMd },
}

export function ExportTab() {
  const [format, setFormat] = useState<string | null>(null)
  const [processing, setProcessing] = useState(false)
  const [result, setResult] = useState<string>("")
  const subs = useMemo(() => getSubscriptions(), [])
  const setFormActive = useSetFormActive()

  useEffect(() => {
    setFormActive(!format)
  }, [format, setFormActive])

  const doExport = useCallback((fmt: string) => {
    setProcessing(true)
    queueMicrotask(() => {
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
      setProcessing(false)
    })
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
      ) : processing ? (
        <Text>
          <Spinner type="dots" /> Exporting…
        </Text>
      ) : (
        <Text color={result.startsWith("Error") ? "red" : "green"}>{result}</Text>
      )}
    </Box>
  )
}
