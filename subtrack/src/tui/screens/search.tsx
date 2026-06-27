import { Box, Text, useInput } from "ink"
import { TextInput } from "@inkjs/ui"
import { useState, useMemo } from "react"
import { getSubscriptions } from "../../db.ts"
import { useTui } from "../context/app-context.tsx"
import { formatPrice } from "../../price.ts"

export function SearchScreen() {
  const { dispatch } = useTui()
  const [query, setQuery] = useState("")

  useInput((input, key) => {
    if (key.escape) dispatch({ type: "SET_SCREEN", screen: "list" })
  })

  const results = useMemo(() => {
    if (!query.trim()) return []
    const q = query.toLowerCase()
    return getSubscriptions().filter(
      (s) => s.name.toLowerCase().includes(q) || s.tags.some((t) => t.toLowerCase().includes(q)) || (s.notes ?? "").toLowerCase().includes(q),
    )
  }, [query])

  return (
    <Box flexDirection="column" flexGrow={1} paddingX={1}>
      <Box marginBottom={1}>
        <Text bold underline>Search</Text>
      </Box>
      <Box marginBottom={1}>
        <TextInput placeholder="Search subscriptions..." defaultValue={query} onChange={setQuery} />
      </Box>
      {results.length === 0 ? (
        <Text dimColor>{query ? "No results" : "Type to search"}</Text>
      ) : (
        results.map((s) => (
          <Box key={s.id}>
            <Box width={24}><Text bold wrap="truncate-end">{s.name}</Text></Box>
            <Box width={14}><Text dimColor>{formatPrice(s.price, s.currency)}</Text></Box>
            <Text dimColor>{s.tags.join(", ")}</Text>
          </Box>
        ))
      )}
    </Box>
  )
}
