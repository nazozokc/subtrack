import { Box, Text, useInput } from "ink"
import { TextInput, Select } from "@inkjs/ui"
import { useState, useCallback } from "react"
import { writeTrial } from "../../db.ts"
import { useTui } from "../context/app-context.tsx"

type FormData = { name: string; expiresAt: string; price: string; currency: string; cycle: string; notes: string }
type Step = "name" | "expiresAt" | "price" | "currency" | "cycle" | "notes" | "confirm"
const STEPS: Step[] = ["name", "expiresAt", "price", "currency", "cycle", "notes", "confirm"]

export function TrialAddScreen() {
  const { dispatch } = useTui()
  const [data, setData] = useState<FormData>({ name: "", expiresAt: "", price: "", currency: "USD", cycle: "monthly", notes: "" })
  const [stepIdx, setStepIdx] = useState(0)
  const step = STEPS[stepIdx]

  const update = useCallback((f: keyof FormData, v: string) => setData((d) => ({ ...d, [f]: v })), [])
  const next = useCallback(() => { if (stepIdx < STEPS.length - 1) setStepIdx(stepIdx + 1) }, [stepIdx])
  const cancel = useCallback(() => dispatch({ type: "SET_SCREEN", screen: "trials" }), [dispatch])

  useInput((input, key) => { if (key.escape) cancel() }, { isActive: step !== "confirm" })
  useInput((input) => {
    if (step === "confirm") {
      if (input === "y") {
        writeTrial({ name: data.name, expiresAt: data.expiresAt, price: data.price ? Number(data.price) : null, currency: data.currency || null, cycle: data.cycle || null, notes: data.notes || null })
        dispatch({ type: "SET_SCREEN", screen: "trials" })
      } else if (input === "n") cancel()
    }
  }, { isActive: step === "confirm" })

  return (
    <Box flexDirection="column" flexGrow={1} paddingX={1}>
      <Box marginBottom={1}><Text bold>Add Trial — Step {stepIdx + 1}/{STEPS.length}</Text></Box>
      <Box flexDirection="column" flexGrow={1}>
        {step === "name" && <TextInput placeholder="Trial name (e.g. Netflix Free)" defaultValue={data.name} onChange={(v) => update("name", v)} onSubmit={next} />}
        {step === "expiresAt" && <TextInput placeholder="Expiration date (YYYY-MM-DD)" defaultValue={data.expiresAt} onChange={(v) => update("expiresAt", v)} onSubmit={next} />}
        {step === "price" && <TextInput placeholder="Price after trial (optional)" defaultValue={data.price} onChange={(v) => update("price", v)} onSubmit={next} />}
        {step === "currency" && <Select options={[{label:"USD ($)",value:"USD"},{label:"JPY (¥)",value:"JPY"},{label:"EUR (€)",value:"EUR"},{label:"GBP (£)",value:"GBP"}]} defaultValue={data.currency} onChange={(v) => { update("currency", v); next() }} />}
        {step === "cycle" && <Select options={[{label:"Weekly",value:"weekly"},{label:"Monthly",value:"monthly"},{label:"Quarterly",value:"quarterly"},{label:"Yearly",value:"yearly"}]} defaultValue={data.cycle} onChange={(v) => { update("cycle", v); next() }} />}
        {step === "notes" && <TextInput placeholder="Notes (optional)" defaultValue={data.notes} onChange={(v) => update("notes", v)} onSubmit={next} />}
        {step === "confirm" && (
          <Box flexDirection="column" gap={1}>
            <Text>Name: <Text bold>{data.name}</Text></Text>
            <Text>Expires: <Text bold>{data.expiresAt}</Text></Text>
            {data.price && <Text>Price: <Text bold>{data.price} {data.currency}</Text></Text>}
            <Text>Save? <Text bold color="green" inverse> y </Text> / <Text bold color="red" inverse> n </Text></Text>
          </Box>
        )}
      </Box>
    </Box>
  )
}
