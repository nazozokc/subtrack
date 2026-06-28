import { Box, Text } from "ink"
import { SIDEBAR_ITEMS } from "../types.ts"

export function HelpScreen() {
  return (
    <Box flexDirection="column" flexGrow={1} paddingX={1}>
      <Box marginBottom={1}>
        <Text bold underline>Help & Keybindings</Text>
      </Box>

      <Box flexDirection="column" gap={1} marginBottom={1}>
        <Text bold>Global Navigation</Text>
        <Text>  j/k or ↑/↓   Move selection in list/sidebar</Text>
        <Text>  h/l           Focus: sidebar/content</Text>
        <Text>  Tab           Toggle focus: sidebar ↔ content</Text>
        <Text>  Enter         Select sidebar item</Text>
        <Text>  Esc           Go back to previous screen</Text>
        <Text>  q             Quit (on list) or go back</Text>
        <Text>  ?             This help screen</Text>
        <Text>  :command      Execute command (:q, :help, /filter)</Text>
      </Box>

      <Box flexDirection="column" gap={1} marginBottom={1}>
        <Text bold>List Screen</Text>
        <Text>  a             Add new subscription</Text>
        <Text>  e             Edit selected subscription</Text>
        <Text>  d             Delete selected subscription</Text>
        <Text>  Enter         View full details</Text>
        <Text>  /             Filter subscriptions</Text>
        <Text>  v             Toggle multi-select</Text>
        <Text>  r             Open Reports</Text>
        <Text>  c             Open Config</Text>
        <Text>  g/G           Go to top/bottom</Text>
        <Text>  s             Cycle sort (name/price/cycle/status/id)</Text>
        <Text>  Ctrl+d/u      Page down/up</Text>
        <Text>  S             Toggle status (active→paused→cancelled)</Text>
        <Text>  R             Refresh data</Text>
      </Box>

      <Box flexDirection="column" gap={1} marginBottom={1}>
        <Text bold>Detail Screen</Text>
        <Text>  e             Edit subscription</Text>
        <Text>  d             Delete subscription</Text>
        <Text>  r             Toggle raw values</Text>
        <Text>  q / Esc       Go back</Text>
      </Box>

      <Box flexDirection="column" gap={1} marginBottom={1}>
        <Text bold>Reports & Tools Screens</Text>
        <Text>  ←/→ or h/l    Switch between tabs</Text>
      </Box>

      <Box flexDirection="column" gap={1} marginBottom={1}>
        <Text bold>Form Screens (Add/Edit)</Text>
        <Text>  Tab/Enter     Next field</Text>
        <Text>  Esc           Cancel</Text>
        <Text>  y/n           Confirm/Cancel</Text>
      </Box>

      <Box flexDirection="column" gap={1} marginBottom={1}>
        <Text bold>Available Screens</Text>
        {SIDEBAR_ITEMS.map((item, i) => (
          <Box key={item.screen}>
            <Box width={4}><Text dimColor>{i + 1}.</Text></Box>
            <Box width={22}>
              <Text bold wrap="truncate-end">
                {item.icon} {item.label}
              </Text>
            </Box>
          </Box>
        ))}
      </Box>

      <Box marginTop={1}>
        <Text dimColor>Press Esc or q to go back</Text>
      </Box>
    </Box>
  )
}
