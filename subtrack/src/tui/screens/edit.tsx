import { useMemo } from "react"
import { Box, Text } from "ink"
import { SubscriptionForm } from "./subscription-form.tsx"
import { getSubscription, updateSubscription } from "../../db.ts"
import { useTui } from "../context/app-context.tsx"
import type { AddSharedArgs } from "../../types.ts"

export function EditScreen() {
  const { state, dispatch } = useTui()

  const sub = useMemo(
    () => (state.editId ? getSubscription(state.editId) : undefined),
    [state.editId],
  )

  if (!sub) {
    return (
      <Box flexGrow={1} alignItems="center" justifyContent="center">
        <Text dimColor>Subscription not found</Text>
      </Box>
    )
  }

  const handleSave = (data: AddSharedArgs) => {
    updateSubscription(sub.id, data)
    dispatch({ type: "SET_SCREEN", screen: "list" })
    dispatch({ type: "SET_EDIT_ID", id: null })
  }

  const handleCancel = () => {
    dispatch({ type: "SET_SCREEN", screen: "list" })
    dispatch({ type: "SET_EDIT_ID", id: null })
  }

  return (
    <SubscriptionForm
      title={`Edit: ${sub.name}`}
      initial={{
        name: sub.name,
        price: String(sub.price),
        currency: sub.currency,
        cycle: sub.cycle,
        billingDay: sub.billingDay ? String(sub.billingDay) : "",
        status: sub.status,
        paymentMethod: sub.paymentMethod ?? "",
        tags: sub.tags.join(", "),
        notes: sub.notes ?? "",
      }}
      onSave={handleSave}
      onCancel={handleCancel}
    />
  )
}
