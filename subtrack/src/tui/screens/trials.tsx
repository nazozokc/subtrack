import { Box, Text } from "ink"
import { useMemo } from "react"
import { getTrials } from "../../db.ts"

export function TrialsScreen() {
  const trials = useMemo(() => getTrials(), [])
  const now = new Date()

  return (
    <Box flexDirection="column" flexGrow={1} paddingX={1}>
      <Box marginBottom={1}>
        <Text bold underline>Free Trials</Text>
      </Box>
      {trials.length === 0 ? (
        <Text dimColor>No trials tracked</Text>
      ) : (
        trials.map((t) => {
          const expires = new Date(t.expiresAt)
          const daysLeft = Math.ceil((expires.getTime() - now.getTime()) / 86400000)
          const color = daysLeft < 0 ? "red" : daysLeft <= 3 ? "yellow" : "green"
          return (
            <Box key={t.id}>
              <Box width={24}><Text bold wrap="truncate-end">{t.name}</Text></Box>
              <Box width={14}>
                <Text color={color}>
                  {t.expiresAt} {daysLeft < 0 ? "(expired)" : `(${daysLeft}d)`}
                </Text>
              </Box>
              {t.price !== null && (
                <Text dimColor>
                  → {t.price} {t.currency ?? ""} /{t.cycle ?? "mo"}
                </Text>
              )}
            </Box>
          )
        })
      )}
    </Box>
  )
}
