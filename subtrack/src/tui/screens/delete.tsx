import { Box, Text, useInput } from "ink"
import { useMemo } from "react"
import { getSubscription, deleteSubscription } from "../../db.ts"
import { useTui } from "../context/app-context.tsx"

export function DeleteScreen() {
  const { state, dispatch } = useTui()

  const sub = useMemo(
    () => (state.editId ? getSubscription(state.editId) : undefined),
    [state.editId],
  )

  useInput((input) => {
    if (input === "y" || input === "Y") {
      if (sub) {
        deleteSubscription(sub.id)
      }
      dispatch({ type: "SET_EDIT_ID", id: null })
      dispatch({ type: "SET_SCREEN", screen: "list" })
    } else if (input === "n" || input === "N") {
      dispatch({ type: "SET_EDIT_ID", id: null })
      dispatch({ type: "SET_SCREEN", screen: "list" })
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
          <Text bold>{sub.price} {sub.currency}</Text>
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
