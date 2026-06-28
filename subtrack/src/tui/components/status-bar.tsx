import { Box, Text } from "ink"
import Gradient from "ink-gradient"
import { useTui } from "../context/app-context.tsx"
import type { Screen } from "../types.ts"

const SCREEN_LABELS: Record<Screen, string> = {
  list: "List",
  add: "Add",
  edit: "Edit",
  delete: "Delete",
  detail: "Detail",
  reports: "Reports",
  config: "Config",
  tools: "Tools",
  help: "Help",
}

function Breadcrumb({ history, current }: { history: Screen[]; current: Screen }) {
  // Build breadcrumb trail: unique screens in order, last 3 max + current
  const trail: string[] = []
  const seen = new Set<string>()
  for (const s of history) {
    const label = SCREEN_LABELS[s]
    if (!seen.has(label) && label !== SCREEN_LABELS[current]) {
      trail.push(label)
      seen.add(label)
    }
  }
  // Keep only last 3
  const displayTrail = trail.slice(-3)

  return (
    <Text dimColor>
      {" "}
      {displayTrail.map((l) => (
        <Text key={l} dimColor>
          {l} <Text bold color="gray">›</Text>{" "}
        </Text>
      ))}
      <Text bold color="white">{SCREEN_LABELS[current]}</Text>{" "}
    </Text>
  )
}

export function StatusBar() {
  const { state } = useTui()
  const modeColor = state.mode === "NORMAL" ? "green" : "yellow"

  return (
    <Box width="100%" borderStyle="single" borderColor="gray" minHeight={1}>
      <Box paddingLeft={1} flexGrow={1}>
        <Gradient name="cristal"><Text bold>subtrack</Text></Gradient>
        <Text dimColor> TUI</Text>
      </Box>

      <Box>
        <Breadcrumb history={state.history} current={state.screen} />
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
