import { Box, Text } from "ink"
import { useTui } from "../context/app-context.tsx"
import type { Screen } from "../types.ts"

const HINT_TEXT: Record<Screen, string> = {
  list: "j↓ k↑  /filter  a:add  e:edit  d:delete  v:select  r:reports  q:quit  ?help",
  add: "Tab/Enter:next  Enter:save  Esc:cancel",
  edit: "Tab/Enter:next  Enter:save  Esc:cancel",
  delete: "y:delete  n:cancel",
  reports: "← →:tab  h/l:tab  Esc:back",
  config: "1-9:edit  Esc:back",
  tools: "← →:tab  h/l:tab  Esc:back",
  help: "Esc/q:back",
}

export function CommandBar() {
  const { state } = useTui()

  if (state.mode === "COMMAND") {
    return (
      <Box width="100%" borderStyle="single" borderColor="yellow">
        <Text bold color="yellow">
          :{state.filterText}
        </Text>
        <Text dimColor>█</Text>
      </Box>
    )
  }

  return (
    <Box width="100%" borderStyle="single" borderColor="gray">
      <Text dimColor>
        {" "}{HINT_TEXT[state.screen]}{" "}
      </Text>
    </Box>
  )
}
