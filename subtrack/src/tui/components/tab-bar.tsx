import { Box, Text, useWindowSize } from "ink"
import { useMemo } from "react"

type Props = {
  tabs: readonly string[]
  activeTab: string
  tabLabels: Record<string, string>
}

export function TabBar({ tabs, activeTab, tabLabels }: Props) {
  const { columns: termCols } = useWindowSize()

  // Derive tab width from available columns (after sidebar + padding)
  const tabWidth = useMemo(() => {
    const available = Math.max(40, termCols - 26) // 22 sidebar + 4 padding
    const perTab = Math.max(10, Math.floor(available / tabs.length))
    return perTab
  }, [termCols, tabs.length])

  const barWidth = tabs.length * tabWidth

  return (
    <Box flexDirection="column" marginBottom={1}>
      <Box width={barWidth}>
        {tabs.map((tab, i) => {
          const isActive = tab === activeTab
          const label = `${tabLabels[tab] ?? tab}`
          const paddedLabel = ` ${label.padEnd(tabWidth - 3)} `
          const separator = i < tabs.length - 1 ? (
            <Text dimColor>│</Text>
          ) : null

          return (
            <Box key={tab} width={tabWidth}>
              {isActive ? (
                <Text bold color="cyan" inverse wrap="truncate-end">{paddedLabel}</Text>
              ) : (
                <Text dimColor wrap="truncate-end">{paddedLabel}</Text>
              )}
              {separator}
            </Box>
          )
        })}
      </Box>
      <Text dimColor>
        {"─".repeat(barWidth)}
      </Text>
    </Box>
  )
}
