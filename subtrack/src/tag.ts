import { consola } from "consola"
import { getTagsWithCount, renameTag, deleteTag, pruneTags } from "./db.ts"

export function handleTagList() {
  const tags = getTagsWithCount()
  if (tags.length === 0) {
    consola.info("No tags found")
    return
  }
  const maxNameLen = Math.max(...tags.map((t) => t.name.length), 4)
  consola.log(`${"Name".padEnd(maxNameLen)}  Subscriptions`)
  consola.log("─".repeat(maxNameLen + 14))
  for (const t of tags) {
    consola.log(`${t.name.padEnd(maxNameLen)}  ${t.count}`)
  }
}

export function handleTagRename(oldName: string, newName: string) {
  if (!oldName || !newName) {
    consola.error("Usage: subtrack tag rename <old> <new>")
    return
  }
  try {
    if (renameTag(oldName, newName)) {
      consola.success(`Renamed tag: "${oldName}" → "${newName}"`)
    } else {
      consola.error(`Tag "${oldName}" not found`)
    }
  } catch (e) {
    consola.error(`Failed to rename tag: ${String(e)}`)
  }
}

export function handleTagDelete(name: string) {
  if (!name) {
    consola.error("Usage: subtrack tag delete <name>")
    return
  }
  if (deleteTag(name)) {
    consola.success(`Deleted tag: "${name}"`)
  } else {
    consola.error(`Tag "${name}" not found`)
  }
}

export function handleTagPrune() {
  const count = pruneTags()
  if (count > 0) {
    consola.success(`Removed ${count} orphaned tag${count > 1 ? "s" : ""}`)
  } else {
    consola.info("No orphaned tags found")
  }
}
