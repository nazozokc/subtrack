import { Box, Text } from "ink"
import { useTui } from "../context/app-context.tsx"
import type { Screen } from "../types.ts"

type HintGroup = {
  label: string
  color: string
  hints: string
}

const HINT_GROUPS: Record<Screen, HintGroup[]> = {
  list: [
    { label: "Nav", color: "cyan", hints: "j/k · g/G · /" },
    { label: "Action", color: "green", hints: "a:add · e:edit · d:del · Enter:detail" },
    { label: "Data", color: "yellow", hints: "s:sort · S:status · v:sel" },
    { label: "System", color: "blue", hints: "r:rpt · c:cfg · R:ref · ?:help · q:quit" },
  ],
  add: [
    { label: "Form", color: "green", hints: "Enter:next · Esc:cancel" },
  ],
  edit: [
    { label: "Form", color: "green", hints: "Enter:next · Esc:cancel" },
  ],
  delete: [
    { label: "Confirm", color: "red", hints: "y:delete · n:cancel" },
  ],
  detail: [
    { label: "Action", color: "green", hints: "e:edit · d:delete" },
    { label: "View", color: "cyan", hints: "r:raw" },
    { label: "Back", color: "gray", hints: "q/Esc:back" },
  ],
  reports: [
    { label: "Tab", color: "cyan", hints: "← → · h/l" },
    { label: "Back", color: "gray", hints: "Esc:back" },
  ],
  config: [
    { label: "Edit", color: "green", hints: "1-9:edit · Enter:save" },
    { label: "Back", color: "gray", hints: "Esc:back" },
  ],
  tools: [
    { label: "Tab", color: "cyan", hints: "← → · h/l" },
    { label: "Back", color: "gray", hints: "Esc:back" },
  ],
  help: [
    { label: "Back", color: "gray", hints: "Esc/q:back" },
  ],
}

export function CommandBar() {
  const { state } = useTui()

  if (state.mode === "COMMAND") {
    const cmdText = state.filterText || ""
    return (
      <Box width="100%" minHeight={1}>
        <Text bold color="yellow">
          :{cmdText}
        </Text>
        <Text bold color="yellow">
          ▎
        </Text>
      </Box>
    )
  }

  const groups = HINT_GROUPS[state.screen]

  return (
    <Box width="100%" minHeight={1}>
      <Box paddingLeft={1} gap={1}>
        {groups.map((g) => (
          <Box key={g.label}>
            <Text color={g.color} bold>
              {g.label}
            </Text>
            <Text dimColor>
              : {g.hints}
            </Text>
          </Box>
        ))}
      </Box>
    </Box>
  )
}
