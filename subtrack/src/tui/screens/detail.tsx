import { Box, Text, useInput, useWindowSize } from "ink"
import { useMemo, useState } from "react"
import { getSubscription } from "../../db.ts"
import { useTui } from "../context/app-context.tsx"
import { formatPrice } from "../../price.ts"
import { colors, borderStyle, sidebar } from "../theme.ts"
import { Divider } from "../components/divider.tsx"
import { Header } from "../components/header.tsx"

// ── Shared ──

const STATUS_COLOR: Record<string, string> = {
  active: colors.statusActive,
  paused: colors.statusPaused,
  cancelled: colors.statusCancelled,
}

const STATUS_LABEL: Record<string, string> = {
  active: "● Active",
  paused: "◐ Paused",
  cancelled: "○ Cancelled",
}

const CYCLE_LABEL: Record<string, string> = {
  weekly: "/week",
  "bi-weekly": "/2 weeks",
  monthly: "/month",
  quarterly: "/quarter",
  "semi-annual": "/6 months",
  yearly: "/year",
}

// ── Detail Screen — full-page view (used by screen-router) ──

export function DetailScreen() {
  const { state, dispatch } = useTui()
  const { columns: termCols } = useWindowSize()
  const [showRaw, setShowRaw] = useState(false)

  const sub = useMemo(
    () => (state.selectedId ? getSubscription(state.selectedId) : undefined),
    [state.selectedId],
  )

  useInput(
    (input, key) => {
      if (input === "e" && state.selectedId !== null) {
        dispatch({ type: "SET_SCREEN", screen: "edit" })
        return
      }
      if (input === "d" && state.selectedId !== null) {
        dispatch({ type: "SET_SCREEN", screen: "delete" })
        return
      }
      if (input === "r") {
        setShowRaw((p) => !p)
        return
      }
    },
    { isActive: true },
  )

  if (!sub) {
    return (
      <Box flexGrow={1} alignItems="center" justifyContent="center">
        <Text color={colors.textDim}>Subscription not found</Text>
      </Box>
    )
  }

  return (
    <Box flexDirection="column" flexGrow={1}>
      <Header gradient>Subscription Detail</Header>

      <Box
        borderStyle={borderStyle}
        borderColor={colors.border}
        paddingX={1}
        paddingY={1}
        flexDirection="column"
        flexGrow={1}
      >
        {/* Name + status */}
        <Box marginBottom={1}>
          <Box flexGrow={1}>
            <Text bold color={colors.text} wrap="truncate-end">
              {sub.name}
            </Text>
          </Box>
          <Box>
            <Text bold color={STATUS_COLOR[sub.status] ?? colors.text}>
              {STATUS_LABEL[sub.status] ?? sub.status}
            </Text>
          </Box>
        </Box>

        <Divider width={Math.min(60, termCols - sidebar.width - 6)} />

        {/* Fields */}
        <Box marginTop={1} flexDirection="column" gap={0}>
          <FieldRow label="Price" value={formatPrice(sub.price, sub.currency)} valueColor={colors.warning} suffix={CYCLE_LABEL[sub.cycle] ?? `/${sub.cycle}`} />
          <FieldRow label="Billing Cycle" value={sub.cycle} />
          <FieldRow label="Billing Day" value={sub.billingDay != null ? String(sub.billingDay) : "—"} suffix={sub.billingDay ? "(day of month)" : undefined} />
          <FieldRow label="Payment Method" value={sub.paymentMethod ?? "—"} />
          <FieldRow label="Created" value={sub.createdAt} />
          <FieldRow label="Tags" value={sub.tags.length > 0 ? sub.tags.join(", ") : "—"} />
        </Box>

        {/* Notes */}
        {sub.notes && (
          <>
            <Divider width={Math.min(60, termCols - sidebar.width - 6)} />
            <Box marginTop={1} flexDirection="column">
              <Text color={colors.textDim}>Notes:</Text>
              <Box
                borderStyle={borderStyle}
                borderColor={colors.border}
                paddingX={1}
                paddingY={1}
                marginTop={1}
              >
                <Text wrap="wrap">{sub.notes}</Text>
              </Box>
            </Box>
          </>
        )}

        {/* Raw view */}
        {showRaw && (
          <Box marginTop={1}>
            <Text color={colors.textDim}>
              ID: {sub.id} | Price (raw): {sub.price} {sub.currency}
            </Text>
          </Box>
        )}
      </Box>

      {/* Actions */}
      <Box marginTop={1}>
        <Box borderStyle={borderStyle} borderColor={colors.border} paddingX={1}>
          <Text color={colors.textDim}>
            {"  e:edit  d:delete  r:raw  q/Esc:back"}
          </Text>
        </Box>
      </Box>
    </Box>
  )
}

