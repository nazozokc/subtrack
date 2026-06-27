import { Box, Text } from "ink"
import { SubscriptionForm } from "./subscription-form.tsx"
import { writeSubscription } from "../../db.ts"
import { useTui } from "../context/app-context.tsx"
import { useState } from "react"
import type { AddSharedArgs } from "../../types.ts"

export function AddScreen() {
  const { dispatch } = useTui()
  const [error, setError] = useState<string | null>(null)

  const handleSave = (data: AddSharedArgs) => {
    try {
      writeSubscription(data)
      dispatch({ type: "SET_SCREEN", screen: "list" })
      dispatch({ type: "SET_LIST_INDEX", index: 0 })
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e))
    }
  }

  const handleCancel = () => {
    dispatch({ type: "SET_SCREEN", screen: "list" })
  }

  return (
    <Box flexDirection="column" flexGrow={1}>
      {error && <Box paddingX={1}><Text color="red">{error}</Text></Box>}
      <SubscriptionForm
        title="Add Subscription"
        onSave={handleSave}
        onCancel={handleCancel}
      />
    </Box>
  )
}
