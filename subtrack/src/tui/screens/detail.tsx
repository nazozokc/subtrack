import { Box, Text, useInput } from "ink"
import Gradient from "ink-gradient"
import { useMemo, useState } from "react"
import { getSubscription } from "../../db.ts"
import { useTui } from "../context/app-context.tsx"
import { formatPrice } from "../../price.ts"

export function DetailScreen() {
  const { state, dispatch } = useTui()
  const [showRaw, setShowRaw] = useState(false)

  const sub = useMemo(
    () => (state.selectedId ? getSubscription(state.selectedId) : undefined),
    [state.selectedId],
  )

  useInput(
    (input, key) => {
      if (key.escape || input === "q") {
        handleBack()
        return
      }
      if (input === "e") {
        if (state.selectedId !== null) {
          dispatch({ type: "SET_SCREEN", screen: "edit" })
        }
        return
      }
      if (input === "d") {
        if (state.selectedId !== null) {
          dispatch({ type: "SET_SCREEN", screen: "delete" })
        }
        return
      }
      if (input === "r") {
        setShowRaw((p) => !p)
        return
      }
    },
    { isActive: true },
  )

  function handleBack() {
    dispatch({ type: "GO_BACK" })
  }

  if (!sub) {
    return (
      <Box flexGrow={1} alignItems="center" justifyContent="center">
        <Text dimColor>Subscription not found</Text>
      </Box>
    )
  }

  // ── Computed fields ──

  const statusColor: Record<string, string> = {
    active: "green",
    paused: "yellow",
    cancelled: "red",
  }

  const statusLabel: Record<string, string> = {
    active: "● Active",
    paused: "◐ Paused",
    cancelled: "○ Cancelled",
  }

  const cycleLabel: Record<string, string> = {
    weekly: "/week",
    "bi-weekly": "/2 weeks",
    monthly: "/month",
    quarterly: "/quarter",
    "semi-annual": "/6 months",
    yearly: "/year",
  }

  return (
    <Box flexDirection="column" flexGrow={1} paddingX={1} paddingY={1}>
      {/* Header */}
      <Box marginBottom={1}>
        <Gradient name="pastel">
          <Text bold inverse>
            {" Subscription Detail "}
          </Text>
        </Gradient>
      </Box>

      {/* Main card */}
      <Box
        borderStyle="round"
        borderColor="cyan"
        paddingX={2}
        paddingY={1}
        flexDirection="column"
      >
        {/* Name + status row */}
        <Box marginBottom={1}>
          <Box flexGrow={1}>
            <Text bold color="white">
              {sub.name}
            </Text>
          </Box>
          <Box>
            <Text bold color={statusColor[sub.status]}>
              {statusLabel[sub.status]}
            </Text>
          </Box>
        </Box>

        <Text dimColor>{"─".repeat(50)}</Text>

        {/* Price */}
        <Box marginTop={1}>
          <Box width={18}><Text dimColor>Price:</Text></Box>
          <Text bold color="yellow">
            {formatPrice(sub.price, sub.currency)}
          </Text>
          <Text dimColor>{" "}{cycleLabel[sub.cycle] ?? `/${sub.cycle}`}</Text>
        </Box>

        {/* Cycle */}
        <Box>
          <Box width={18}><Text dimColor>Billing Cycle:</Text></Box>
          <Text bold>{sub.cycle}</Text>
        </Box>

        {/* Billing day */}
        <Box>
          <Box width={18}><Text dimColor>Billing Day:</Text></Box>
          <Text bold>{sub.billingDay ?? "—"}</Text>
          {sub.billingDay && (
            <Text dimColor>{" "}(day of month)</Text>
          )}
        </Box>

        {/* Payment method */}
        <Box>
          <Box width={18}><Text dimColor>Payment Method:</Text></Box>
          <Text bold>{sub.paymentMethod ?? "—"}</Text>
        </Box>

        {/* Created */}
        <Box>
          <Box width={18}><Text dimColor>Created:</Text></Box>
          <Text bold>{sub.createdAt}</Text>
        </Box>

        {/* Tags */}
        <Box>
          <Box width={18}><Text dimColor>Tags:</Text></Box>
          <Text bold>
            {sub.tags.length > 0 ? sub.tags.join(", ") : "—"}
          </Text>
        </Box>

        {/* Notes */}
        {sub.notes && (
          <>
            <Box marginTop={1}><Text dimColor>{"─".repeat(50)}</Text></Box>
            <Box marginTop={1} flexDirection="column">
              <Text dimColor>Notes:</Text>
              <Box
                borderStyle="single"
                borderColor="gray"
                paddingX={1}
                paddingY={1}
                marginTop={1}
              >
                <Text wrap="wrap">{sub.notes}</Text>
              </Box>
            </Box>
          </>
        )}
      </Box>

      {/* Actions */}
      <Box marginTop={1} flexDirection="column">
        <Box
          borderStyle="round"
          borderColor="gray"
          paddingX={1}
          paddingY={0}
        >
          <Text dimColor>
            {"  "}e: edit    d: delete    q/ Esc: back
          </Text>
          {showRaw && (
            <Text dimColor>
              {"  "}ID: {sub.id} | Price (raw): {sub.price} {sub.currency}
            </Text>
          )}
        </Box>
      </Box>
    </Box>
  )
}
