import { Box, Text, useInput, useApp } from "ink"
import { AppProvider, useTui } from "./context/app-context.tsx"
import { Sidebar } from "./components/sidebar.tsx"
import { StatusBar } from "./components/status-bar.tsx"
import { CommandBar } from "./components/command-bar.tsx"
import { ListScreen } from "./screens/list.tsx"
import { AddScreen } from "./screens/add.tsx"
import { EditScreen } from "./screens/edit.tsx"
import { DeleteScreen } from "./screens/delete.tsx"
import { SearchScreen } from "./screens/search.tsx"
import { TagsScreen } from "./screens/tags.tsx"
import { TagManageScreen } from "./screens/tag-manage.tsx"
import { TrialsScreen } from "./screens/trials.tsx"
import { TrialAddScreen } from "./screens/trial-add.tsx"
import { TrialExpiringScreen } from "./screens/trial-expiring.tsx"
import { BulkScreen } from "./screens/bulk.tsx"
import { SummaryScreen } from "./screens/summary.tsx"
import { PaymentScreen } from "./screens/payment.tsx"
import { UpcomingScreen } from "./screens/upcoming.tsx"
import { AnalyticsScreen } from "./screens/analytics.tsx"
import { CompareScreen } from "./screens/compare.tsx"
import { ForecastScreen } from "./screens/forecast.tsx"
import { ConfigScreen } from "./screens/config.tsx"
import { UsageScreen } from "./screens/usage.tsx"
import { ExportScreen } from "./screens/export.tsx"
import { ImportScreen } from "./screens/import.tsx"
import { BackupScreen } from "./screens/backup.tsx"
import { RestoreScreen } from "./screens/restore.tsx"
import { HelpScreen } from "./screens/help.tsx"
import { getSubscriptions } from "../db.ts"
import { SIDEBAR_ITEMS, SCREEN_TITLES } from "./types.ts"
import { useRef } from "react"
import type { Screen } from "./types.ts"

// ── Placeholder for unimplemented screens ──────────────

function PlaceholderScreen({ screen }: { screen: Screen }) {
  return (
    <Box flexGrow={1} alignItems="center" justifyContent="center" flexDirection="column">
      <Text bold color="cyan">
        {SCREEN_TITLES[screen]}
      </Text>
      <Text dimColor>
        Not yet implemented — coming in a future update
      </Text>
      <Text dimColor>
        Press Esc to go back
      </Text>
    </Box>
  )
}

// ── Screen router ─────────────────────────────────────

function CurrentScreen() {
  const { state } = useTui()

  switch (state.screen) {
    case "list":
      return <ListScreen />
    case "search":
      return <SearchScreen />
    case "add":
      return <AddScreen />
    case "edit":
      return <EditScreen />
    case "delete":
      return <DeleteScreen />
    case "tags":
      return <TagsScreen />
    case "tag-manage":
      return <TagManageScreen />
    case "trials":
      return <TrialsScreen />
    case "trial-add":
      return <TrialAddScreen />
    case "trial-expiring":
      return <TrialExpiringScreen />
    case "bulk":
      return <BulkScreen />
    case "summary":
      return <SummaryScreen />
    case "payment":
      return <PaymentScreen />
    case "upcoming":
      return <UpcomingScreen />
    case "analytics":
      return <AnalyticsScreen />
    case "compare":
      return <CompareScreen />
    case "forecast":
      return <ForecastScreen />
    case "config":
      return <ConfigScreen />
    case "usage":
      return <UsageScreen />
    case "export":
      return <ExportScreen />
    case "import":
      return <ImportScreen />
    case "backup":
      return <BackupScreen />
    case "restore":
      return <RestoreScreen />
    case "help":
      return <HelpScreen />
    default:
      return <PlaceholderScreen screen={state.screen} />
  }
}

// ── Keyboard handler component ────────────────────────

