import { Box, Text, useInput } from "ink"
import { TextInput } from "@inkjs/ui"
import { useState, useMemo, useEffect, useCallback } from "react"
import { useTui, useSetFormActive } from "../context/app-context.tsx"
import { loadConfig, setConfig, CONFIG_KEYS } from "../../config.ts"
import type { ConfigKey } from "../../config.ts"
import { colors, borderStyle } from "../theme.ts"
import { Header } from "../components/header.tsx"

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
    if (key.escape && editKey) {
      setEditKey(null)
      setResult(null)
    }
    if (!editKey && /^\d$/.test(input)) {
      const idx = Number(input) - 1
      if (idx >= 0 && idx < CONFIG_KEYS.length) {
        setEditKey(CONFIG_KEYS[idx])
        setEditValue(String(config[CONFIG_KEYS[idx] as ConfigKey]))
      }
    }
  })

  const save = useCallback((k: string, v: string) => {
    const ok = setConfig(k as ConfigKey, v)
    if (ok) {
      setRev((n) => n + 1)
      setResult(`Saved ${k} = ${v}`)
      setEditKey(null)
    } else {
      setResult("Failed to save config")
    }
  }, [])

  return (
    <Box flexDirection="column" flexGrow={1}>
      <Header>Configuration</Header>

      <Box
        borderStyle={borderStyle}
        borderColor={colors.border}
        paddingX={1}
        paddingY={1}
        flexDirection="column"
        flexGrow={1}
      >
        {CONFIG_KEYS.map((k, i) => (
          <Box key={k}>
            <Box width={4}>
              <Text color={colors.textDim}>{i + 1}.</Text>
            </Box>
            <Box width={20}>
              <Text bold>{k}:</Text>
            </Box>
            <Box flexGrow={1}>
              {editKey === k ? (
                <TextInput
                  defaultValue={editValue}
                  onChange={setEditValue}
                  onSubmit={(v) => save(k, v)}
                />
              ) : (
                <Text color={colors.textDim}>{String(config[k])}</Text>
              )}
            </Box>
          </Box>
        ))}
      </Box>

      {result && (
        <Box marginTop={1} borderStyle={borderStyle} borderColor={colors.success} paddingX={1}>
          <Text color={colors.success}>{result}</Text>
        </Box>
      )}

      <Box marginTop={1}>
        <Text color={colors.textDim}>
          Press a number to edit, Enter to save, Esc to go back
        </Text>
      </Box>
    </Box>
  )
}
