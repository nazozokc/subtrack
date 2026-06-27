import { Box, Text } from "ink"
import { useMemo } from "react"
import { getSubscriptions } from "../../db.ts"

const OCCURRENCES_PER_YEAR: Record<string, number> = {
  weekly: 52, "bi-weekly": 26, monthly: 12,
  quarterly: 4, "semi-annual": 2, yearly: 1,
}

export function PaymentScreen() {
  const subs = useMemo(() => getSubscriptions(), [])
  const active = subs.filter((s) => s.status !== "cancelled")

  const monthlyByCurrency = useMemo(() => {
    const map = new Map<string, number>()
    for (const sub of active) {
      const factor = (OCCURRENCES_PER_YEAR[sub.cycle] ?? 12) / 12
      map.set(sub.currency, (map.get(sub.currency) ?? 0) + Math.round(sub.price * factor))
    }
    return Array.from(map.entries()).sort((a, b) => b[1] - a[1])
  }, [active])

  const yearlyByCurrency = useMemo(() => {
    const map = new Map<string, number>()
    for (const sub of active) {
      const factor = (OCCURRENCES_PER_YEAR[sub.cycle] ?? 12)
      map.set(sub.currency, (map.get(sub.currency) ?? 0) + Math.round(sub.price * factor))
    }
    return Array.from(map.entries()).sort((a, b) => b[1] - a[1])
  }, [active])

  return (
    <Box flexDirection="column" flexGrow={1} paddingX={1}>
      <Box marginBottom={1}>
        <Text bold underline>Payment Totals</Text>
      </Box>

      {active.length === 0 ? (
        <Text dimColor>No active subscriptions</Text>
      ) : (
        <>
          <Box marginBottom={1}>
            <Text bold color="cyan">Monthly</Text>
            {monthlyByCurrency.map(([currency, total]) => (
              <Box key={currency}>
                <Box width={12}><Text dimColor>{currency}</Text></Box>
                <Text bold color="yellow">{formatPrice(total, currency)}</Text>
              </Box>
            ))}
          </Box>
          <Box>
            <Text bold color="cyan">Yearly</Text>
            {yearlyByCurrency.map(([currency, total]) => (
              <Box key={currency}>
                <Box width={12}><Text dimColor>{currency}</Text></Box>
                <Text bold color="yellow">{formatPrice(total, currency)}</Text>
              </Box>
            ))}
          </Box>
        </>
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
