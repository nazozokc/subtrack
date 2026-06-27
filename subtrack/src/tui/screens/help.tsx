import { Box, Text, useInput } from "ink"
import { useTui } from "../context/app-context.tsx"
import { SIDEBAR_ITEMS } from "../types.ts"

export function HelpScreen() {
  const { dispatch } = useTui()

  useInput((input, key) => {
    if (key.escape || input === "q") dispatch({ type: "SET_SCREEN", screen: "list" })
  })

  return (
    <Box flexDirection="column" flexGrow={1} paddingX={1}>
      <Box marginBottom={1}>
        <Text bold underline>Help & Keybindings</Text>
      </Box>

      <Box flexDirection="column" gap={1} marginBottom={1}>
        <Text bold>Sidebar Navigation</Text>
        <Text>  {Array.from({ length: SIDEBAR_ITEMS.length }, (_, i) => i + 1).join(" ")} {SIDEBAR_ITEMS.length}</Text>
        <Text>  j/k          Move down/up in sidebar or list</Text>
        <Text>  Tab           Focus toggle: sidebar ↔ content</Text>
      </Box>

      <Box flexDirection="column" gap={1} marginBottom={1}>
        <Text bold>List Screen</Text>
        <Text>  a             Add subscription</Text>
        <Text>  e             Edit selected subscription</Text>
        <Text>  dd            Delete selected subscription</Text>
        <Text>  /             Search</Text>
      </Box>

      <Box flexDirection="column" gap={1} marginBottom={1}>
        <Text bold>Global</Text>
        <Text>  :command      Execute command</Text>
        <Text>  gg            Go to top</Text>
        <Text>  G             Go to bottom</Text>
        <Text>  h/l or ←/→    Navigate screens within a section</Text>
        <Text>  q             Quit</Text>
        <Text>  ?             This help screen</Text>
      </Box>

      <Box flexDirection="column" gap={1} marginBottom={1}>
        <Text bold>Available Screens</Text>
        {SIDEBAR_ITEMS.map((item, i) => (
          <Box key={item.id}>
            <Box width={4}><Text dimColor>{i + 1}.</Text></Box>
            <Box width={22}><Text bold wrap="truncate-end">{item.icon} {item.label}</Text></Box>
          </Box>
        ))}
      </Box>

      <Box marginTop={1}>
        <Text dimColor>Press Esc or q to go back</Text>
      </Box>
    </Box>
  )
}
