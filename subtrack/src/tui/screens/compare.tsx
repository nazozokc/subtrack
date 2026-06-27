import { Box, Text } from "ink"
import { useMemo } from "react"
import { getSubscriptions } from "../../db.ts"

export function CompareScreen() {
  const subs = useMemo(() => getSubscriptions(), [])
  const active = subs.filter((s) => s.status !== "cancelled")
  const now = new Date()
  const currentMonth = now.getMonth()
  const currentYear = now.getFullYear()

  const monthlyTotal = useMemo(() => {
    const map = new Map<string, number>()
    for (const sub of active) {
      const factor = sub.cycle === "weekly" ? 52/12 : sub.cycle === "bi-weekly" ? 26/12 : sub.cycle === "quarterly" ? 4/12 : sub.cycle === "semi-annual" ? 2/12 : sub.cycle === "yearly" ? 1/12 : 1
      map.set(sub.currency, (map.get(sub.currency) ?? 0) + Math.round(sub.price * factor))
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

function formatPrice(price: number, currency: string): string {
  try {
    return new Intl.NumberFormat("en-US", {
      style: "currency", currency, minimumFractionDigits: 0, maximumFractionDigits: 0,
    }).format(price)
  } catch {
    return `${currency} ${price}`
  }
}
