import { Box, Text, useWindowSize } from "ink"
import { getSubscriptions } from "../../db.ts"
import { useTui } from "../context/app-context.tsx"
import type { SharedArgs, Status } from "../../types.ts"
import { useMemo, useEffect } from "react"
import { formatPrice } from "../../price.ts"

// ── Status color helpers ──────────────────────────────

function statusColor(status: Status): string {
  switch (status) {
    case "active": return "green"
    case "paused": return "yellow"
    case "cancelled": return "red"
    default: return "white"
  }
}

// ── Column layout ────────────────────────────────────

type Column = {
  key: string
  label: string
  minWidth: number
  flex: number
  align: "left" | "right"
}

const COLUMNS: Column[] = [
  { key: "name", label: "Name", minWidth: 8, flex: 4, align: "left" },
  { key: "status", label: "Status", minWidth: 6, flex: 2, align: "left" },
  { key: "cycle", label: "Cycle", minWidth: 6, flex: 2, align: "left" },
  { key: "tags", label: "Tags", minWidth: 4, flex: 3, align: "left" },
  { key: "price", label: "Price", minWidth: 8, flex: 3, align: "right" },
]

function calcWidths(availableWidth: number): number[] {
  const totalFlex = COLUMNS.reduce((s, c) => s + c.flex, 0)
  const widths = COLUMNS.map((c) => c.minWidth)
  const remaining = availableWidth - widths.reduce((s, w) => s + w, 0)

  if (remaining > 0) {
    let allocated = 0
    for (let i = 0; i < COLUMNS.length - 1; i++) {
      const extra = Math.floor((remaining * COLUMNS[i].flex) / totalFlex)
      widths[i] += extra
      allocated += extra
    }
    // Give all remaining pixels to the last column
    widths[COLUMNS.length - 1] += remaining - allocated
  }

  return widths
}

// ── Component ────────────────────────────────────────

export function ListScreen() {
  const { state, dispatch } = useTui()
  const { columns: termCols, rows: termRows } = useWindowSize()

  // Available content area
  const sidebarWidth = 24 // 22 + border padding
  const availableWidth = Math.max(40, termCols - sidebarWidth)
  const headerHeight = 3 // status border + title row + separator
  const footerHeight = 3 // command bar border
  const availableHeight = Math.max(5, termRows - headerHeight - footerHeight)

  const widths = calcWidths(availableWidth)

  // Fetch and filter subscriptions
  const subs = useMemo(() => {
    let all = getSubscriptions()
    if (state.filterText) {
      const q = state.filterText.toLowerCase()
      all = all.filter(
        (s) =>
          s.name.toLowerCase().includes(q) ||
          s.tags.some((t) => t.toLowerCase().includes(q)),
      )
    }
    return all
  }, [state.filterText])

  // Sync selected subscription ID with global state
  useEffect(() => {
    if (subs.length === 0) {
      dispatch({ type: "SET_SELECTED_SUB_ID", id: null })
    } else if (state.listIndex >= subs.length) {
      dispatch({ type: "SET_SELECTED_SUB_ID", id: subs[subs.length - 1].id })
    } else {
      dispatch({ type: "SET_SELECTED_SUB_ID", id: subs[state.listIndex].id })
    }
  }, [state.listIndex, subs, dispatch])

  // Scroll offset
  const maxVisible = Math.max(1, availableHeight - 2) // -2 for header/separator
  const scrollOffset = Math.max(
    0,
    Math.min(state.listIndex - Math.floor(maxVisible / 2), Math.max(0, subs.length - maxVisible)),
  )

  const visibleSubs = subs.slice(scrollOffset, scrollOffset + maxVisible)

  // Totals by currency
  const totals = useMemo(() => {
    const map = new Map<string, number>()
    for (const sub of subs) {
      map.set(sub.currency, (map.get(sub.currency) ?? 0) + sub.price)
    }
    return map
  }, [subs])

  // ── Render ──

  return (
    <Box flexDirection="column" flexGrow={1} paddingX={1}>
      {/* Title + count */}
      <Box>
        <Text bold>
          Subscriptions{" "}
        </Text>
        <Text dimColor>
          ({subs.length} total)
        </Text>
        {state.filterText && (
          <Text dimColor>
            {" "}filtered by "{state.filterText}"
          </Text>
        )}
      </Box>

      {/* Header row */}
      <Box>
        {COLUMNS.map((col, i) => (
          <Box key={col.key} width={widths[i]}>
            <Text bold underline color="cyan">
              {col.align === "right"
                ? col.label.padStart(widths[i])
                : col.label.padEnd(widths[i])}
            </Text>
          </Box>
        ))}
      </Box>

      <Text dimColor>
        {"─".repeat(availableWidth)}
      </Text>

      {/* Data rows */}
      {visibleSubs.length === 0 ? (
        <Box flexGrow={1} alignItems="center" justifyContent="center">
          <Text dimColor>
            {state.filterText ? "No subscriptions match filter" : "No subscriptions yet"}
          </Text>
        </Box>
      ) : (
        visibleSubs.map((sub, idx) => {
          const globalIdx = scrollOffset + idx
          const isSelected = globalIdx === state.listIndex
          const isActiveFocus = state.focus === "content"

          return (
            <Box key={sub.id}>
              {/* Name */}
              <Box width={widths[0]}>
                <Text
                  bold={isSelected && isActiveFocus}
                  inverse={isSelected && isActiveFocus}
                  wrap="truncate-end"
                >
                  {sub.name.padEnd(widths[0]).slice(0, widths[0])}
                </Text>
              </Box>
              {/* Status */}
              <Box width={widths[1]}>
                <Text
                  color={statusColor(sub.status)}
                  inverse={isSelected && isActiveFocus}
                >
                  {sub.status.padEnd(widths[1]).slice(0, widths[1])}
                </Text>
              </Box>
              {/* Cycle */}
              <Box width={widths[2]}>
                <Text
                  inverse={isSelected && isActiveFocus}
                  dimColor={!isSelected}
                >
                  {sub.cycle.padEnd(widths[2]).slice(0, widths[2])}
                </Text>
              </Box>
              {/* Tags */}
              <Box width={widths[3]}>
                <Text
                  inverse={isSelected && isActiveFocus}
                  wrap="truncate-end"
                >
                  {(sub.tags.length > 0
                    ? sub.tags.join(", ")
                    : "—"
                  ).padEnd(widths[3]).slice(0, widths[3])}
                </Text>
              </Box>
              {/* Price */}
              <Box width={widths[4]} justifyContent="flex-end">
                <Text
                  bold
                  inverse={isSelected && isActiveFocus}
                >
                  {formatPrice(sub.price, sub.currency).padStart(widths[4]).slice(0, widths[4])}
                </Text>
              </Box>
            </Box>
          )
        })
      )}

      {/* Totals */}
      {totals.size > 0 && (
        <>
          <Text dimColor>
            {"─".repeat(availableWidth)}
          </Text>
          {Array.from(totals.entries()).map(([currency, total]) => (
            <Box key={currency}>
              <Box width={availableWidth - widths[widths.length - 1]}>
                <Text bold dimColor>
                  {"Total".padEnd(availableWidth - widths[widths.length - 1])}
                </Text>
              </Box>
              <Box width={widths[widths.length - 1]} justifyContent="flex-end">
                <Text bold color="yellow">
                  {formatPrice(total, currency)}
                </Text>
              </Box>
            </Box>
          ))}
        </>
      )}
    </Box>
  )
}
