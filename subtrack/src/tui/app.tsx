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

/**
 * Layout structure (top to bottom) — uses every terminal row:
 *
 * 1  StatusLine                   ← flexShrink=0, 1 row
 * 2+ Sidebar | Content            ← flexGrow=1, fills all remaining rows
 *    (fixed width) (fills rest)
 * N  CommandBar                   ← flexShrink=0, 1 row
 *
 * Overlays (absolute positioned, on top):
 *   - Toast:  bottom-center
 *   - Palette: full-screen overlay
 */
function AppInner() {
  const { state } = useTui()

  return (
    <Box flexDirection="column" height="100%" position="relative">
      <KeyboardHandler />

      {/* ── 1-row compact header ── */}
      <Box flexShrink={0}>
        <StatusBar />
      </Box>

      {/* ── Main area — fills EVERYTHING between header & footer ── */}
      <Box
        flexGrow={1}
        flexDirection="row"
        minHeight={0}
        height="100%"
      >
        <Sidebar />
        <Box flexGrow={1} minWidth={0}>
          <CurrentScreen />
        </Box>
      </Box>

      {/* ── 1-row footer ── */}
      <Box flexShrink={0}>
        <CommandBar />
      </Box>

      {/* ── Overlays ── */}
      <Toast />
      {state.paletteOpen && <CommandPalette />}
    </Box>
  )
}
