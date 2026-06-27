import { Box, Text } from "ink"
import { useMemo } from "react"
import { getLlmUsage } from "../../db.ts"

export function UsageScreen() {
  const entries = useMemo(() => getLlmUsage({ limit: 50 }), [])
  const total = entries.reduce((s, e) => s + e.cost, 0)

  return (
    <Box flexDirection="column" flexGrow={1} paddingX={1}>
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
              <Box width={14}><Text bold wrap="truncate-end">{e.provider}</Text></Box>
              <Box width={20}><Text dimColor wrap="truncate-end">{e.model}</Text></Box>
              <Box width={12}><Text>{e.date}</Text></Box>
              <Box width={10}><Text>${(e.cost / 100).toFixed(4)}</Text></Box>
              {e.description && <Text dimColor wrap="truncate-end">{e.description}</Text>}
            </Box>
          ))}
          <Box marginTop={1}>
            <Text bold color="yellow">Total: ${(total / 100).toFixed(2)}</Text>
          </Box>
        </>
      )}
    </Box>
  )
}
