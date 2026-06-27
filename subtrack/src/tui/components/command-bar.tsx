import { Box, Text } from "ink"
import { useTui } from "../context/app-context.tsx"
import { type Screen } from "../types.ts"

const HINT_TEXT: Record<Screen, string> = {
  list: "j↓ k↑ /filter :cmd ?help q:quit",
  search: "j↓ k↑ Enter:open :cmd ?help q:quit",
  add: "Tab:next Enter:save Esc:cancel",
  edit: "Tab:next Enter:save Esc:cancel",
  delete: "j↓ k↑ Space:toggle Enter:confirm Esc:cancel",
  tags: "j↓ k↑ Enter:filter",
  "tag-manage": "j↓ k↑ Enter:manage",
  trials: "j↓ k↑ Enter:detail a:add",
  "trial-add": "Tab:next Enter:save Esc:cancel",
  "trial-expiring": "j↓ k↑ Enter:detail",
  bulk: "Tab:select Enter:execute",
  summary: "q:quit",
  payment: "q:quit",
  upcoming: "j↓ k↑",
  analytics: "q:quit",
  compare: "q:quit",
  forecast: "q:quit",
  config: "j↓ k↑ e:edit",
  usage: "j↓ k↑ d:delete",
  export: "Enter:export",
  import: "Enter:import",
  backup: "Enter:backup",
  restore: "j↓ k↑ Enter:restore",
  help: "q:quit",
}

export function CommandBar() {
  const { state, dispatch } = useTui()

  if (state.confirmQuit) {
    return (
      <Box width="100%" borderStyle="single" borderColor="yellow">
        <Text bold color="yellow">
          {" "}Quit subtrack TUI?{" "}
        </Text>
        <Text bold color="green">
          y
        </Text>
        <Text>
          /N{" "}
        </Text>
      </Box>
    )
  }

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

  if (state.mode === "SEARCH") {
    return (
      <Box width="100%" borderStyle="single" borderColor="blue">
        <Text bold color="blue">
          /{state.filterText}
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
