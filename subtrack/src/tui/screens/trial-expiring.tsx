import { Box, Text } from "ink"
import { useMemo } from "react"
import { getTrialsExpiringSoon } from "../../db.ts"

export function TrialExpiringScreen() {
  const trials = useMemo(() => getTrialsExpiringSoon(7), [])

  return (
    <Box flexDirection="column" flexGrow={1} paddingX={1}>
      <Box marginBottom={1}>
        <Text bold underline>Trials Expiring Within 7 Days</Text>
      </Box>
      {trials.length === 0 ? (
        <Text dimColor>No trials expiring soon</Text>
      ) : (
        trials.map((t) => (
          <Box key={t.id}>
            <Box width={24}><Text bold wrap="truncate-end">{t.name}</Text></Box>
            <Text color="yellow">{t.expiresAt}</Text>
          </Box>
        ))
      )}
    </Box>
  )
}
