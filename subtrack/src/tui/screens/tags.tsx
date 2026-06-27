import { Box, Text, useInput } from "ink"
import { TextInput } from "@inkjs/ui"
import { useState, useMemo } from "react"
import { tagsSubscription } from "../../db.ts"
import { useTui } from "../context/app-context.tsx"

export function TagsScreen() {
  const { dispatch } = useTui()
  const [tagInput, setTagInput] = useState("")

  useInput((input, key) => {
    if (key.escape) {
      dispatch({ type: "SET_SCREEN", screen: "list" })
      dispatch({ type: "SET_FILTER_TEXT", value: "" })
    }
  })

  const filtered = useMemo(() => {
    if (!tagInput.trim()) return []
    const names = tagInput.split(",").map((t) => t.trim()).filter(Boolean)
    if (names.length === 0) return []
    return tagsSubscription(names)
  }, [tagInput])

  return (
    <Box flexDirection="column" flexGrow={1} paddingX={1}>
      <Box marginBottom={1}>
        <Text bold underline>Filter by Tags</Text>
      </Box>
      <Box marginBottom={1}>
        <TextInput
          placeholder="Enter tag names, comma-separated..."
          defaultValue={tagInput}
          onChange={setTagInput}
        />
      </Box>
      {filtered.length === 0 ? (
        <Text dimColor>{tagInput ? "No matching subscriptions" : "Type tags to filter"}</Text>
      ) : (
        <Box flexDirection="column">
          <Text dimColor>{filtered.length} result{filtered.length !== 1 ? "s" : ""}</Text>
          {filtered.map((sub) => (
            <Box key={sub.id}>
              <Box width={24}><Text bold wrap="truncate-end">{sub.name}</Text></Box>
              <Text dimColor>{sub.tags.join(", ")}</Text>
            </Box>
          ))}
        </Box>
      )}
    </Box>
  )
}
