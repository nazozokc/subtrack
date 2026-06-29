import { Box, Text, useWindowSize, useInput, useApp } from "ink"
import Gradient from "ink-gradient"
import { getSubscriptions, getSubscription, updateSubscription } from "../../db.ts"
import { useTui, type SortField } from "../context/app-context.tsx"
import type { Status } from "../../types.ts"
import { SIDEBAR_WIDTH } from "../types.ts"
import { useMemo, useEffect } from "react"
import { formatPrice } from "../../price.ts"
import { colors, statusColor, statusLabel } from "../theme.ts"

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

  // Available width: full terminal minus sidebar (if shown) minus border(2)
  const sidebarW = state.showSidebar ? SIDEBAR_WIDTH + 1 : 0 // +1 for │ separator
  const availableWidth = Math.max(40, termCols - sidebarW - 3)
  const LAYOUT_OVERHEAD = 3 // StatusBar + CommandBar + border top/bottom
  const availableHeight = Math.max(5, termRows - LAYOUT_OVERHEAD - 1) // -1 for header

  const widths = calcWidths(availableWidth)

  // ── Data ──

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

  // Clamp listIndex
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

  // ── Scroll ──

  const maxVisible = Math.max(1, availableHeight - 1) // -1 for totals row
  const scrollPosition = subs.length > 0
    ? `${clampedListIndex + 1}/${subs.length}`
    : ""
  const scrollOffset = Math.max(
    0,
    Math.min(clampedListIndex - Math.floor(maxVisible / 2), Math.max(0, subs.length - maxVisible)),
  )
  const visibleSubs = subs.slice(scrollOffset, scrollOffset + maxVisible)

  // Visual scrollbar (thin, right side)
  const scrollBarWidth = 6
  const scrollBar = useMemo(() => {
    if (subs.length <= maxVisible) return ""
    const pos = clampedListIndex / (subs.length - 1)
    const filled = Math.round(pos * scrollBarWidth)
    return "▓".repeat(filled) + "░".repeat(scrollBarWidth - filled)
  }, [clampedListIndex, subs.length, maxVisible])

  // ── Totals ──

  const totals = useMemo(() => {
    const map = new Map<string, number>()
    for (const sub of subs) {
      map.set(sub.currency, (map.get(sub.currency) ?? 0) + sub.price)
    }
    return map
  }, [subs])

  const activeCount = useMemo(() => subs.filter((s) => s.status === "active").length, [subs])
  const { exit } = useApp()

  // ── Keyboard ──

  useInput(
    (input: string, key) => {
      if (state.focus !== "content") return

      // Navigation
      if (key.upArrow || input === "k") {
        dispatch({ type: "SET_LIST_INDEX", index: Math.max(0, state.listIndex - 1) })
        return
      }
      if (key.downArrow || input === "j") {
        dispatch({ type: "SET_LIST_INDEX", index: state.listIndex + 1 })
        return
      }
      if (key.pageUp || (key.ctrl && input === "u")) {
        const jump = Math.max(1, Math.floor(termRows / 2))
        dispatch({ type: "SET_LIST_INDEX", index: Math.max(0, state.listIndex - jump) })
        return
      }
      if (key.pageDown || (key.ctrl && input === "d")) {
        const jump = Math.max(1, Math.floor(termRows / 2))
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

      // 3-pane: toggle detail preview
      if (input === "|") {
        if (state.selectedId !== null) {
          dispatch({ type: "TOGGLE_DETAIL" })
        }
        return
      }

      // Actions
      if (input === "a") {
        dispatch({ type: "SET_SCREEN", screen: "add" })
        return
      }
      // Enter in 3-pane mode: toggle detail; without 3-pane: go to detail screen
      if (key.return && state.selectedId !== null) {
        if (state.showDetail) {
          dispatch({ type: "SET_SCREEN", screen: "detail" })
        } else {
          dispatch({ type: "TOGGLE_DETAIL" })
        }
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
                message: `${sub.name} → ${label[newStatus]}`,
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

  const barshow = scrollBar && scrollPosition ? `${scrollBar} ${scrollPosition}` : ""

  return (
    <Box flexDirection="column" flexGrow={1}>
      {/* ── Row 1: Title + stats ── */}
      <Box>
        <Box flexGrow={1}>
          <Gradient name="pastel">
            <Text bold>
              Subscriptions{" "}
            </Text>
          </Gradient>
          <Text color={colors.textDim}>
            {subs.length} total · {activeCount} active
          </Text>
          {state.filterText && (
            <Text color={colors.info}>
              {"  ▶ "}
              {state.filterText.length > 20
                ? state.filterText.slice(0, 20) + "…"
                : state.filterText}
            </Text>
          )}
          {state.multiSelect.size > 0 && (
            <Text bold color={colors.warning}>
              {"  ["}{state.multiSelect.size}{"]"}
            </Text>
          )}
        </Box>
        {barshow && (
          <Box flexShrink={0}>
            <Text color={colors.textDim}>{barshow}</Text>
          </Box>
        )}
      </Box>

      {/* ── Row 2: Column headers ── */}
      <Box>
        {/* Multi-select spacer */}
        <Box width={2} />
        {COLUMNS.map((col, i) => (
          <Box key={col.key} width={widths[i]}>
            <Text
              bold
              underline
              color={col.sortField === sortField ? colors.primary : colors.textDim}
            >
              {col.align === "right"
                ? (col.label + sortArrow(col.sortField, sortField, sortDesc)).padStart(widths[i])
                : (col.label + sortArrow(col.sortField, sortField, sortDesc)).padEnd(widths[i])}
            </Text>
          </Box>
        ))}
      </Box>

      {/* ── Data rows ── */}
      <Box flexGrow={1} flexDirection="column" minHeight={0}>
        {visibleSubs.length === 0 ? (
          <Box flexGrow={1} alignItems="center" justifyContent="center" flexDirection="column">
            {state.filterText ? (
              <>
                <Text color={colors.textDim}>🔍 No subscriptions match filter</Text>
                <Box marginTop={1}>
                  <Text color={colors.info}>  Esc or Ctrl+L</Text>
                  <Text color={colors.textDim}>  Clear filter</Text>
                </Box>
              </>
            ) : (
              <Box
                borderStyle="round"
                borderColor={colors.primary}
                paddingX={2}
                paddingY={1}
                flexDirection="column"
                alignItems="center"
              >
                <Text bold color={colors.primary}>📋 No subscriptions yet</Text>
                <Box marginTop={1}>
                  <Text color={colors.textDim}>
                    Press  <Text bold color={colors.success}>a</Text>  to add your first subscription
                  </Text>
                </Box>
                <Box marginTop={1}>
                  <Text color={colors.textDim}>
                    Or use  <Text bold color={colors.success}>Ctrl+P</Text>  to open command palette
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

            // Selection: use background-color highlight (not inverse)
            const selStyle = isSelected && isActiveFocus

            return (
              <Box key={sub.id} minHeight={1}>
                {/* Multi-select / selection marker */}
                <Box width={2} flexShrink={0}>
                  {isMultiSelected ? (
                    <Text bold color={colors.warning}>●</Text>
                  ) : selStyle ? (
                    <Text color={colors.primary}>▸</Text>
                  ) : (
                    <Text color={colors.textDim}> </Text>
                  )}
                </Box>

                {/* Name */}
                <Box width={widths[0]}>
                  {selStyle ? (
                    <Text
                      bold
                      color={colors.selectedFg}
                      backgroundColor={colors.selectedBg}
                      wrap="truncate-end"
                    >
                      {sub.name.padEnd(widths[0]).slice(0, widths[0])}
                    </Text>
                  ) : (
                    <Text
                      color={isEven ? colors.textDim : colors.text}
                      wrap="truncate-end"
                    >
                      {sub.name.padEnd(widths[0]).slice(0, widths[0])}
                    </Text>
                  )}
                </Box>

                {/* Status */}
                <Box width={widths[1]}>
                  <Text
                    color={statusColor(sub.status)}
                    backgroundColor={selStyle ? colors.selectedBg : undefined}
                  >
                    {statusLabel(sub.status).padEnd(widths[1]).slice(0, widths[1])}
                  </Text>
                </Box>

                {/* Cycle */}
                <Box width={widths[2]}>
                  <Text
                    color={isEven && !selStyle ? colors.textDim : colors.text}
                    backgroundColor={selStyle ? colors.selectedBg : undefined}
                  >
                    {sub.cycle.padEnd(widths[2]).slice(0, widths[2])}
                  </Text>
                </Box>

                {/* Billing Day */}
                <Box width={widths[3]} justifyContent="flex-end">
                  <Text
                    color={isEven && !selStyle ? colors.textDim : colors.text}
                    backgroundColor={selStyle ? colors.selectedBg : undefined}
                  >
                    {sub.billingDay
                      ? String(sub.billingDay).padStart(widths[3]).slice(0, widths[3])
                      : "—".padStart(widths[3])}
                  </Text>
                </Box>

                {/* Tags */}
                <Box width={widths[4]}>
                  <Text
                    wrap="truncate-end"
                    color={isEven && !selStyle ? colors.textDim : colors.text}
                    backgroundColor={selStyle ? colors.selectedBg : undefined}
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
                    color={selStyle ? colors.selectedFg : colors.warning}
                    backgroundColor={selStyle ? colors.selectedBg : undefined}
                  >
                    {formatPrice(sub.price, sub.currency).padStart(widths[5]).slice(0, widths[5])}
                  </Text>
                </Box>
              </Box>
            )
          })
        )}
      </Box>

      {/* ── Totals footer ── */}
      {totals.size > 0 && (
        <Box>
          <Box width={2} />
          <Text bold color={colors.textDim} underline>
            Total{"  "}
          </Text>
          {Array.from(totals.entries()).map(([currency, total]) => (
            <Text key={currency} bold color={colors.warning}>
              {formatPrice(total, currency)}{"  "}
            </Text>
          ))}
        </Box>
      )}
    </Box>
  )
}
