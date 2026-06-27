import { Box, Text } from "ink"
import { useMemo } from "react"
import { getSubscriptions } from "../../db.ts"
import { formatPrice } from "../../price.ts"

export function CompareScreen() {
  const subs = useMemo(() => getSubscriptions(), [])
  const active = subs.filter((s) => s.status === "active")
  const now = new Date()
  const currentMonth = now.getMonth()
  const currentYear = now.getFullYear()

  const monthlyTotal = useMemo(() => {
    const map = new Map<string, number>()
    for (const sub of active) {
      const factor = sub.cycle === "weekly" ? 52/12 : sub.cycle === "bi-weekly" ? 26/12 : sub.cycle === "quarterly" ? 4/12 : sub.cycle === "semi-annual" ? 2/12 : sub.cycle === "yearly" ? 1/12 : 1
      map.set(sub.currency, (map.get(sub.currency) ?? 0) + sub.price * factor)
    }
    for (const [currency, total] of map) {
      map.set(currency, Math.round(total))
    }
    return map
  }, [active])

  return (
    <Box flexDirection="column" flexGrow={1} paddingX={1}>
      <Box marginBottom={1}>
        <Text bold underline>Period Comparison</Text>
      </Box>
      {active.length === 0 ? (
        <Text dimColor>No active subscriptions</Text>
      ) : (
        Array.from(monthlyTotal.entries()).map(([currency, total]) => (
          <Box key={currency}>
            <Box width={20}><Text dimColor>{currency}/month:</Text></Box>
            <Text bold color="yellow">{formatPrice(total, currency)}</Text>
          </Box>
        ))
      )}
    </Box>
  )
}


