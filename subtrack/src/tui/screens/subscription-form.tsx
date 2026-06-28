import { Box, Text, useInput } from "ink"
import { TextInput, Select } from "@inkjs/ui"
import { useState, useCallback, useEffect } from "react"
import { useSetFormActive } from "../context/app-context.tsx"
import type { AddSharedArgs, Cycle, Status } from "../../types.ts"

// ── Types ──────────────────────────────────────────────

type FormData = {
  name: string
  price: string
  currency: string
  cycle: string
  billingDay: string
  status: string
  paymentMethod: string
  tags: string
  notes: string
}

type FormStep =
  | "name" | "price" | "currency" | "cycle"
  | "billingDay" | "status" | "paymentMethod"
  | "tags" | "notes" | "confirm"

type Props = {
  initial?: Partial<FormData>
  onSave: (data: AddSharedArgs) => void
  onCancel: () => void
  title: string
}

const defaultForm = (initial?: Partial<FormData>): FormData => ({
  name: initial?.name ?? "",
  price: initial?.price ?? "",
  currency: initial?.currency ?? "USD",
  cycle: initial?.cycle ?? "monthly",
  billingDay: initial?.billingDay ?? "",
  status: initial?.status ?? "active",
  paymentMethod: initial?.paymentMethod ?? "",
  tags: initial?.tags ?? "",
  notes: initial?.notes ?? "",
})

// ── Option lists ───────────────────────────────────────

const CURRENCIES = [
  { label: "USD ($)", value: "USD" },
  { label: "JPY (¥)", value: "JPY" },
  { label: "EUR (€)", value: "EUR" },
  { label: "GBP (£)", value: "GBP" },
  { label: "CAD (C$)", value: "CAD" },
  { label: "AUD (A$)", value: "AUD" },
  { label: "KRW (₩)", value: "KRW" },
  { label: "CNY (¥)", value: "CNY" },
  { label: "INR (₹)", value: "INR" },
  { label: "MXN (Mex$)", value: "MXN" },
  { label: "BRL (R$)", value: "BRL" },
  { label: "CHF (Fr)", value: "CHF" },
]

const CYCLES = [
  { label: "Weekly", value: "weekly" },
  { label: "Bi-weekly", value: "bi-weekly" },
  { label: "Monthly", value: "monthly" },
  { label: "Quarterly", value: "quarterly" },
  { label: "Semi-annual", value: "semi-annual" },
  { label: "Yearly", value: "yearly" },
]

const STATUSES = [
  { label: "Active", value: "active" },
  { label: "Paused", value: "paused" },
  { label: "Cancelled", value: "cancelled" },
]

// ── Steps ──────────────────────────────────────────────

const STEPS: FormStep[] = [
  "name", "price", "currency", "cycle",
  "billingDay", "status", "paymentMethod",
  "tags", "notes", "confirm",
]

const STEP_LABELS: Record<FormStep, string> = {
  name: "Name",
  price: "Price (in smallest unit — e.g. 1490 for ¥1,490)",
  currency: "Currency",
  cycle: "Billing Cycle",
  billingDay: "Billing Day of Month (1-31, optional)",
  status: "Status",
  paymentMethod: "Payment Method (optional — e.g. credit_card, paypal)",
  tags: "Tags (comma-separated, optional)",
  notes: "Notes (optional)",
  confirm: "Confirm",
}

// ── Validation ─────────────────────────────────────────

function validateStep(step: FormStep, data: FormData): string | null {
  switch (step) {
    case "name":
      if (!data.name.trim()) return "Name is required"
      break
    case "price":
      if (!data.price.trim()) return "Price is required"
      if (isNaN(Number(data.price)) || Number(data.price) < 0 || !Number.isInteger(Number(data.price))) return "Price must be a non-negative integer"
      break
    case "billingDay":
      if (data.billingDay.trim()) {
        const d = Number(data.billingDay)
        if (isNaN(d) || d < 1 || d > 31 || !Number.isInteger(d)) return "Billing day must be 1-31"
      }
      break
  }
  return null
}

// ── Component ─────────────────────────────────────────

