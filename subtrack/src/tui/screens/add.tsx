import { SubscriptionForm } from "./subscription-form.tsx"
import { writeSubscription } from "../../db.ts"
import { useTui } from "../context/app-context.tsx"
import type { AddSharedArgs } from "../../types.ts"

export function AddScreen() {
  const { dispatch } = useTui()

  const handleSave = (data: AddSharedArgs) => {
    writeSubscription(data)
    dispatch({ type: "SET_SCREEN", screen: "list" })
    dispatch({ type: "SET_LIST_INDEX", index: 0 })
  }

  const handleCancel = () => {
    dispatch({ type: "SET_SCREEN", screen: "list" })
  }

  return (
    <SubscriptionForm
      title="Add Subscription"
      onSave={handleSave}
      onCancel={handleCancel}
    />
  )
}
