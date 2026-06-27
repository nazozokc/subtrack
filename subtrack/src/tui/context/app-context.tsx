import {
  createContext,
  useContext,
  useReducer,
  useCallback,
  type ReactNode,
} from "react"
import type { Screen, Mode, Focus } from "../types.ts"

export type AppState = {
  screen: Screen
  mode: Mode
  focus: Focus
  sidebarIndex: number
  listIndex: number
  filterText: string
  confirmQuit: boolean
  editId: number | null
  selectedSubId: number | null
}

export type AppAction =
  | { type: "SET_SCREEN"; screen: Screen }
  | { type: "SET_MODE"; mode: Mode }
  | { type: "SET_FOCUS"; focus: Focus }
  | { type: "SET_SIDEBAR_INDEX"; index: number }
  | { type: "SET_LIST_INDEX"; index: number }
  | { type: "SET_FILTER_TEXT"; value: string }
  | { type: "SET_CONFIRM_QUIT"; value: boolean }
  | { type: "SET_EDIT_ID"; id: number | null }
  | { type: "SET_SELECTED_SUB_ID"; id: number | null }
  | { type: "TOGGLE_FOCUS" }

const initialState: AppState = {
  screen: "list",
  mode: "NORMAL",
  focus: "content",
  sidebarIndex: 0,
  listIndex: 0,
  filterText: "",
  confirmQuit: false,
  editId: null,
  selectedSubId: null,
}

function appReducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case "SET_SCREEN":
      return { ...state, screen: action.screen, mode: "NORMAL", focus: "content", confirmQuit: false }
    case "SET_MODE":
      return { ...state, mode: action.mode, confirmQuit: false }
    case "SET_FOCUS":
      return { ...state, focus: action.focus }
    case "SET_SIDEBAR_INDEX":
      return { ...state, sidebarIndex: action.index }
    case "SET_LIST_INDEX":
      return { ...state, listIndex: action.index }
    case "SET_FILTER_TEXT":
      return { ...state, filterText: action.value }
    case "SET_CONFIRM_QUIT":
      return { ...state, confirmQuit: action.value }
    case "SET_EDIT_ID":
      return { ...state, editId: action.id }
    case "SET_SELECTED_SUB_ID":
      return { ...state, selectedSubId: action.id }
    case "TOGGLE_FOCUS":
      return { ...state, focus: state.focus === "sidebar" ? "content" : "sidebar" }
    default:
      return state
  }
}

type AppContextValue = {
  state: AppState
  dispatch: React.Dispatch<AppAction>
}

const AppContext = createContext<AppContextValue | null>(null)

export function AppProvider({ children }: { children: ReactNode }) {
  const [state, dispatch] = useReducer(appReducer, initialState)

  return (
    <AppContext.Provider value={{ state, dispatch }}>
      {children}
    </AppContext.Provider>
  )
}

export function useTui(): AppContextValue {
  const ctx = useContext(AppContext)
  if (!ctx) throw new Error("useTui must be used within <AppProvider>")
  return ctx
}

// ── Convenience dispatchers ──────────────────────────

export function useSetScreen() {
  const { dispatch } = useTui()
  return useCallback(
    (screen: Screen) => dispatch({ type: "SET_SCREEN", screen }),
    [dispatch],
  )
}

export function useSetMode() {
  const { dispatch } = useTui()
  return useCallback(
    (mode: Mode) => dispatch({ type: "SET_MODE", mode }),
    [dispatch],
  )
}

export function useSetFilterText() {
  const { dispatch } = useTui()
  return useCallback(
    (value: string) => dispatch({ type: "SET_FILTER_TEXT", value }),
    [dispatch],
  )
}
