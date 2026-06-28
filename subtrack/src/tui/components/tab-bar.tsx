import { Box, Text } from "ink"

type Props = {
  tabs: readonly string[]
  activeTab: string
  tabLabels: Record<string, string>
}

export function TabBar({ tabs, activeTab, tabLabels }: Props) {
  const barWidth = tabs.length * 20

  return (
    <Box flexDirection="column" marginBottom={1}>
      <Box width={barWidth}>
        {tabs.map((tab, i) => {
          const isActive = tab === activeTab
          const label = `${tabLabels[tab] ?? tab}`
          const paddedLabel = ` ${label.padEnd(16)} `
          const separator = i < tabs.length - 1 ? (
            <Text dimColor>│</Text>
          ) : null

          return (
            <Box key={tab}>
              {isActive ? (
                <Text bold color="cyan" inverse>{paddedLabel}</Text>
              ) : (
                <Text dimColor>{paddedLabel}</Text>
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
