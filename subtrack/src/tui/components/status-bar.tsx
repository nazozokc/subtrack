import { Box, Text } from "ink"
import { useTui } from "../context/app-context.tsx"
import { SCREEN_TITLES } from "../types.ts"

export function StatusBar() {
  const { state } = useTui()
  const modeColor = state.mode === "NORMAL" ? "green" : state.mode === "COMMAND" ? "yellow" : "blue"

  return (
    <Box width="100%" borderStyle="single" borderColor="gray">
      <Box flexGrow={1}>
        <Text bold> subtrack</Text>
        <Text dimColor> TUI</Text>
      </Box>
      <Box>
        <Text dimColor> {SCREEN_TITLES[state.screen]} </Text>
      </Box>
      <Box>
        <Text color={modeColor} bold>
          {" "}{state.mode}{" "}
        </Text>
      </Box>
    </Box>
  )
}
