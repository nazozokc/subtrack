import { Box, Text } from "ink"
import { useMemo } from "react"
import { getSubscriptions } from "../../db.ts"

function formatPrice(price: number, currency: string): string {
  try {
    return new Intl.NumberFormat("en-US", {
      style: "currency", currency, minimumFractionDigits: 0, maximumFractionDigits: 0,
    }).format(price)
  } catch {
    return `${currency} ${price}`
  }
}

export function AnalyticsScreen() {
  const subs = useMemo(() => getSubscriptions(), [])

  const activeSubs = subs.filter((s) => s.status !== "cancelled")

  // Tag frequency
  const tagFreq = useMemo(() => {
    const map = new Map<string, number>()
    for (const sub of activeSubs) {
      for (const tag of sub.tags) {
        map.set(tag, (map.get(tag) ?? 0) + 1)
      }
    }
    return Array.from(map.entries()).sort((a, b) => b[1] - a[1])
  }, [activeSubs])

  // Cost by cycle
  const cycleCost = useMemo(() => {
    const map = new Map<string, number>()
    for (const sub of activeSubs) {
      map.set(sub.cycle, (map.get(sub.cycle) ?? 0) + sub.price)
    }
    return Array.from(map.entries()).sort((a, b) => b[1] - a[1])
  }, [activeSubs])

  // Most expensive
  const sortedByPrice = useMemo(
    () => [...activeSubs].sort((a, b) => b.price - a.price).slice(0, 5),
    [activeSubs],
  )

  return (
    <Box flexDirection="column" flexGrow={1} paddingX={1}>
      <Box marginBottom={1}>
        <Text bold underline>Analytics</Text>
      </Box>

      {activeSubs.length === 0 ? (
        <Text dimColor>No active subscriptions</Text>
      ) : (
        <>
          <Box marginBottom={1} flexDirection="column">
            <Text bold color="cyan">Most Expensive (top 5)</Text>
            {sortedByPrice.map((sub, i) => (
              <Box key={sub.id}>
                <Box width={4}><Text dimColor>{(i + 1) + "."}</Text></Box>
                <Box width={24}><Text bold wrap="truncate-end">{sub.name}</Text></Box>
                <Box width={14}><Text>{formatPrice(sub.price, sub.currency)}</Text></Box>
                <Text dimColor>/{sub.cycle}</Text>
              </Box>
            ))}
          </Box>

          <Box marginBottom={1} flexDirection="column">
            <Text bold color="cyan">Cost by Cycle</Text>
            {cycleCost.map(([cycle, total]) => (
              <Box key={cycle}>
                <Box width={16}><Text>{cycle}</Text></Box>
                <Text dimColor>{
                  // Show first currency group
                  formatPrice(total, subs.find((s) => s.cycle === cycle)?.currency ?? "USD")
                } total</Text>
              </Box>
            ))}
          </Box>

          <Box flexDirection="column">
            <Text bold color="cyan">Top Tags</Text>
            {tagFreq.length === 0 ? (
              <Text dimColor>No tags</Text>
            ) : (
              tagFreq.slice(0, 10).map(([tag, count]) => (
                <Box key={tag}>
                  <Box width={20}><Text bold wrap="truncate-end">{tag}</Text></Box>
                  <Text dimColor>{count} subscription{count !== 1 ? "s" : ""}</Text>
                </Box>
              ))
            )}
          </Box>
        </>
      )}
    </Box>
  )
}
