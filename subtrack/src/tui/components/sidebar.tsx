import { Box, Text } from "ink"
import {
  SIDEBAR_ITEMS,
  type SidebarItem,
  type SidebarSection,
} from "../types.ts"
import { useTui } from "../context/app-context.tsx"

const SECTION_HEADS: Record<SidebarSection, string> = {
  data: "📋 Data",
  "tags-trials-bulk": "🏷️ Tags",
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

/**
 * Row descriptor for sidebar rendering.
 * - "head": section header, not selectable
 * - "item": sidebar item, selectable
 * The `itemIndex` is the index into SIDEBAR_ITEMS[], -1 for headers.
 */
type Row = { kind: "head"; section: SidebarSection; itemIndex: -1 } | { kind: "item"; section: SidebarSection; item: SidebarItem; itemIndex: number }

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

/** Screens that only exist as sub‑routes and are not directly reachable from the sidebar. */
const PLACEHOLDER_SCREENS = new Set([
  "tag-manage", "trial-add", "trial-expiring",
])

export function Sidebar() {
  const { state, dispatch } = useTui()
  const isFocused = state.focus === "sidebar"
  const rows = buildRows()

  // Current selected screen index
  const currentIdx = SIDEBAR_ITEMS.findIndex((i) => i.screen === state.screen)

  return (
    <Box
      width={22}
      flexDirection="column"
      borderStyle="round"
      borderColor={isFocused ? "cyan" : "gray"}
      minHeight={20}
    >
      {rows.map((row) => {
        if (row.kind === "head") {
          return (
            <Box key={`head-${row.section}`} marginTop={1}>
              <Text dimColor bold>
                {" "}{SECTION_HEADS[row.section]}
              </Text>
            </Box>
          )
        }

        const { item, itemIndex } = row
        const isCurrent = itemIndex === currentIdx
        const isPlaceholder = PLACEHOLDER_SCREENS.has(item.screen)

        // Focus highlight: when sidebar is focused and this item is the "selected" one
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
            ) : isPlaceholder ? (
              <Text dimColor>
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
