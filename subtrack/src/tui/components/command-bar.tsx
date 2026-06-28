import { Box, Text } from "ink"
import { useTui } from "../context/app-context.tsx"
import type { Screen } from "../types.ts"

const HINT_TEXT: Record<Screen, string> = {
  list: "j/k  /  Enter:detail  a:add  e:edit  d:del  s:sort  S:status  v:sel  r:rpt  c:cfg  R:ref  ?:help  q:quit",
  add: "Enter:next  Esc:cancel",
  edit: "Enter:next  Esc:cancel",
  delete: "y:delete  n:cancel",
  detail: "e:edit  d:delete  r:raw  q/ Esc:back",
  reports: "← →:tab  h/l:tab  Esc:back",
  config: "1-9:edit  Enter:save  Esc:back",
  tools: "← →:tab  h/l:tab  Esc:back",
  help: "Esc/q:back",
}

export function CommandBar() {
  const { state } = useTui()

  if (state.mode === "COMMAND") {
    const cmdText = state.filterText || ""
    return (
      <Box width="100%" borderStyle="single" borderColor="yellow" minHeight={1}>
        <Text bold color="yellow">
          :{cmdText}
        </Text>
        <Text bold color="yellow">
          ▎
        </Text>
      </Box>
    )
  }

  return (
    <Box width="100%" borderStyle="single" borderColor="gray" minHeight={1}>
      <Text dimColor>
        {" "}{HINT_TEXT[state.screen]}{" "}
      </Text>
    </Box>
  )
}
