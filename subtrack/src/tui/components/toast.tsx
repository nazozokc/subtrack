import { Box, Text } from "ink"
import { useEffect, useRef } from "react"
import { useTui } from "../context/app-context.tsx"

const TOAST_COLORS: Record<string, string> = {
  success: "green",
  error: "red",
  info: "blue",
}

const TOAST_ICONS: Record<string, string> = {
  success: "✓",
  error: "✗",
  info: "ℹ",
}

export function Toast() {
  const { state, dispatch } = useTui()
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // Clear toast on screen change (only if not on list)
  // Intentionally only re-runs on screen change, not when toast is set
  useEffect(() => {
    if (state.toast && state.screen !== "list") {
      dispatch({ type: "CLEAR_TOAST" })
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state.screen, dispatch])

  // Auto-dismiss toast after 2s
  useEffect(() => {
    if (state.toast) {
      if (timerRef.current) clearTimeout(timerRef.current)
      timerRef.current = setTimeout(() => {
        dispatch({ type: "CLEAR_TOAST" })
      }, 2000)
    }
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current)
    }
  }, [state.toast, dispatch])

  if (!state.toast) return null

  const color = TOAST_COLORS[state.toast.type] ?? "white"
  const icon = TOAST_ICONS[state.toast.type] ?? "•"

  return (
    <Box position="absolute" bottom={1} width="100%" justifyContent="center">
      <Box
        borderStyle="round"
        borderColor={color}
        paddingX={1}
        minHeight={1}
      >
        <Text color={color} bold>
          {" "}{icon} {state.toast.message}{" "}
        </Text>
      </Box>
    </Box>
  )
}
