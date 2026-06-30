import { Box, Text, useInput } from "ink"
import { useState, useMemo } from "react"
import { calcCalendarEntries } from "../../calendar.ts"
import { useTui } from "../context/app-context.tsx"
import { colors } from "../theme.ts"
import { formatPrice } from "../../display.ts"

export function CalendarScreen() {
  const { dispatch } = useTui()
  const now = new Date()
  const [month, setMonth] = useState(now.getMonth() + 1)
  const [year, setYear] = useState(now.getFullYear())

  useInput(
    (input, key) => {
      if (key.leftArrow) {
        if (month === 1) {
          setMonth(12)
          setYear(year - 1)
        } else {
          setMonth(month - 1)
        }
        return
      }
      if (key.rightArrow) {
        if (month === 12) {
          setMonth(1)
          setYear(year + 1)
        } else {
          setMonth(month + 1)
        }
        return
      }
      if (input === "q" || key.escape) {
        dispatch({ type: "GO_BACK" })
        return
      }
    },
    { isActive: true },
  )

  const entries = useMemo(() => calcCalendarEntries(month, year), [month, year])
  const entryMap = useMemo(
    () => new Map(entries.map((e) => [e.day, e.subs])),
    [entries],
  )

  const MONTH_NAMES = [
    "January", "February", "March", "April", "May", "June",
    "July", "August", "September", "October", "November", "December",
  ]

  const firstDay = new Date(year, month - 1, 1).getDay()
  const totalDays = new Date(year, month, 0).getDate()

  const calendarRows: string[] = []
  let line = ""
  for (let i = 0; i < firstDay; i++) line += "   "
  for (let day = 1; day <= totalDays; day++) {
    const dayEntries = entryMap.get(day)
    if (dayEntries) {
      line += ` ${colors.primary}${String(day).padStart(2)}`
    } else {
      line += ` ${String(day).padStart(2)}`
    }
    if ((firstDay + day) % 7 === 0) {
      calendarRows.push(line)
      line = ""
    }
  }
  if (line !== "") calendarRows.push(line)

  return (
    <Box flexDirection="column" flexGrow={1}>
      {/* Header */}
      <Box marginBottom={1}>
        <Text bold color={colors.primary}>
          {"📅 "}{MONTH_NAMES[month - 1]} {year}
        </Text>
      </Box>

      {/* Calendar grid */}
      <Box>
        <Box paddingLeft={2}>
          <Text>
            <Text dimColor>Su Mo Tu We Th Fr Sa{"\n"}</Text>
            {calendarRows.join("\n")}
          </Text>
        </Box>
      </Box>

      {/* Events list */}
      <Box marginTop={1} flexDirection="column">
        {entries.length === 0 ? (
          <Text dimColor>  No billing events this month</Text>
        ) : (
          <>
            <Text bold color={colors.textDim}>Billing events:</Text>
            <Box marginTop={1} flexDirection="column" gap={0}>
              {entries.map((entry) =>
                entry.subs.map((sub) => (
                  <Box key={`${sub.id}-${entry.day}`}>
                    <Box width={8}>
                      <Text color={colors.primary}>Day {String(entry.day).padStart(2)}</Text>
                    </Box>
                    <Box width={22}>
                      <Text bold wrap="truncate-end">{sub.name}</Text>
                    </Box>
                    <Box width={16}>
                      <Text color="yellow">{formatPrice(sub.price, sub.currency)}</Text>
                    </Box>
                    <Box>
                      <Text
                        color={
                          sub.status === "active"
                            ? "green"
                            : sub.status === "paused"
                              ? "yellow"
                              : "gray"
                        }
                      >
                        {sub.status}
                      </Text>
                    </Box>
                  </Box>
                )),
              )}
            </Box>
          </>
        )}
      </Box>

      {/* Navigation hint */}
      <Box marginTop={1}>
        <Text dimColor>
          ← → navigate month · q/Esc back
        </Text>
      </Box>
    </Box>
  )
}
