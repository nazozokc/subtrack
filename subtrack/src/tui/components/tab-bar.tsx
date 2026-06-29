import { Box, Text } from "ink"
import { useMemo } from "react"
import { SIDEBAR_WIDTH } from "../types.ts"

type Props = {
  tabs: readonly string[]
  activeTab: string
  tabLabels: Record<string, string>
}

/**
 * Compact 1-line tab bar.
 * Tabs shrink/grow with available width using fixed-width segments.
 */
export function TabBar({ tabs, activeTab, tabLabels }: Props) {
  // Available width = full terminal minus sidebar + padding
  // We don't have useWindowSize here; rely on parent to give us room.
  // Calculate proportional widths based on tab count.
  const barWidth = useMemo(() => {
    // Tabs render inside the content area; the content area is
    // columns - SIDEBAR_WIDTH - 4 (border + padding). We'll use a
    // reasonable default of 80 and let the parent Box handle overflow.
    return undefined // let Box fill naturally
  }, [tabs.length])

  return (
    <Box flexDirection="column" marginBottom={0}>
      <Box flexGrow={1}>
        {tabs.map((tab) => {
          const isActive = tab === activeTab
          const label = tabLabels[tab] ?? tab

          return (
            <Box key={tab} marginRight={1}>
              {isActive ? (
                <Text bold color="cyan" inverse>
                  {" "}{label}{" "}
                </Text>
              ) : (
                <Text dimColor>
                  {" "}{label}{" "}
                </Text>
              )}
            </Box>
          )
        })}
      </Box>
    </Box>
  )
}
