import { Box, Text, useInput } from "ink"
import { TextInput } from "@inkjs/ui"
import { useState, useMemo, useEffect } from "react"
import { useTui, useSetFormActive } from "../context/app-context.tsx"
import { loadConfig, setConfig, CONFIG_KEYS, resetConfig } from "../../config.ts"
import type { ConfigKey } from "../../config.ts"

export function ConfigScreen() {
  const { dispatch } = useTui()
  const [editKey, setEditKey] = useState<string | null>(null)
  const [editValue, setEditValue] = useState("")
  const [result, setResult] = useState<string | null>(null)
  const [rev, setRev] = useState(0)
  const config = useMemo(() => loadConfig(), [rev])
  const setFormActive = useSetFormActive()

  useEffect(() => {
    setFormActive(editKey !== null)
    return () => setFormActive(false)
  }, [editKey, setFormActive])

  useInput((input, key) => {
    if (key.escape) {
      if (editKey) {
        setEditKey(null)
        setResult(null)
      }
    }
    if (!editKey && /^\d$/.test(input)) {
      const idx = Number(input) - 1
      if (idx >= 0 && idx < CONFIG_KEYS.length) {
        setEditKey(CONFIG_KEYS[idx])
        setEditValue(String(config[CONFIG_KEYS[idx] as ConfigKey]))
      }
    }
  })

  const save = (k: string, v: string) => {
    const ok = setConfig(k as ConfigKey, v)
    if (ok) {
      resetConfig()
      setRev((n) => n + 1)
      setResult(`Saved ${k} = ${v}`)
      setEditKey(null)
    } else {
      setResult("Failed to save config")
    }
  }

  return (
    <Box flexDirection="column" flexGrow={1} paddingX={1}>
      <Box marginBottom={1}>
        <Text bold underline>Configuration</Text>
      </Box>
      {CONFIG_KEYS.map((k, i) => (
        <Box key={k}>
          <Box width={4}><Text dimColor>{i + 1}.</Text></Box>
          <Box width={20}><Text>{k}</Text></Box>
          <Text>{editKey === k ? "→" : ":"}</Text>
          <Box marginLeft={1}>
            {editKey === k ? (
              <TextInput
                defaultValue={editValue}
                onChange={setEditValue}
                onSubmit={(v) => save(k, v)}
              />
            ) : (
              <Text dimColor>{String(config[k])}</Text>
            )}
          </Box>
        </Box>
      ))}
      {result && <Text color="green">{result}</Text>}
      <Box marginTop={1}>
        <Text dimColor>Press number to edit, Esc to go back</Text>
      </Box>
    </Box>
  )
}