export function SubscriptionForm({ initial, onSave, onCancel, title }: Props) {
  const [data, setData] = useState<FormData>(() => defaultForm(initial))
  const [stepIdx, setStepIdx] = useState(0)
  const [error, setError] = useState<string | null>(null)
  const step = STEPS[stepIdx]
  const setFormActive = useSetFormActive()

  // Prevent global key handler conflicts while form is active
  useEffect(() => {
    setFormActive(true)
    return () => setFormActive(false)
  }, [setFormActive])

  const goNext = useCallback(() => {
    const err = validateStep(step, data)
    if (err) {
      setError(err)
      return
    }
    setError(null)
    if (stepIdx < STEPS.length - 1) {
      setStepIdx(stepIdx + 1)
    }
  }, [step, stepIdx, data])

  const updateField = useCallback(
    (field: keyof FormData, value: string) => {
      setData((d) => ({ ...d, [field]: value }))
      setError(null)
    },
    [],
  )

  // Handle input for the current step
  useInput(
    (input, key) => {
      if (step === "confirm") {
        if (input === "y" || input === "Y") {
          handleConfirm()
        } else if (input === "n" || input === "N" || key.escape) {
          onCancel()
        }
      } else if (key.escape) {
        onCancel()
      }
    },
    { isActive: true },
  )

  const handleConfirm = useCallback(() => {
    const price = Number(data.price)
    if (isNaN(price) || price < 0 || !Number.isInteger(price)) {
      setError("Price must be a non-negative integer")
      return
    }

    const tags = data.tags
      .split(",")
      .map((t) => t.trim())
      .filter(Boolean)

    const billingDay = data.billingDay.trim()
      ? Number(data.billingDay)
      : null

    const status = data.status as Status

    onSave({
      name: data.name.trim(),
      price,
      currency: data.currency,
      cycle: data.cycle as Cycle,
      status,
      billingDay,
      paymentMethod: data.paymentMethod.trim() || null,
      notes: data.notes.trim() || null,
      tags,
    })
  }, [data, onSave])

  // ── Render step ──

  return (
    <Box flexDirection="column" flexGrow={1} paddingX={1}>
      <Box marginBottom={1}>
        <Text bold>{title}</Text>
        <Text dimColor>
          {" "}— Step {stepIdx + 1}/{STEPS.length}
        </Text>
      </Box>

      <Box flexDirection="column" marginBottom={1}>
        <Text bold underline>
          {STEP_LABELS[step]}
        </Text>
      </Box>

      {error && (
        <Box marginBottom={1}>
          <Text color="red">{error}</Text>
        </Box>
      )}

      <Box flexDirection="column" flexGrow={1}>
        {renderStep(step, data, updateField, goNext)}
      </Box>

      {/* Summary footer */}
      <Box marginTop={1} flexDirection="column">
        <Text dimColor wrap="truncate-end">
          Name: <Text bold>{data.name || "—"}</Text>
          {" | "}Price: <Text bold>{data.price || "—"}</Text>
          {" | "}Cycle: <Text bold>{data.cycle}</Text>
        </Text>
        <Text dimColor>
          {step === "confirm" ? "[y] confirm  [n] cancel" : "[Enter] next  [Esc] cancel"}
        </Text>
      </Box>
    </Box>
  )
}

// ── Step renderer ─────────────────────────────────────

function renderStep(
  step: FormStep,
  data: FormData,
  update: (field: keyof FormData, value: string) => void,
  next: () => void,
) {
  switch (step) {
    case "name":
      return (
        <TextInput
          placeholder="e.g. Netflix"
          defaultValue={data.name}
          onChange={(v) => update("name", v)}
          onSubmit={next}
        />
      )

    case "price":
      return (
        <TextInput
          placeholder="e.g. 1490"
          defaultValue={data.price}
          onChange={(v) => update("price", v)}
          onSubmit={next}
        />
      )

    case "currency":
      return (
        <Select
          options={CURRENCIES}
          defaultValue={data.currency}
          onChange={(v) => {
            update("currency", v)
            next()
          }}
        />
      )

    case "cycle":
      return (
        <Select
          options={CYCLES}
          defaultValue={data.cycle}
          onChange={(v) => {
            update("cycle", v)
            next()
          }}
        />
      )

    case "billingDay":
      return (
        <TextInput
          placeholder="e.g. 15 (leave empty for unknown)"
          defaultValue={data.billingDay}
          onChange={(v) => update("billingDay", v)}
          onSubmit={next}
        />
      )

    case "status":
      return (
        <Select
          options={STATUSES}
          defaultValue={data.status}
          onChange={(v) => {
            update("status", v)
            next()
          }}
        />
      )

    case "paymentMethod":
      return (
        <TextInput
          placeholder="e.g. credit_card (optional)"
          defaultValue={data.paymentMethod}
          onChange={(v) => update("paymentMethod", v)}
          onSubmit={next}
        />
      )

    case "tags":
      return (
        <TextInput
          placeholder="e.g. video, entertainment (comma-separated)"
          defaultValue={data.tags}
          onChange={(v) => update("tags", v)}
          onSubmit={next}
        />
      )

    case "notes":
      return (
        <TextInput
          placeholder="Optional notes..."
          defaultValue={data.notes}
          onChange={(v) => update("notes", v)}
          onSubmit={next}
        />
      )

    case "confirm": {
      const price = Number(data.price)
      const billingDay = data.billingDay.trim() ? Number(data.billingDay) : null
      return (
        <Box flexDirection="column" gap={1}>
          <Box>
            <Box width={16}><Text dimColor>Name:</Text></Box>
            <Text bold>{data.name}</Text>
          </Box>
          <Box>
            <Box width={16}><Text dimColor>Price:</Text></Box>
            <Text bold>{isNaN(price) ? "—" : `${price} ${data.currency}`}</Text>
          </Box>
          <Box>
            <Box width={16}><Text dimColor>Cycle:</Text></Box>
            <Text bold>{data.cycle}</Text>
          </Box>
          <Box>
            <Box width={16}><Text dimColor>Status:</Text></Box>
            <Text bold>{data.status}</Text>
          </Box>
          {billingDay && (
            <Box>
              <Box width={16}><Text dimColor>Billing Day:</Text></Box>
              <Text bold>{billingDay}</Text>
            </Box>
          )}
          {data.paymentMethod.trim() && (
            <Box>
              <Box width={16}><Text dimColor>Method:</Text></Box>
              <Text bold>{data.paymentMethod.trim()}</Text>
            </Box>
          )}
          {data.tags.trim() && (
            <Box>
              <Box width={16}><Text dimColor>Tags:</Text></Box>
              <Text bold>{data.tags}</Text>
            </Box>
          )}
          {data.notes.trim() && (
            <Box>
              <Box width={16}><Text dimColor>Notes:</Text></Box>
              <Text bold wrap="truncate-end">{data.notes}</Text>
            </Box>
          )}
        </Box>
      )
    }
  }
}


