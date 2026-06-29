import { Box, Text } from "ink"
import {
  SIDEBAR_ITEMS,
  SIDEBAR_WIDTH,
  type SidebarItem,
  type SidebarSection,
  type Screen,
} from "../types.ts"
import { useTui } from "../context/app-context.tsx"

const SECTION_HEADS: Record<SidebarSection, string> = {
  data: "📋 Data",
  reports: "📊 Reports",
  system: "⚙️ System",
}

type SectionGroup = {
  section: SidebarSection
  items: SidebarItem[]
}

function groupItems(): SectionGroup[] {
  const map = new Map<SidebarSection, SidebarItem[]>()
  for (const item of SIDEBAR_ITEMS) {
    if (!map.has(item.section)) map.set(item.section, [])
    map.get(item.section)!.push(item)
  }
  return Array.from(map.entries()).map(([section, items]) => ({ section, items }))
}

type Row =
  | { kind: "head"; section: SidebarSection; itemIndex: -1 }
  | { kind: "item"; section: SidebarSection; item: SidebarItem; itemIndex: number }

function buildRows(): Row[] {
  const rows: Row[] = []
  let idx = 0
  for (const group of groupItems()) {
    rows.push({ kind: "head", section: group.section, itemIndex: -1 })
    for (const item of group.items) {
      rows.push({ kind: "item", section: group.section, item, itemIndex: idx })
      idx++
    }
  }
  return rows
}

export function Sidebar() {
  const { state } = useTui()
  const isFocused = state.focus === "sidebar"
  const rows = buildRows()
  // Map non-sidebar screens to their parent sidebar item
  const sidebarScreen: Screen =
    state.screen === "edit" || state.screen === "delete" || state.screen === "detail"
      ? "list"
      : state.screen
  const currentIdx = SIDEBAR_ITEMS.findIndex((i) => i.screen === sidebarScreen)

  return (
    <Box
      width={SIDEBAR_WIDTH}
      flexDirection="column"
      borderStyle="round"
      borderColor={isFocused ? "cyan" : "gray"}
      flexGrow={1}
    >
      {rows.map((row, ri) => {
        if (row.kind === "head") {
          const isFirst = ri === 0
          return (
            <Box key={`head-${row.section}`} marginTop={isFirst ? 0 : 1} paddingLeft={1}>
              <Text dimColor bold>
                {" "}{SECTION_HEADS[row.section]}
              </Text>
            </Box>
          )
        }

        const { item, itemIndex } = row
        const isCurrent = itemIndex === currentIdx
        const focusedHere = isFocused && itemIndex === state.sidebarIndex

        return (
          <Box key={item.screen} paddingLeft={1}>
            {focusedHere ? (
              <Text bold color="cyan" inverse>
                {" "}{item.icon} {item.label.padEnd(14)}{" "}
              </Text>
            ) : isCurrent ? (
              <Text bold color="cyan">
                {" "}{item.icon} {item.label.padEnd(14)}{" "}
              </Text>
            ) : (
              <Text>
                {" "}{item.icon} {item.label.padEnd(14)}{" "}
              </Text>
            )}
          </Box>
        )
      })}
    </Box>
  )
}
