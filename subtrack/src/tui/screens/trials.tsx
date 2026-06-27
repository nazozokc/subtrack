import { Box, Text } from "ink"
import { useMemo } from "react"
import { getTrials } from "../../db.ts"
import { formatPrice } from "../../price.ts"

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
          const nowMs = now.getTime()
          const expiresMs = expires.getTime()
          const daysLeft = Math.ceil((expiresMs - nowMs) / 86400000)
          const expired = expiresMs < nowMs
          const color = expired ? "red" : daysLeft <= 3 ? "yellow" : "green"
          return (
            <Box key={t.id}>
              <Box width={24}><Text bold wrap="truncate-end">{t.name}</Text></Box>
              <Box width={14}>
                <Text color={color}>
                  {t.expiresAt} {expired ? "(expired)" : `(${daysLeft}d)`}
                </Text>
              </Box>
              {t.price !== null && (
                <Text dimColor>
                  → {formatPrice(t.price, t.currency ?? "USD")} /{t.cycle ?? "mo"}
                </Text>
              )}
            </Box>
          )
        })
      )}
    </Box>
  )
}
