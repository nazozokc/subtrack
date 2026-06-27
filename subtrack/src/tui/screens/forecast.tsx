import { Box, Text } from "ink"
import { useMemo } from "react"
import { getSubscriptions } from "../../db.ts"
import { formatPrice } from "../../price.ts"

export function ForecastScreen() {
  const subs = useMemo(() => getSubscriptions(), [])
  const active = subs.filter((s) => s.status === "active")

  const monthly = useMemo(() => {
    const map = new Map<string, number>()
    for (const sub of active) {
      const f = sub.cycle === "weekly" ? 52/12 : sub.cycle === "bi-weekly" ? 26/12 : sub.cycle === "quarterly" ? 4/12 : sub.cycle === "semi-annual" ? 2/12 : sub.cycle === "yearly" ? 1/12 : 1
      map.set(sub.currency, (map.get(sub.currency) ?? 0) + Math.round(sub.price * f))
    }
    return map
  }, [active])

  const yearly = useMemo(() => {
    const map = new Map<string, number>()
    for (const sub of active) {
      const f = sub.cycle === "weekly" ? 52 : sub.cycle === "bi-weekly" ? 26 : sub.cycle === "quarterly" ? 4 : sub.cycle === "semi-annual" ? 2 : sub.cycle === "yearly" ? 1 : 12
      map.set(sub.currency, (map.get(sub.currency) ?? 0) + Math.round(sub.price * f))
    }
    return map
  }, [active])

  return (
    <Box flexDirection="column" flexGrow={1} paddingX={1}>
      <Box marginBottom={1}>
        <Text bold underline>Spending Forecast</Text>
      </Box>
      {active.length === 0 ? (
        <Text dimColor>No active subscriptions</Text>
      ) : (
        <>
          {Array.from(monthly.entries()).map(([c, t]) => (
            <Box key={c}>
              <Box width={16}><Text dimColor>Monthly ({c})</Text></Box>
              <Text bold>{formatPrice(t, c)}</Text>
            </Box>
          ))}
          {Array.from(yearly.entries()).map(([c, t]) => (
            <Box key={c}>
              <Box width={16}><Text dimColor>Yearly ({c})</Text></Box>
              <Text bold color="yellow">{formatPrice(t, c)}</Text>
            </Box>
          ))}
        </>
      )}
    </Box>
  )
}


