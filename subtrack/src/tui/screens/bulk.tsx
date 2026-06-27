import { Box, Text, useInput } from "ink"
import { Select, TextInput } from "@inkjs/ui"
import { useState, useMemo, useCallback } from "react"
import { useTui } from "../context/app-context.tsx"
import { getSubscriptions, deleteSubscription, updateSubscription } from "../../db.ts"

export function BulkScreen() {
  const { dispatch } = useTui()
  const [step, setStep] = useState<"action" | "tag" | "confirm" | "done">("action")
  const [selected, setSelected] = useState<Set<number>>(new Set())
  const [action, setAction] = useState<"delete" | "tag" | "edit" | null>(null)
  const [newTag, setNewTag] = useState("")
  const [result, setResult] = useState<string | null>(null)

  const subs = useMemo(() => getSubscriptions(), [])

  useInput((input, key) => {
    if (key.escape) dispatch({ type: "SET_SCREEN", screen: "list" })
  })

  const toggleSub = useCallback((id: number) => {
    setSelected((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id); else next.add(id)
      return next
    })
  }, [])

  const executeAction = useCallback(() => {
    if (!action || selected.size === 0) return
    switch (action) {
      case "delete": {
        let count = 0
        for (const id of selected) { if (deleteSubscription(id)) count++ }
        setResult(`Deleted ${count} subscription${count !== 1 ? "s" : ""}`)
        break
      }
      case "tag": {
        if (!newTag.trim()) { setResult("No tag provided"); break }
        let count = 0
        for (const id of selected) {
          const sub = subs.find((s) => s.id === id)
          if (sub && !sub.tags.includes(newTag.trim())) {
            updateSubscription(id, { ...sub, tags: [...sub.tags, newTag.trim()] })
            count++
          }
        }
        setResult(`Added tag "${newTag}" to ${count} subscription${count !== 1 ? "s" : ""}`)
        break
      }
      default: setResult("Action not implemented")
    }
    setStep("done")
  }, [action, selected, newTag, subs])

  return (
    <Box flexDirection="column" flexGrow={1} paddingX={1}>
      <Box marginBottom={1}>
        <Text bold underline>Bulk Operations</Text>
      </Box>

      {step === "action" && (
        <>
          <Text dimColor>Select action:</Text>
          <Select options={[{label:"Delete selected",value:"delete"},{label:"Add tag to selected",value:"tag"}]} onChange={(v) => { setAction(v as "delete" | "tag"); if (v === "tag") setStep("tag"); else setStep("confirm") }} />
        </>
      )}

      {step === "tag" && (
        <Box flexDirection="column" gap={1}>
          <TextInput placeholder="Tag name to add..." defaultValue={newTag} onChange={setNewTag} onSubmit={() => setStep("confirm")} />
        </Box>
      )}

      {step === "confirm" && (
        <Box flexDirection="column" gap={1}>
          <Text>Apply <Text bold>{action}</Text> to {selected.size} subscription{selected.size !== 1 ? "s" : ""}?</Text>
          <Select options={[{label:"Yes",value:"yes"},{label:"Cancel",value:"no"}]} onChange={(v) => { if (v === "yes") executeAction(); else dispatch({ type: "SET_SCREEN", screen: "list" }) }} />
        </Box>
      )}

      {step === "done" && (
        <Box flexDirection="column" gap={1}>
          <Text color="green">{result}</Text>
          <Text dimColor>Press any key to return</Text>
          <Select options={[{label:"Back to list",value:"back"}]} onChange={() => dispatch({ type: "SET_SCREEN", screen: "list" })} />
        </Box>
      )}
    </Box>
  )
}