// ── Detail Preview — compact 3-pane inline view (no own input handler) ──

export function DetailPreview() {
  const { state } = useTui()

  const sub = useMemo(
    () => (state.selectedId ? getSubscription(state.selectedId) : undefined),
    [state.selectedId],
  )

  if (!sub) {
    return (
      <Box flexGrow={1} alignItems="center" justifyContent="center">
        <Text color={colors.textDim}>Not found</Text>
      </Box>
    )
  }

  return (
    <Box flexDirection="column" flexGrow={1}>
      {/* Compact header */}
      <Box>
        <Box flexGrow={1}>
          <Text bold color={colors.primary} wrap="truncate-end">
            {sub.name}
          </Text>
        </Box>
        <Text bold color={STATUS_COLOR[sub.status] ?? colors.text}>
          {"●"}
        </Text>
      </Box>

      <Divider />

      {/* 2-column compact info */}
      <Box flexDirection="column" gap={0}>
        <Box flexDirection="row">
          <Box width={12}><Text color={colors.textDim}>Price:</Text></Box>
          <Text bold color={colors.warning}>
            {formatPrice(sub.price, sub.currency)}
          </Text>
          <Text color={colors.textDim}>{" "}{CYCLE_LABEL[sub.cycle] ?? `/${sub.cycle}`}</Text>
        </Box>
        <Box flexDirection="row">
          <Box width={12}><Text color={colors.textDim}>Cycle:</Text></Box>
          <Text>{sub.cycle}</Text>
        </Box>
        <Box flexDirection="row">
          <Box width={12}><Text color={colors.textDim}>Bill Day:</Text></Box>
          <Text>{sub.billingDay ?? "—"}</Text>
        </Box>
        <Box flexDirection="row">
          <Box width={12}><Text color={colors.textDim}>Method:</Text></Box>
          <Text>{sub.paymentMethod ?? "—"}</Text>
        </Box>
        {sub.tags.length > 0 && (
          <Box flexDirection="row">
            <Box width={12}><Text color={colors.textDim}>Tags:</Text></Box>
            <Text wrap="truncate-end">{sub.tags.join(", ")}</Text>
          </Box>
        )}
      </Box>

      {sub.notes && (
        <>
          <Divider />
          <Text color={colors.textDim} wrap="truncate-end">{sub.notes}</Text>
        </>
      )}

      <Divider />

      <Text color={colors.textDim}>
        e:edit  d:del  |:close  Enter:full
      </Text>
    </Box>
  )
}

// ── Field row ──

type FieldRowProps = {
  label: string
  value: string
  valueColor?: string
  suffix?: string
}

function FieldRow({ label, value, valueColor, suffix }: FieldRowProps) {
  return (
    <Box>
      <Box width={18}>
        <Text color={colors.textDim}>{label}:</Text>
      </Box>
      <Text bold color={valueColor ?? colors.text}>
        {value}
      </Text>
      {suffix && (
        <Text color={colors.textDim}>{" "}{suffix}</Text>
      )}
    </Box>
  )
}
