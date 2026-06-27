import { Box, Text, useInput } from "ink"
import { TextInput } from "@inkjs/ui"
import { useState, useMemo, useCallback } from "react"
import { getTagsWithCount, renameTag, deleteTag, pruneTags } from "../../db.ts"
import { useTui } from "../context/app-context.tsx"

type Mode = "list" | "rename-old" | "rename-new" | "delete" | "prune"

export function TagManageScreen() {
  const { dispatch } = useTui()
  const [mode, setMode] = useState<Mode>("list")
  const [renameOld, setRenameOld] = useState("")
  const [renameNew, setRenameNew] = useState("")
  const [deleteName, setDeleteName] = useState("")
  const [message, setMessage] = useState<string | null>(null)
  const [refresh, setRefresh] = useState(0)

  const tags = useMemo(() => getTagsWithCount(), [refresh])

  const refreshTags = useCallback(() => setRefresh((n) => n + 1), [])

  const doRename = useCallback(() => {
    if (renameOld.trim() && renameNew.trim()) {
      try {
        renameTag(renameOld.trim(), renameNew.trim())
        refreshTags()
        setMessage(`Renamed "${renameOld}" → "${renameNew}"`)
      } catch (e: unknown) {
        setMessage(`Error: ${e instanceof Error ? e.message : String(e)}`)
      }
      setMode("list")
    }
  }, [renameOld, renameNew, refreshTags])

  useInput((input, key) => {
    if (key.escape) {
      if (mode !== "list") { setMode("list"); setMessage(null); return }
      dispatch({ type: "SET_SCREEN", screen: "list" })
      return
    }

    if (mode === "list") {
      if (input === "r") setMode("rename-old")
      else if (input === "d") setMode("delete")
      else if (input === "p") {
        const count = pruneTags()
        setMessage(`Pruned ${count} orphaned tag${count !== 1 ? "s" : ""}`)
        refreshTags()
      }
    }
  })

  return (
    <Box flexDirection="column" flexGrow={1} paddingX={1}>
      <Box marginBottom={1}>
        <Text bold underline>Tag Management</Text>
        <Text dimColor>  [r] rename  [d] delete  [p] prune  [Esc] back</Text>
      </Box>

      {message && <Text color={message.startsWith("Error") ? "red" : message.startsWith("Pruned") ? "yellow" : "green"}>{message}</Text>}

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

      {mode === "rename-old" && (
        <Box flexDirection="column" gap={1}>
          <Text dimColor>Current tag name:</Text>
          <TextInput placeholder="e.g. streaming" defaultValue={renameOld} onChange={setRenameOld} onSubmit={() => { if (renameOld.trim()) setMode("rename-new") }} />
        </Box>
      )}

      {mode === "rename-new" && (
        <Box flexDirection="column" gap={1}>
          <Text dimColor>New tag name:</Text>
          <TextInput placeholder="e.g. video" defaultValue={renameNew} onChange={setRenameNew} onSubmit={doRename} />
        </Box>
      )}

      {mode === "delete" && (
        <Box flexDirection="column" gap={1}>
          <TextInput placeholder="Tag name to delete" defaultValue={deleteName} onChange={setDeleteName} onSubmit={() => { if (deleteName.trim()) { try { deleteTag(deleteName.trim()); refreshTags(); setMessage(`Deleted tag: ${deleteName}`) } catch (e: unknown) { setMessage(`Error: ${e instanceof Error ? e.message : String(e)}`) } setMode("list") } }} />
        </Box>
      )}
    </Box>
  )
}
