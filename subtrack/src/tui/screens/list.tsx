import { Box, Text, useWindowSize, useInput, useApp } from "ink"
import { getSubscriptions, getSubscription, updateSubscription } from "../../db.ts"
import { useTui, type SortField } from "../context/app-context.tsx"
import type { Status } from "../../types.ts"
import { SIDEBAR_WIDTH } from "../types.ts"
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
    case "active": return "active"
    case "paused": return "paused"
    case "cancelled": return "cancelled"
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
  { key: "status", label: "Status", minWidth: 7, flex: 2, align: "left", sortField: "status" },
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

  const LAYOUT_OVERHEAD = 2 // StatusBar + CommandBar (both 1 row each)
  const availableWidth = Math.max(40, termCols - SIDEBAR_WIDTH - 3) // sidebar border + padding
  const HEADER_ROWS = 2 // title line + column headers line

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

  const totals = useMemo(() => {
    const map = new Map<string, number>()
    for (const sub of subs) {
      map.set(sub.currency, (map.get(sub.currency) ?? 0) + sub.price)
    }
    return map
  }, [subs])

  const activeCount = useMemo(() => subs.filter((s) => s.status === "active").length, [subs])
  const { exit } = useApp()

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

  // Calculate rows available for data
  const footerRows = totals.size > 0 ? totals.size + 1 : 0 // separator + currency lines
  const dataRows = Math.max(1, termRows - LAYOUT_OVERHEAD - HEADER_ROWS - footerRows)
  const scrollOffset = Math.max(
    0,
    Math.min(clampedListIndex - Math.floor(dataRows / 2), Math.max(0, subs.length - dataRows)),
  )
  const visibleSubs = subs.slice(scrollOffset, scrollOffset + dataRows)

  // Scroll position indicator
  const scrollPos = subs.length > dataRows
    ? `${clampedListIndex + 1}/${subs.length}`
    : subs.length > 0
      ? `${subs.length}`
      : ""

  // ── Keyboard handler ──
  useInput(
    (input: string, key) => {
      if (state.focus !== "content") return

      if (key.upArrow || input === "k") {
        dispatch({ type: "SET_LIST_INDEX", index: Math.max(0, state.listIndex - 1) })
        return
      }
      if (key.downArrow || input === "j") {
        dispatch({ type: "SET_LIST_INDEX", index: state.listIndex + 1 })
        return
      }
      if (key.pageUp || (key.ctrl && input === "u")) {
        const jump = Math.max(1, Math.floor(dataRows / 2))
        dispatch({ type: "SET_LIST_INDEX", index: Math.max(0, state.listIndex - jump) })
        return
      }
      if (key.pageDown || (key.ctrl && input === "d")) {
        const jump = Math.max(1, Math.floor(dataRows / 2))
        dispatch({ type: "SET_LIST_INDEX", index: state.listIndex + jump })
        return
      }
      if (key.home || input === "g") {
        dispatch({ type: "SET_LIST_INDEX", index: 0 })
        return
      }
      if (key.end || input === "G") {
        dispatch({ type: "SET_LIST_INDEX", index: Number.MAX_SAFE_INTEGER })
        return
      }

      if (input === "a") {
        dispatch({ type: "SET_SCREEN", screen: "add" })
        return
      }
      if (key.return && state.selectedId !== null) {
        dispatch({ type: "SET_SCREEN", screen: "detail" })
        return
      }
      if (input === "e" && state.selectedId !== null) {
        dispatch({ type: "SET_SCREEN", screen: "edit" })
        return
      }
      if (input === "d" && state.selectedId !== null) {
        dispatch({ type: "SET_SCREEN", screen: "delete" })
        return
      }
      if (input === "/") {
        dispatch({ type: "SET_MODE", mode: "COMMAND" })
        dispatch({ type: "SET_FILTER_TEXT", value: "/" })
        return
      }
      if (input === "v") {
        if (state.selectedId !== null) {
          dispatch({ type: "MULTI_SELECT_TOGGLE", id: state.selectedId })
        }
        return
      }
      if (input === "r") {
        dispatch({ type: "SET_SCREEN", screen: "reports" })
        return
      }
      if (input === "c") {
        dispatch({ type: "SET_SCREEN", screen: "config" })
        return
      }
      if (input === "R") {
        dispatch({ type: "INCREMENT_REFRESH_KEY" })
        return
      }
      if (input === "s") {
        dispatch({ type: "SET_SORT" })
        return
      }
      if (input === "S" && state.selectedId !== null) {
        const sub = getSubscription(state.selectedId)
        if (sub) {
          const cycle: Record<Status, Status> = {
            active: "paused",
            paused: "cancelled",
            cancelled: "active",
          }
          const label: Record<Status, string> = {
            active: "Active",
            paused: "Paused",
            cancelled: "Cancelled",
          }
          const newStatus = cycle[sub.status]
          try {
            updateSubscription(sub.id, { status: newStatus })
            dispatch({ type: "INCREMENT_REFRESH_KEY" })
            dispatch({
              type: "SET_TOAST",
              toast: {
                message: `${sub.name} -> ${label[newStatus]}`,
                type: "info",
              },
            })
          } catch (e: unknown) {
            dispatch({
              type: "SET_TOAST",
              toast: {
                message: `Failed to update ${sub.name}: ${e instanceof Error ? e.message : String(e)}`,
                type: "error",
              },
            })
          }
        }
        return
      }
    },
    { isActive: state.focus === "content" && !state.formActive && !state.paletteOpen },
  )

  // ── Render ──

  return (
    <Box flexDirection="column" flexGrow={1} paddingX={1}>
      {/* Header: title + stats + scroll position */}
      <Box>
        <Box flexGrow={1}>
          <Text bold>
            Subscriptions{" "}
          </Text>
          <Text dimColor>
            {subs.length} total · {activeCount} active
          </Text>
          {state.filterText && (
            <Text color="blue">
              {" /"}{state.filterText.length > 12
                ? state.filterText.slice(0, 12) + "…"
                : state.filterText}
            </Text>
          )}
          {state.multiSelect.size > 0 && (
            <Text color="yellow" bold>
              {" ["}{state.multiSelect.size}{"]"}
            </Text>
          )}
        </Box>
        <Box>
          {scrollPos && (
            <Text dimColor>
              {" "}{scrollPos}{" "}
            </Text>
          )}
        </Box>
      </Box>

      {/* Column headers */}
      <Box>
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

      {/* Separator */}
      <Text dimColor>
        {"─".repeat(availableWidth)}
      </Text>

      {/* Data rows - fills remaining space */}
      {visibleSubs.length === 0 ? (
        <Box flexGrow={1} alignItems="center" justifyContent="center" flexDirection="column">
          {state.filterText ? (
            <>
              <Text dimColor>No subscriptions match filter</Text>
              <Box marginTop={1}>
                <Text color="blue">Esc or Ctrl+L</Text>
                <Text dimColor> {" "}Clear filter</Text>
              </Box>
            </>
          ) : (
            <Box flexDirection="column" alignItems="center">
              <Text dimColor>No subscriptions yet</Text>
              <Box marginTop={1}>
                <Text dimColor>
                  Press <Text bold color="green">a</Text> to add your first subscription
                </Text>
              </Box>
            </Box>
          )}
        </Box>
      ) : (
        visibleSubs.map((sub, idx) => {
          const globalIdx = scrollOffset + idx
          const isSelected = globalIdx === clampedListIndex
          const isActiveFocus = state.focus === "content"
          const isMultiSelected = state.multiSelect.has(sub.id)
          const isEven = globalIdx % 2 === 0
          const selStyle = isSelected && isActiveFocus

          return (
            <Box key={sub.id}>
              {/* Multi-select marker */}
              <Box width={2}>
                {isMultiSelected ? (
                  <Text bold color="yellow">{">"}</Text>
                ) : selStyle ? (
                  <Text color="cyan">{">"}</Text>
                ) : (
                  <Text> </Text>
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
                <Text inverse={selStyle} dimColor={!selStyle}>
                  {sub.cycle.padEnd(widths[2]).slice(0, widths[2])}
                </Text>
              </Box>
              {/* Billing Day */}
              <Box width={widths[3]} justifyContent="flex-end">
                <Text inverse={selStyle} dimColor={!selStyle && isEven}>
                  {sub.billingDay ? String(sub.billingDay).padStart(widths[3]).slice(0, widths[3]) : "-".padStart(widths[3])}
                </Text>
              </Box>
              {/* Tags */}
              <Box width={widths[4]}>
                <Text inverse={selStyle} wrap="truncate-end" dimColor={!selStyle && isEven}>
                  {(sub.tags.length > 0 ? sub.tags.join(", ") : "-")
                    .padEnd(widths[4]).slice(0, widths[4])}
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

      {/* Footer: totals by currency */}
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