function KeyboardHandler() {
  const { state, dispatch } = useTui()
  const { exit } = useApp()
  const pendingSeq = useRef<{ key: string; timer: ReturnType<typeof setTimeout> } | null>(null)

  // Form screens use @inkjs/ui components or handle their own input
  const isFormScreen = state.screen === "add" || state.screen === "edit" || state.screen === "delete"

  useInput((input: string, key: any) => {
    const { mode, focus, filterText, confirmQuit, sidebarIndex, selectedSubId } = state
    if (isFormScreen) return

    // ── Quit confirmation handling ──
    if (confirmQuit) {
      if (input === "y" || input === "Y") {
        exit()
      } else {
        dispatch({ type: "SET_CONFIRM_QUIT", value: false })
      }
      return
    }

    // ── MODE: COMMAND ──
    if (mode === "COMMAND") {
      if (key.escape) {
        dispatch({ type: "SET_MODE", mode: "NORMAL" })
        return
      }
      if (key.return) {
        executeCommand(filterText, state, dispatch, exit)
        return
      }
      if (key.backspace) {
        dispatch({ type: "SET_FILTER_TEXT", value: filterText.slice(0, -1) })
        return
      }
      if (input.length === 1) {
        dispatch({ type: "SET_FILTER_TEXT", value: filterText + input })
      }
      return
    }

    // ── MODE: SEARCH ──
    if (mode === "SEARCH") {
      if (key.escape) {
        dispatch({ type: "SET_MODE", mode: "NORMAL" })
        dispatch({ type: "SET_FILTER_TEXT", value: "" })
        return
      }
      if (key.return) {
        // Apply filter and return to normal mode
        dispatch({ type: "SET_MODE", mode: "NORMAL" })
        return
      }
      if (key.backspace) {
        dispatch({ type: "SET_FILTER_TEXT", value: filterText.slice(0, -1) })
        return
      }
      if (input.length === 1) {
        dispatch({ type: "SET_FILTER_TEXT", value: filterText + input })
      }
      return
    }

    // ── MODE: NORMAL ──

    // Handle two-key sequences (gg, dd)
    if (!key.shift && !key.ctrl && !key.meta && (input === "g" || input === "d")) {
      if (pendingSeq.current?.key === input) {
        clearTimeout(pendingSeq.current.timer)
        pendingSeq.current = null
        if (input === "g") {
          dispatch({ type: "SET_LIST_INDEX", index: 0 })
        } else if (input === "d" && selectedSubId !== null) {
          dispatch({ type: "SET_EDIT_ID", id: selectedSubId })
          dispatch({ type: "SET_SCREEN", screen: "delete" })
        }
        return
      }
      clearTimeout(pendingSeq.current?.timer)
      pendingSeq.current = {
        key: input,
        timer: setTimeout(() => {
          pendingSeq.current = null
        }, 400),
      }
      return
    }

    // Clear pending sequence on other keys
    if (pendingSeq.current) {
      clearTimeout(pendingSeq.current.timer)
      pendingSeq.current = null
    }

    // Arrow keys
    if (key.upArrow) { moveUp(focus, state, dispatch); return }
    if (key.downArrow) { moveDown(focus, state, dispatch); return }
    if (key.pageUp) { pageUp(state, dispatch); return }
    if (key.pageDown) { pageDown(state, dispatch); return }
    if (key.home) { dispatch({ type: "SET_LIST_INDEX", index: 0 }); return }
    if (key.end) { dispatch({ type: "SET_LIST_INDEX", index: getMaxIndex() }); return }

    // Vim navigation
    if (input === "k" && !key.ctrl) { moveUp(focus, state, dispatch); return }
    if (input === "j" && !key.ctrl) { moveDown(focus, state, dispatch); return }
    if (input === "h") { dispatch({ type: "SET_FOCUS", focus: "sidebar" }); return }
    if (input === "l") { dispatch({ type: "SET_FOCUS", focus: "content" }); return }

    // G (Shift+g) — go to bottom
    if (input === "G" || (input === "g" && key.shift)) {
      dispatch({ type: "SET_LIST_INDEX", index: getMaxIndex() })
      return
    }

    // Enter — select
    if (key.return) {
      if (focus === "sidebar") {
        const item = SIDEBAR_ITEMS[sidebarIndex]
        if (item) {
          dispatch({ type: "SET_SCREEN", screen: item.screen })
          dispatch({ type: "SET_LIST_INDEX", index: 0 })
        }
      }
      return
    }

    // Escape — back
    if (key.escape) {
      if (focus === "sidebar") {
        dispatch({ type: "SET_FOCUS", focus: "content" })
      }
      return
    }

    // Tab — toggle focus
    if (key.tab) {
      dispatch({ type: "TOGGLE_FOCUS" })
      return
    }

    // q — quit (with confirmation)
    if (input === "q" && !key.ctrl) {
      dispatch({ type: "SET_CONFIRM_QUIT", value: true })
      return
    }

    // a — add
    if (input === "a") {
      dispatch({ type: "SET_SCREEN", screen: "add" })
      dispatch({ type: "SET_LIST_INDEX", index: 0 })
      return
    }

    // e — edit selected subscription
    if (input === "e" && selectedSubId !== null) {
      dispatch({ type: "SET_EDIT_ID", id: selectedSubId })
      dispatch({ type: "SET_SCREEN", screen: "edit" })
      return
    }

    // r — refresh
    if (input === "r") {
      // Force re-render by toggling through a dummy state
      dispatch({ type: "SET_LIST_INDEX", index: state.listIndex })
      return
    }

    // n / N — next/prev search result (placeholder)
    if (input === "n" && !key.shift) {
      dispatch({ type: "SET_LIST_INDEX", index: Math.min(state.listIndex + 1, getMaxIndex()) })
      return
    }
    if (input === "N" || (input === "n" && key.shift)) {
      dispatch({ type: "SET_LIST_INDEX", index: Math.max(state.listIndex - 1, 0) })
      return
    }

    // ? — help
    if (input === "?") {
      const helpItem = SIDEBAR_ITEMS.find((i) => i.screen === "help")
      if (helpItem) {
        dispatch({ type: "SET_SCREEN", screen: "help" })
      }
      return
    }

    // Ctrl+d / Ctrl+u
    if (key.ctrl && input === "d") {
      pageDown(state, dispatch)
      return
    }
    if (key.ctrl && input === "u") {
      pageUp(state, dispatch)
      return
    }

    // : — enter command mode
    if (input === ":") {
      dispatch({ type: "SET_MODE", mode: "COMMAND" })
      dispatch({ type: "SET_FILTER_TEXT", value: "" })
      return
    }

    // / — enter search mode
    if (input === "/") {
      dispatch({ type: "SET_MODE", mode: "SEARCH" })
      dispatch({ type: "SET_FILTER_TEXT", value: "" })
      return
    }
  })

  return null
}

