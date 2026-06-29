import { Box, Text } from "ink"
import { useTui } from "../context/app-context.tsx"

const SCREEN_LABEL: Record<string, string> = {
  list: "List",
  add: "Add",
  edit: "Edit",
  delete: "Del",
  detail: "Detail",
  reports: "Reports",
  config: "Config",
  tools: "Tools",
  help: "Help",
}

/**
 * Compact 1-line status bar.
 *   left:  subtrack · MODE · ScreenName
 *   right: filter indicator · multi-select count
 */
export function StatusBar() {
  const { state } = useTui()
  const modeColor = state.mode === "NORMAL" ? "green" : "yellow"
  const modeLabel = state.mode === "NORMAL" ? "NORMAL" : "CMD"

  const screenLabel = SCREEN_LABEL[state.screen] ?? state.screen

  return (
    <Box width="100%" minHeight={1} paddingX={1}>
      {/* Left side — brand + mode + screen */}
      <Box flexGrow={1}>
        <Text bold color="cyan">
          subtrack
        </Text>
        <Text color={modeColor} bold>
          {" · "}{modeLabel}
        </Text>
        <Text bold>
          {" · "}{screenLabel}
        </Text>
      </Box>

      {/* Right side — filter + multi-select */}
      <Box>
        {state.filterText && (
          <Text color="blue">
            /{state.filterText.length > 16
              ? state.filterText.slice(0, 16) + "…"
              : state.filterText}{" "}
          </Text>
        )}
        {state.multiSelect.size > 0 && (
          <Text color="yellow" bold>
            [{state.multiSelect.size}]{" "}
          </Text>
        )}
      </Box>
    </Box>
  )
}
