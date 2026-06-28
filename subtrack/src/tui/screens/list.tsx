import { Box, Text, useWindowSize } from "ink"
import Gradient from "ink-gradient"
import { getSubscriptions } from "../../db.ts"
import { useTui, type SortField } from "../context/app-context.tsx"
import type { Status } from "../../types.ts"
import { useMemo, useEffect } from "react"
import { formatPrice } from "../../price.ts"

// ── Sort types ─────────────────────────────────────────

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
  sortField?: SortField
}

const COLUMNS: Column[] = [
  { key: "name", label: "Name", minWidth: 8, flex: 4, align: "left", sortField: "name" },
  { key: "status", label: "Status", minWidth: 8, flex: 2, align: "left", sortField: "status" },
  { key: "cycle", label: "Cycle", minWidth: 6, flex: 2, align: "left", sortField: "cycle" },
  { key: "billingDay", label: "Bill", minWidth: 4, flex: 1, align: "right" },
  { key: "tags", label: "Tags", minWidth: 4, flex: 3, align: "left" },
  { key: "price", label: "Price", minWidth: 10, flex: 3, align: "right", sortField: "price" },
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

// ── Sort indicator ────────────────────────────────────

function sortArrow(field: SortField | undefined, sortField: SortField, desc: boolean): string {
  if (field !== sortField) return ""
  return desc ? " ▼" : " ▲"
}

// ── Component ────────────────────────────────────────

export function ListScreen() {
  const { state, dispatch } = useTui()
  const { columns: termCols, rows: termRows } = useWindowSize()

  const { sortField, sortDesc } = state

  const sidebarWidth = 22
  const availableWidth = Math.max(40, termCols - sidebarWidth - 2) // -2 for sidebar border + screen padding
  const headerHeight = 4
  const footerHeight = 3
  const availableHeight = Math.max(5, termRows - headerHeight - footerHeight)

  const widths = calcWidths(availableWidth)

  // Fetch, sort, and filter subscriptions
  const subs = useMemo(() => {
    const all = getSubscriptions(sortField, sortDesc)
    if (state.filterText) {
      const q = state.filterText.toLowerCase()
      return all.filter(
        (s) =>
          s.name.toLowerCase().includes(q) ||
          s.tags.some((t) => t.toLowerCase().includes(q)) ||
          (s.notes ?? "").toLowerCase().includes(q),
      )
    }
    return all
  }, [state.filterText, sortField, sortDesc, state.refreshKey])

  // Clamp listIndex to valid range
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

  // Scroll position + visual scrollbar
  const maxVisible = Math.max(1, availableHeight - 2)
  const scrollPosition = subs.length > 0
    ? `${clampedListIndex + 1}/${subs.length}`
    : ""
  const scrollOffset = Math.max(
    0,
    Math.min(clampedListIndex - Math.floor(maxVisible / 2), Math.max(0, subs.length - maxVisible)),
  )
  const visibleSubs = subs.slice(scrollOffset, scrollOffset + maxVisible)

  // Visual scrollbar (horizontal, like a progress bar)
  const scrollBarWidth = 8
  const scrollBar = useMemo(() => {
    if (subs.length <= maxVisible) return "        ".split("").map(() => "░").join("")
    const pos = clampedListIndex / (subs.length - 1)
    const filled = Math.round(pos * scrollBarWidth)
    return "▓".repeat(filled) + "░".repeat(scrollBarWidth - filled)
  }, [clampedListIndex, subs.length, maxVisible])

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
      {/* Title bar */}
      <Box marginTop={1}>
        <Box flexGrow={1}>
          <Gradient name="pastel">
            <Text bold>
              Subscriptions{" "}
            </Text>
          </Gradient>
          <Text dimColor>
            {subs.length} total · {activeCount} active
          </Text>
          {state.filterText && (
            <Text color="blue">
              {" "}▶ "{state.filterText.length > 16
                ? state.filterText.slice(0, 16) + "…"
                : state.filterText}"
            </Text>
          )}
          {state.multiSelect.size > 0 && (
            <Text color="yellow" bold>
              {" "}[{state.multiSelect.size}]
            </Text>
          )}
        </Box>
        <Box>
          {scrollPosition && (
            <>
              <Text color="gray">{scrollBar}</Text>
              <Text dimColor>
                {" "}{scrollPosition}{" "}
              </Text>
            </>
          )}
        </Box>
      </Box>

      {/* Column headers with sort indicators */}
      <Box marginTop={1}>
        {COLUMNS.map((col, i) => (
          <Box key={col.key} width={widths[i]}>
            <Text bold underline color={col.sortField === sortField ? "cyan" : "gray"}>
              {col.align === "right"
                ? (col.label + sortArrow(col.sortField, sortField, sortDesc)).padStart(widths[i])
                : (col.label + sortArrow(col.sortField, sortField, sortDesc)).padEnd(widths[i])}
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
          {state.filterText ? (
            <>
              <Text dimColor>🔍 No subscriptions match filter</Text>
              <Box marginTop={1}>
                <Text color="blue">  Esc or Ctrl+L</Text>
                <Text dimColor>  Clear filter</Text>
              </Box>
            </>
          ) : (
            <>
              <Box
                borderStyle="round"
                borderColor="cyan"
                paddingX={2}
                paddingY={1}
                flexDirection="column"
                alignItems="center"
              >
                <Text bold color="cyan">📋 No subscriptions yet</Text>
                <Box marginTop={1}>
                  <Text dimColor>
                    Press  <Text bold color="green">a</Text>  to add your first subscription
                  </Text>
                </Box>
                <Box marginTop={1}>
                  <Text dimColor>
                    Or use  <Text bold color="green">Ctrl+P</Text>  to open command palette
                  </Text>
                </Box>

              </Box>
            </>
          )}
        </Box>
      ) : (
        visibleSubs.map((sub, idx) => {
          const globalIdx = scrollOffset + idx
          const isSelected = globalIdx === clampedListIndex
          const isActiveFocus = state.focus === "content"
          const isMultiSelected = state.multiSelect.has(sub.id)
          const isEven = globalIdx % 2 === 0

          // Selection glow: selected rows get bold + inverse (swap fg/bg)
          // Multi-selected items get a yellow arrow marker
          const selStyle = isSelected && isActiveFocus

          return (
            <Box key={sub.id}>
              {/* Multi-select marker column */}
              <Box width={2}>
                {isMultiSelected ? (
                  <Text bold color="yellow">▶</Text>
                ) : selStyle ? (
                  <Text color="cyan">▸</Text>
                ) : (
                  <Text dimColor> </Text>
                )}
              </Box>
              {/* Name */}
              <Box width={widths[0]}>
                <Text
                  bold={selStyle}
                  inverse={selStyle}
                  wrap="truncate-end"
                  color={selStyle ? "white" : isEven ? "gray" : "white"}
                >
                  {sub.name.padEnd(widths[0]).slice(0, widths[0])}
                </Text>
              </Box>
              {/* Status */}
              <Box width={widths[1]}>
                <Text
                  color={statusColor(sub.status)}
                  inverse={selStyle}
                  dimColor={!selStyle && isEven}
                >
                  {statusLabel(sub.status).padEnd(widths[1]).slice(0, widths[1])}
                </Text>
              </Box>
              {/* Cycle */}
              <Box width={widths[2]}>
                <Text
                  inverse={selStyle}
                  dimColor={!selStyle}
                >
                  {sub.cycle.padEnd(widths[2]).slice(0, widths[2])}
                </Text>
              </Box>
              {/* Billing Day */}
              <Box width={widths[3]} justifyContent="flex-end">
                <Text
                  inverse={selStyle}
                  dimColor={!selStyle && isEven}
                >
                  {sub.billingDay ? String(sub.billingDay).padStart(widths[3]).slice(0, widths[3]) : "—".padStart(widths[3])}
                </Text>
              </Box>
              {/* Tags */}
              <Box width={widths[4]}>
                <Text
                  inverse={selStyle}
                  wrap="truncate-end"
                  dimColor={!selStyle && isEven}
                >
                  {(sub.tags.length > 0
                    ? sub.tags.join(", ")
                    : "—"
                  ).padEnd(widths[4]).slice(0, widths[4])}
                </Text>
              </Box>
              {/* Price */}
              <Box width={widths[5]} justifyContent="flex-end">
                <Text
                  bold
                  inverse={selStyle}
                  dimColor={!selStyle && isEven}
                  color={selStyle ? "white" : "yellow"}
                >
                  {formatPrice(sub.price, sub.currency).padStart(widths[5]).slice(0, widths[5])}
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
