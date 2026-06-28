import { Box, Text, useInput, useApp } from "ink"
import type { AppState, AppAction } from "./context/app-context.tsx"
import { AppProvider, useTui } from "./context/app-context.tsx"
import { Sidebar } from "./components/sidebar.tsx"
import { StatusBar } from "./components/status-bar.tsx"
import { CommandBar } from "./components/command-bar.tsx"
import { Toast } from "./components/toast.tsx"
import { CommandPalette } from "./components/command-palette.tsx"
import { ListScreen } from "./screens/list.tsx"
import { AddScreen } from "./screens/add.tsx"
import { EditScreen } from "./screens/edit.tsx"
import { DeleteScreen } from "./screens/delete.tsx"
import { DetailScreen } from "./screens/detail.tsx"
import { ReportsScreen } from "./screens/reports.tsx"
import { ConfigScreen } from "./screens/config.tsx"
import { ToolsScreen } from "./screens/tools.tsx"
import { HelpScreen } from "./screens/help.tsx"
import { SIDEBAR_ITEMS, REPORT_TABS, TOOLS_TABS } from "./types.ts"
import { getSubscription, updateSubscription } from "../db.ts"
import type { Status } from "../types.ts"

// ── Screen router ─────────────────────────────────────

function CurrentScreen() {
  const { state } = useTui()

  switch (state.screen) {
    case "list":
      return <ListScreen />
    case "add":
      return <AddScreen />
    case "edit":
      return <EditScreen />
    case "delete":
      return <DeleteScreen />
    case "detail":
      return <DetailScreen />
    case "reports":
      return <ReportsScreen />
    case "config":
      return <ConfigScreen />
    case "tools":
      return <ToolsScreen />
    case "help":
      return <HelpScreen />
    default:
      return (
        <Box flexGrow={1} alignItems="center" justifyContent="center">
          <Text dimColor>Unknown screen</Text>
        </Box>
      )
  }
}

// ── Keyboard handler ──────────────────────────────────

