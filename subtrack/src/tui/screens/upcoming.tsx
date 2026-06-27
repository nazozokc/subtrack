import { Box, Text } from "ink"
import { useMemo } from "react"
import { getSubscriptions } from "../../db.ts"
import { formatPrice } from "../../price.ts"

export function UpcomingScreen() {
  const subs = useMemo(() => getSubscriptions(), [])
  const today = new Date()
  const in7Days = new Date(today.getTime() + 7 * 86400000)

  const upcoming = useMemo(() => {
    return subs
      .filter((s) => s.status === "active" && s.billingDay !== null)
      .map((s) => ({
        ...s,
        nextBill: computeNextBill(s.billingDay!, today, in7Days),
      }))
      .filter((s) => s.nextBill !== null)
      .sort((a, b) => a.nextBill!.getTime() - b.nextBill!.getTime())
  }, [subs])

  return (
    <Box flexDirection="column" flexGrow={1} paddingX={1}>
      <Box marginBottom={1}>
        <Text bold underline>Upcoming Bills (next 7 days)</Text>
      </Box>
      {upcoming.length === 0 ? (
        <Text dimColor>No bills due in the next 7 days</Text>
      ) : (
        upcoming.map((s) => (
          <Box key={s.id}>
            <Box width={24}><Text bold wrap="truncate-end">{s.name}</Text></Box>
            <Box width={16}>
              <Text color="yellow">
                {s.nextBill!.toLocaleDateString("en-US", { month: "short", day: "numeric" })}
              </Text>
            </Box>
            <Box width={14}><Text>{formatPrice(s.price, s.currency)}</Text></Box>
            <Text dimColor>/{s.cycle}</Text>
          </Box>
        ))
      )}
    </Box>
  )
}

function computeNextBill(day: number, from: Date, until: Date): Date | null {
  const year = from.getFullYear()
  const month = from.getMonth()
  const lastDay = new Date(year, month + 1, 0).getDate()
  const clampedDay = Math.min(day, lastDay)
  let d = new Date(year, month, clampedDay)
  if (d < from) {
    const nextLastDay = new Date(year, month + 2, 0).getDate()
    d = new Date(year, month + 1, Math.min(day, nextLastDay))
  }
  if (d <= until) return d
  return null
}


