import { Box, Text, useWindowSize } from "ink"
import { getSubscriptions } from "../../db.ts"
import { useTui } from "../context/app-context.tsx"
import type { Status } from "../../types.ts"
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

function statusLabel(status: Status): string {
  switch (status) {
    case "active": return "● active"
    case "paused": return "◐ paused"
    case "cancelled": return "○ cancelled"
    default: return status
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
  { key: "status", label: "Status", minWidth: 8, flex: 2, align: "left" },
  { key: "cycle", label: "Cycle", minWidth: 6, flex: 2, align: "left" },
  { key: "tags", label: "Tags", minWidth: 4, flex: 3, align: "left" },
  { key: "price", label: "Price", minWidth: 10, flex: 3, align: "right" },
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
    widths[COLUMNS.length - 1] += remaining - allocated
  }

  return widths
}

// ── Component ────────────────────────────────────────

export function ListScreen() {
  const { state, dispatch } = useTui()
  const { columns: termCols, rows: termRows } = useWindowSize()

  const sidebarWidth = 24
  const availableWidth = Math.max(40, termCols - sidebarWidth)
  const headerHeight = 3
  const footerHeight = 3
  const filterBarHeight = state.filterText ? 2 : 0
  const availableHeight = Math.max(5, termRows - headerHeight - footerHeight - filterBarHeight)

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

  // Sync selectedId — clamp listIndex to valid range
  const clampedListIndex = useMemo(() => {
    if (subs.length === 0) return 0
    return Math.min(state.listIndex, subs.length - 1)
  }, [state.listIndex, subs.length])

  useEffect(() => {
    if (subs.length === 0) {
      dispatch({ type: "SET_SELECTED_ID", id: null })
    } else {
      dispatch({ type: "SET_SELECTED_ID", id: subs[clampedListIndex].id })
    }
  }, [clampedListIndex, subs, dispatch])

  // Scroll offset (centered)
  const maxVisible = Math.max(1, availableHeight - 2)
  const scrollOffset = Math.max(
    0,
    Math.min(clampedListIndex - Math.floor(maxVisible / 2), Math.max(0, subs.length - maxVisible)),
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

  const activeCount = useMemo(() => subs.filter((s) => s.status === "active").length, [subs])

  // ── Render ──

  return (
    <Box flexDirection="column" flexGrow={1} paddingX={1}>
      {/* Title */}
      <Box>
        <Text bold color="white">
          Subscriptions{" "}
        </Text>
        <Text dimColor>
          ({subs.length} total, {activeCount} active)
        </Text>
        {state.filterText && (
          <Text color="blue">
            {" "}🔍 "{state.filterText}"
          </Text>
        )}
        {state.multiSelect.size > 0 && (
          <Text color="yellow" bold>
            {" "}[{state.multiSelect.size} selected]
          </Text>
        )}
      </Box>

      {/* Column headers */}
      <Box marginTop={1}>
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
        <Box flexGrow={1} alignItems="center" justifyContent="center" flexDirection="column">
          <Text dimColor>
            {state.filterText
              ? "No subscriptions match filter"
              : "No subscriptions yet"}
          </Text>
          {!state.filterText && (
            <Text color="cyan" dimColor={false}>
              {" "}Press  a  to add your first subscription
            </Text>
          )}
        </Box>
      ) : (
        visibleSubs.map((sub, idx) => {
          const globalIdx = scrollOffset + idx
          const isSelected = globalIdx === clampedListIndex
          const isActiveFocus = state.focus === "content"
          const isMultiSelected = state.multiSelect.has(sub.id)
          const isEven = globalIdx % 2 === 0

          return (
            <Box key={sub.id}>
              {/* Multi-select marker */}
              <Box width={2}>
                <Text color="yellow">{isMultiSelected ? "▶" : " "}</Text>
              </Box>
              {/* Name */}
              <Box width={widths[0]}>
                <Text
                  bold={isSelected && isActiveFocus}
                  inverse={isSelected && isActiveFocus}
                  wrap="truncate-end"
                  dimColor={!isSelected && !isEven}
                >
                  {sub.name.padEnd(widths[0]).slice(0, widths[0])}
                </Text>
              </Box>
              {/* Status */}
              <Box width={widths[1]}>
                <Text
                  color={statusColor(sub.status)}
                  inverse={isSelected && isActiveFocus}
                  dimColor={!isSelected && !isEven}
                >
                  {statusLabel(sub.status).padEnd(widths[1]).slice(0, widths[1])}
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
                  dimColor={!isSelected && !isEven}
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
                  dimColor={!isSelected && !isEven}
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
