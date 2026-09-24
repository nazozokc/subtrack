/**
 * "Manage" sub-menu (delete / archive / bulk / tags / cleanup).
 */

import { select, input, confirm } from "../prompts.ts"
import { BACK, pickSubscription, pickTag } from "./shared.ts"
import type { Status } from "../types.ts"
import { handleDelete, handleArchive, handleUnarchive } from "../subscription/core.ts"
import { handleBulkStatus, handleBulkDelete, handleBulkTagAdd, handleBulkTagRemove } from "../bulk.ts"
import { handleTagList, handleTagRename, handleTagDelete, handleTagPrune, handleTagMerge } from "../tag.ts"
import { handleCleanup } from "../cleanup.ts"

export async function runManageMenu(): Promise<void> {
  while (true) {
    const action = await select({
      message: "manage",
      pageSize: 10,
      choices: [
        { name: "Delete", description: "Delete subscriptions", value: "delete" },
        { name: "Archive", description: "Archive a subscription", value: "archive" },
        { name: "Unarchive", description: "Unarchive a subscription", value: "unarchive" },
        { name: "Bulk", description: "Bulk status / delete / tag operations", value: "bulk" },
        { name: "Tags", description: "Tag list, rename, delete, prune, merge", value: "tag" },
        { name: "Cleanup", description: "Integrity check, VACUUM, prune audit/tags", value: "cleanup" },
        BACK,
      ],
    })

    switch (action) {
      case "delete": await handleDelete(); break
      case "archive": {
        const id = await pickSubscription("select subscription to archive")
        if (id !== null) handleArchive(id)
        break
      }
      case "unarchive": {
        const id = await pickSubscription("select subscription to unarchive", "archived")
        if (id !== null) handleUnarchive(id)
        break
      }
      case "bulk": await runBulkMenu(); break
      case "tag": await runTagMenu(); break
      case "cleanup": {
        const ok = await confirm({ message: "Run cleanup (integrity check, VACUUM, prune audit/tags)?", default: false })
        if (ok) handleCleanup()
        break
      }
      case "back": return
    }
  }
}

async function runBulkMenu(): Promise<void> {
  while (true) {
    const action = await select({
      message: "bulk operations",
      pageSize: 10,
      choices: [
        { name: "Set status", description: "Change status of matching subscriptions", value: "status" },
        { name: "Delete", description: "Delete matching subscriptions", value: "delete" },
        { name: "Add tag", description: "Add a tag to matching subscriptions", value: "tag-add" },
        { name: "Remove tag", description: "Remove a tag from matching subscriptions", value: "tag-remove" },
        BACK,
      ],
    })

    switch (action) {
      case "status": {
        const status = await select<Status>({
          message: "target status",
          choices: [
            { name: "active", value: "active" },
            { name: "paused", value: "paused" },
            { name: "cancelled", value: "cancelled" },
            { name: "archived", value: "archived" },
          ],
        })
        await handleBulkStatus(status, {}, {})
        break
      }
      case "delete": await handleBulkDelete({}, {}); break
      case "tag-add": {
        const tag = await input({ message: "tag to add:", validate: (v) => v.trim().length > 0 || "Tag required" })
        await handleBulkTagAdd(tag.trim(), {})
        break
      }
      case "tag-remove": {
        const tag = await input({ message: "tag to remove:", validate: (v) => v.trim().length > 0 || "Tag required" })
        await handleBulkTagRemove(tag.trim(), {})
        break
      }
      case "back": return
    }
  }
}

async function runTagMenu(): Promise<void> {
  while (true) {
    const action = await select({
      message: "tag management",
      pageSize: 10,
      choices: [
        { name: "List", description: "List all tags with usage counts", value: "list" },
        { name: "Rename", description: "Rename a tag", value: "rename" },
        { name: "Delete", description: "Delete a tag", value: "delete" },
        { name: "Prune", description: "Remove unused tags", value: "prune" },
        { name: "Merge", description: "Merge one tag into another", value: "merge" },
        BACK,
      ],
    })

    switch (action) {
      case "list": handleTagList(); break
      case "rename": {
        const oldName = await pickTag("select tag to rename")
        if (oldName === null) break
        const newName = await input({ message: "new name:", validate: (v) => v.trim().length > 0 || "Name required" })
        handleTagRename(oldName, newName.trim())
        break
      }
      case "delete": {
        const name = await pickTag("select tag to delete")
        if (name === null) break
        handleTagDelete(name)
        break
      }
      case "prune": handleTagPrune(); break
      case "merge": {
        const source = await pickTag("select tag to merge from")
        if (source === null) break
        const target = await pickTag("select tag to merge into")
        if (target === null) break
        if (source !== target) handleTagMerge(source, target)
        break
      }
      case "back": return
    }
  }
}