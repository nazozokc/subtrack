import { Box, Text, useInput } from "ink"
import { useMemo, useState, useEffect } from "react"
import { getSubscription, deleteSubscription } from "../../db.ts"
import { useTui, useSetFormActive } from "../context/app-context.tsx"
import { formatPrice } from "../../price.ts"

export function DeleteScreen() {
  const { state, dispatch } = useTui()
  const [error, setError] = useState<string | null>(null)
  const setFormActive = useSetFormActive()

  // Prevent global key handler from firing alongside our own
  useEffect(() => {
    setFormActive(true)
    return () => setFormActive(false)
  }, [setFormActive])

  const sub = useMemo(
    () => (state.selectedId ? getSubscription(state.selectedId) : undefined),
    [state.selectedId],
  )

  useInput((input, key) => {
    if (key.escape && !(input === "y" || input === "Y" || input === "n" || input === "N")) {
      dispatch({ type: "GO_BACK" })
      dispatch({ type: "SET_SELECTED_ID", id: null })
      return
    }
    if (input === "y" || input === "Y") {
      if (sub) {
        try {
          deleteSubscription(sub.id)
        } catch (e: unknown) {
          setError(e instanceof Error ? e.message : String(e))
          return
        }
      }
      dispatch({ type: "GO_BACK" })
      dispatch({ type: "SET_SELECTED_ID", id: null })
    } else if (input === "n" || input === "N") {
      dispatch({ type: "GO_BACK" })
      dispatch({ type: "SET_SELECTED_ID", id: null })
    }
  })

  if (!sub) {
    return (
      <Box flexGrow={1} alignItems="center" justifyContent="center" flexDirection="column">
        <Text dimColor>Subscription not found</Text>
      </Box>
    )
  }

  return (
    <Box flexGrow={1} alignItems="center" justifyContent="center" flexDirection="column">
      {error && <Text color="red">{error}</Text>}
      <Box marginBottom={1}>
        <Text bold color="red">
          Delete Subscription
        </Text>
      </Box>

      <Box flexDirection="column" gap={1} marginBottom={1}>
        <Box>
          <Box width={14}><Text dimColor>Name:</Text></Box>
          <Text bold>{sub.name}</Text>
        </Box>
        <Box>
          <Box width={14}><Text dimColor>Price:</Text></Box>
          <Text bold>{formatPrice(sub.price, sub.currency)}</Text>
        </Box>
        <Box>
          <Box width={14}><Text dimColor>Cycle:</Text></Box>
          <Text bold>{sub.cycle}</Text>
        </Box>
        <Box>
          <Box width={14}><Text dimColor>Status:</Text></Box>
          <Text bold>{sub.status}</Text>
        </Box>
      </Box>

      <Box marginTop={1}>
        <Text>
          Are you sure?{" "}
          <Text bold color="green" inverse> y </Text>
          {" "}to delete /{" "}
          <Text bold color="red" inverse> n </Text>
          {" "}to cancel
        </Text>
      </Box>
    </Box>
  )
}
