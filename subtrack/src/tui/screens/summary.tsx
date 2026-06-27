import { Box, Text } from "ink"
import { useMemo } from "react"
import { getSubscriptions } from "../../db.ts"
import { useTui } from "../context/app-context.tsx"
import type { Status } from "../../types.ts"

function formatPrice(price: number, currency: string): string {
  try {
    return new Intl.NumberFormat("en-US", {
      style: "currency", currency, minimumFractionDigits: 0, maximumFractionDigits: 0,
    }).format(price)
  } catch {
    return `${currency} ${price}`
  }
}

export function SummaryScreen() {
  const { dispatch } = useTui()

  const subs = useMemo(() => getSubscriptions(), [])

  const totalActive = subs.filter((s) => s.status === "active").length
  const totalPaused = subs.filter((s) => s.status === "paused").length
  const totalCancelled = subs.filter((s) => s.status === "cancelled").length

  // Monthly cost by currency
  const monthlyByCurrency = useMemo(() => {
    const map = new Map<string, number>()
    for (const sub of subs) {
      if (sub.status === "cancelled") continue
      const factor = sub.cycle === "weekly" ? 52/12
        : sub.cycle === "bi-weekly" ? 26/12
        : sub.cycle === "quarterly" ? 4/12
        : sub.cycle === "semi-annual" ? 2/12
        : sub.cycle === "yearly" ? 1/12
        : 1
      map.set(sub.currency, (map.get(sub.currency) ?? 0) + Math.round(sub.price * factor))
    }
    return map
  }, [subs])

  return (
    <Box flexDirection="column" flexGrow={1} paddingX={1}>
      <Box marginBottom={1}>
        <Text bold underline>Subscription Summary</Text>
      </Box>

      <Box flexDirection="column" gap={1}>
        <Box>
          <Box width={20}><Text dimColor>Total subscriptions:</Text></Box>
          <Text bold>{subs.length}</Text>
        </Box>
        <Box>
          <Box width={20}><Text dimColor>Active:</Text></Box>
          <Text bold color="green">{totalActive}</Text>
        </Box>
        <Box>
          <Box width={20}><Text dimColor>Paused:</Text></Box>
          <Text bold color="yellow">{totalPaused}</Text>
        </Box>
        <Box>
          <Box width={20}><Text dimColor>Cancelled:</Text></Box>
          <Text bold color="red">{totalCancelled}</Text>
        </Box>

        <Box marginTop={1}>
          <Text bold underline>Monthly Cost</Text>
        </Box>
        {monthlyByCurrency.size === 0 ? (
          <Text dimColor>No active subscriptions</Text>
        ) : (
          Array.from(monthlyByCurrency.entries()).map(([currency, total]) => (
            <Box key={currency}>
              <Box width={20}><Text dimColor>{currency}:</Text></Box>
              <Text bold color="yellow">{formatPrice(total, currency)}/mo</Text>
            </Box>
          ))
        )}
      </Box>
    </Box>
  )
}