function KeyboardHandler() {
  const { state, dispatch } = useTui()
  const { exit } = useApp()

  useInput(
    (input: string, key) => {
      // ── COMMAND mode ──
      if (state.mode === "COMMAND") {
        if (key.escape) {
          dispatch({
            type: "SET_FILTER_TEXT",
            value: state.filterText.startsWith("/") ? "" : state.filterText,
          })
          dispatch({ type: "SET_MODE", mode: "NORMAL" })
          return
        }
        if (key.return) {
          executeCommand(state.filterText, state, dispatch, exit)
          return
        }
        if (key.backspace) {
          dispatch({
            type: "SET_FILTER_TEXT",
            value: state.filterText.slice(0, -1),
          })
          return
        }
        if (input.length === 1) {
          dispatch({
            type: "SET_FILTER_TEXT",
            value: state.filterText + input,
          })
        }
        return
      }

      // ── NORMAL mode ──

      // Ctrl+P — command palette
      if (key.ctrl && input === "p") {
        dispatch({ type: "SET_PALETTE_OPEN", open: true })
        return
      }

      // : — enter command mode
      if (input === ":") {
        dispatch({ type: "SET_MODE", mode: "COMMAND" })
        dispatch({ type: "SET_FILTER_TEXT", value: "" })
        return
      }

      // q — quit on list, go back on other screens
      if (input === "q") {
        if (state.screen === "list") {
          exit()
          return
        }
        if (state.history.length > 0) {
          dispatch({ type: "GO_BACK" })
          return
        }
        exit()
        return
      }

      // Esc — go back
      if (key.escape) {
        if (state.focus === "sidebar") {
          dispatch({ type: "SET_FOCUS", focus: "content" })
        } else if (state.history.length > 0) {
          dispatch({ type: "GO_BACK" })
        } else if (state.screen !== "list") {
          dispatch({ type: "SET_SCREEN", screen: "list" })
          dispatch({ type: "SET_LIST_INDEX", index: 0 })
        }
        return
      }

      // Tab — toggle focus
      if (key.tab) {
        dispatch({ type: "TOGGLE_FOCUS" })
        return
      }

      // Ctrl+l — clear filter (vim-like)
      if (key.ctrl && input === "l") {
        if (state.filterText) {
          dispatch({ type: "SET_FILTER_TEXT", value: "" })
        }
        return
      }

      // Arrow / vim navigation for list/config screens (content focus only)
      if (state.focus === "content" && (state.screen === "list" || state.screen === "config")) {
        if (key.upArrow || input === "k") {
          dispatch({ type: "SET_LIST_INDEX", index: Math.max(0, state.listIndex - 1) })
          return
        }
        if (key.downArrow || input === "j") {
          dispatch({ type: "SET_LIST_INDEX", index: state.listIndex + 1 })
          return
        }
        if (key.pageUp || (key.ctrl && input === "u")) {
          const jump = Math.max(1, Math.floor((process.stdout.rows ?? 24) / 2))
          dispatch({ type: "SET_LIST_INDEX", index: Math.max(0, state.listIndex - jump) })
          return
        }
        if (key.pageDown || (key.ctrl && input === "d")) {
          const jump = Math.max(1, Math.floor((process.stdout.rows ?? 24) / 2))
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
      }

      // Sidebar navigation
      if (key.upArrow || input === "k") {
        if (state.focus === "sidebar") {
          dispatch({ type: "SET_SIDEBAR_INDEX", index: Math.max(0, state.sidebarIndex - 1) })
          return
        }
      }
      if (key.downArrow || input === "j") {
        if (state.focus === "sidebar") {
          dispatch({
            type: "SET_SIDEBAR_INDEX",
            index: Math.min(SIDEBAR_ITEMS.length - 1, state.sidebarIndex + 1),
          })
          return
        }
      }

      // 1-6 — sidebar shortcuts (not on config, which uses numbers for editing)
      if (/^[1-6]$/.test(input) && state.screen !== "config") {
        const idx = Number(input) - 1
        const item = SIDEBAR_ITEMS[idx]
        if (item) {
          dispatch({ type: "SET_SCREEN", screen: item.screen })
          dispatch({ type: "SET_LIST_INDEX", index: 0 })
        }
        return
      }

      // ? — help screen (global, any screen)
      if (input === "?") {
        dispatch({ type: "SET_SCREEN", screen: "help" })
        return
      }

      // h/l — focus navigation (not on tabbed screens)
      if (input === "h" && state.screen !== "reports" && state.screen !== "tools") {
        dispatch({ type: "SET_FOCUS", focus: "sidebar" })
        return
      }
      if (input === "l" && state.screen !== "reports" && state.screen !== "tools") {
        dispatch({ type: "SET_FOCUS", focus: "content" })
        return
      }

      // Enter — select sidebar item
      if (key.return && state.focus === "sidebar") {
        const item = SIDEBAR_ITEMS[state.sidebarIndex]
        if (item) {
          dispatch({ type: "SET_SCREEN", screen: item.screen })
          dispatch({ type: "SET_LIST_INDEX", index: 0 })
        }
        return
      }

      // ── Screen-specific shortcuts (only when focus is on content) ──
      if (state.focus === "content") {
        switch (state.screen) {
          case "list": {
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
            break
          }
          case "reports": {
            if (key.leftArrow || input === "h") {
              const idx = REPORT_TABS.indexOf(state.reportsTab)
              const prev = REPORT_TABS[(idx - 1 + REPORT_TABS.length) % REPORT_TABS.length]
              dispatch({ type: "SET_REPORTS_TAB", tab: prev })
              return
            }
            if (key.rightArrow || input === "l") {
              const idx = REPORT_TABS.indexOf(state.reportsTab)
              const next = REPORT_TABS[(idx + 1) % REPORT_TABS.length]
              dispatch({ type: "SET_REPORTS_TAB", tab: next })
              return
            }
            break
          }
          case "tools": {
            if (key.leftArrow || input === "h") {
              const idx = TOOLS_TABS.indexOf(state.toolsTab)
              const prev = TOOLS_TABS[(idx - 1 + TOOLS_TABS.length) % TOOLS_TABS.length]
              dispatch({ type: "SET_TOOLS_TAB", tab: prev })
              return
            }
            if (key.rightArrow || input === "l") {
              const idx = TOOLS_TABS.indexOf(state.toolsTab)
              const next = TOOLS_TABS[(idx + 1) % TOOLS_TABS.length]
              dispatch({ type: "SET_TOOLS_TAB", tab: next })
              return
            }
            break
          }
        }
      }
    },
    { isActive: !state.formActive && !state.paletteOpen },
  )

  return null
}

// ── Commands ─────────────────────────────────────────

function executeCommand(
  cmd: string,
  state: AppState,
  dispatch: React.Dispatch<AppAction>,
  exit: (error?: Error | unknown) => void,
) {
  const trimmed = cmd.trim()

  if (trimmed.startsWith("/")) {
    const query = trimmed.slice(1)
    dispatch({ type: "SET_FILTER_TEXT", value: query })
    dispatch({ type: "SET_MODE", mode: "NORMAL" })
    return
  }
  if (trimmed === "q" || trimmed === "q!") {
    exit()
    return
  }
  if (trimmed === "help") {
    dispatch({ type: "SET_SCREEN", screen: "help" })
    dispatch({ type: "SET_MODE", mode: "NORMAL" })
    return
  }
  if (trimmed === "clear") {
    dispatch({ type: "SET_FILTER_TEXT", value: "" })
    dispatch({ type: "SET_MODE", mode: "NORMAL" })
    return
  }
  // Unknown command — clear filter text and return to NORMAL
  dispatch({ type: "SET_FILTER_TEXT", value: "" })
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

      <StatusBar />

      <Box flexGrow={1} flexDirection="row">
        <Sidebar />
        <Box flexGrow={1}>
          <CurrentScreen />
        </Box>
      </Box>

      <CommandBar />

      <Toast />

      {state.paletteOpen && <CommandPalette />}
    </Box>
  )
}
