import { Box } from "ink"
import { AppProvider, useTui } from "./context/app-context.tsx"
import { KeyboardHandler } from "./keyboard.tsx"
import { CurrentScreen } from "./screen-router.tsx"
import { Sidebar } from "./components/sidebar.tsx"
import { StatusBar } from "./components/status-bar.tsx"
import { CommandBar } from "./components/command-bar.tsx"
import { Toast } from "./components/toast.tsx"
import { CommandPalette } from "./components/command-palette.tsx"

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
