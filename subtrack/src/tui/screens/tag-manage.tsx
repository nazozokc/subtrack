import { Box, Text, useInput } from "ink"
import { TextInput } from "@inkjs/ui"
import { useState, useMemo } from "react"
import { getTagsWithCount, renameTag, deleteTag, pruneTags } from "../../db.ts"
import { useTui } from "../context/app-context.tsx"

type Mode = "list" | "rename" | "delete" | "prune"

export function TagManageScreen() {
  const { dispatch } = useTui()
  const [mode, setMode] = useState<Mode>("list")
  const [renameOld, setRenameOld] = useState("")
  const [renameNew, setRenameNew] = useState("")
  const [deleteName, setDeleteName] = useState("")
  const [message, setMessage] = useState<string | null>(null)

  const tags = useMemo(() => getTagsWithCount(), [])

  useInput((input, key) => {
    if (key.escape) {
      if (mode !== "list") { setMode("list"); setMessage(null); return }
      dispatch({ type: "SET_SCREEN", screen: "list" })
      return
    }

    if (mode === "list") {
      if (input === "r") setMode("rename")
      else if (input === "d") setMode("delete")
      else if (input === "p") {
        const count = pruneTags()
        setMessage(`Pruned ${count} orphaned tag${count !== 1 ? "s" : ""}`)
      }
    }
  })

  return (
    <Box flexDirection="column" flexGrow={1} paddingX={1}>
      <Box marginBottom={1}>
        <Text bold underline>Tag Management</Text>
        <Text dimColor>  [r] rename  [d] delete  [p] prune  [Esc] back</Text>
      </Box>

      {message && <Text color={message.startsWith("Pruned") ? "yellow" : "green"}>{message}</Text>}

      {mode === "list" && (
        <>
          {tags.length === 0 ? (
            <Text dimColor>No tags</Text>
          ) : (
            tags.map((t) => (
              <Box key={t.name}>
                <Box width={24}><Text bold wrap="truncate-end">{t.name}</Text></Box>
                <Text dimColor>{t.count} subscription{t.count !== 1 ? "s" : ""}</Text>
              </Box>
            ))
          )}
        </>
      )}

      {mode === "rename" && (
        <Box flexDirection="column" gap={1}>
          <TextInput placeholder="Current tag name" defaultValue={renameOld} onChange={setRenameOld} onSubmit={() => { if (renameOld.trim() && renameNew.trim()) { renameTag(renameOld.trim(), renameNew.trim()); setMode("list"); setMessage(`Renamed "${renameOld}" → "${renameNew}"`) } }} />
          <TextInput placeholder="New tag name" defaultValue={renameNew} onChange={setRenameNew} />
        </Box>
      )}

      {mode === "delete" && (
        <Box flexDirection="column" gap={1}>
          <TextInput placeholder="Tag name to delete" defaultValue={deleteName} onChange={setDeleteName} onSubmit={() => { if (deleteName.trim()) { deleteTag(deleteName.trim()); setMode("list"); setMessage(`Deleted tag: ${deleteName}`) } }} />
        </Box>
      )}
    </Box>
  )
}