// ── Helper functions ─────────────────────────────────

function getMaxIndex(): number {
  return 999999 // Will be clamped by screen
}

function moveUp(focus: string, state: any, dispatch: any) {
  if (focus === "sidebar") {
    const newIdx = Math.max(0, state.sidebarIndex - 1)
    dispatch({ type: "SET_SIDEBAR_INDEX", index: newIdx })
  } else {
    dispatch({ type: "SET_LIST_INDEX", index: Math.max(0, state.listIndex - 1) })
  }
}

function moveDown(focus: string, state: any, dispatch: any) {
  if (focus === "sidebar") {
    const newIdx = Math.min(SIDEBAR_ITEMS.length - 1, state.sidebarIndex + 1)
    dispatch({ type: "SET_SIDEBAR_INDEX", index: newIdx })
  } else {
    dispatch({ type: "SET_LIST_INDEX", index: state.listIndex + 1 })
  }
}

function pageUp(state: any, dispatch: any) {
  const jump = Math.max(1, Math.floor((process.stdout.rows ?? 24) / 2))
  dispatch({ type: "SET_LIST_INDEX", index: Math.max(0, state.listIndex - jump) })
}

function pageDown(state: any, dispatch: any) {
  const jump = Math.max(1, Math.floor((process.stdout.rows ?? 24) / 2))
  dispatch({ type: "SET_LIST_INDEX", index: state.listIndex + jump })
}

function executeCommand(
  cmd: string,
  state: any,
  dispatch: any,
  exit: (error?: Error | unknown) => void,
) {
  const trimmed = cmd.trim()

  if (trimmed === "q" || trimmed === "q!") {
    exit()
    return
  }

  if (trimmed === "help") {
    const helpItem = SIDEBAR_ITEMS.find((i) => i.screen === "help")
    if (helpItem) dispatch({ type: "SET_SCREEN", screen: "help" })
    dispatch({ type: "SET_MODE", mode: "NORMAL" })
    return
  }

  if (trimmed.startsWith("sort ")) {
    const field = trimmed.slice(5).trim()
    if (["name", "price", "currency", "cycle", "status"].includes(field)) {
      // TODO: pass sort field to list screen
      dispatch({ type: "SET_MODE", mode: "NORMAL" })
      return
    }
  }

  if (trimmed.startsWith("filter ")) {
    const query = trimmed.slice(7).trim()
    dispatch({ type: "SET_FILTER_TEXT", value: query })
    dispatch({ type: "SET_MODE", mode: "NORMAL" })
    return
  }

  // Unknown command — just return to normal mode
  dispatch({ type: "SET_MODE", mode: "NORMAL" })
}

// ── Root App ─────────────────────────────────────────

export function App() {
  return (
    <AppProvider>
      <AppInner />
    </AppProvider>
  )
}

function AppInner() {
  const { state } = useTui()

  return (
    <Box flexDirection="column" height="100%">
      <KeyboardHandler />

      {/* Status bar */}
      <StatusBar />

      {/* Main content: sidebar + screen */}
      <Box flexGrow={1} flexDirection="row" height="100%">
        <Sidebar />
        <Box flexGrow={1} borderStyle="round" borderColor="gray" height="100%">
          <CurrentScreen />
        </Box>
      </Box>

      {/* Command bar */}
      <CommandBar />
    </Box>
  )
}
