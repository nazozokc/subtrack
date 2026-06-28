import { Box, Text } from "ink"
import { useTui } from "../context/app-context.tsx"
import { SCREEN_TITLES } from "../types.ts"

export function StatusBar() {
  const { state } = useTui()
  const modeColor = state.mode === "NORMAL" ? "green" : "yellow"

  return (
    <Box width="100%" borderStyle="single" borderColor="gray" minHeight={1}>
      <Box paddingLeft={1} flexGrow={1}>
        <Text bold color="cyan">subtrack</Text>
        <Text dimColor> TUI</Text>
      </Box>

      <Box>
        <Text dimColor> {SCREEN_TITLES[state.screen]} </Text>
      </Box>

      {state.filterText && (
        <Box>
          <Text color="blue">
            {" "}&gt; {state.filterText.length > 20
              ? state.filterText.slice(0, 20) + "…"
              : state.filterText}{" "}
          </Text>
        </Box>
      )}

      {state.multiSelect.size > 0 && (
        <Box>
          <Text color="yellow" bold>
            {" "}[{state.multiSelect.size}]{" "}
          </Text>
        </Box>
      )}

      <Box paddingRight={1}>
        <Text color={modeColor} bold inverse>
          {" "}{state.mode === "NORMAL" ? " NORMAL " : " CMD "}{" "}
        </Text>
      </Box>
    </Box>
  )
}
