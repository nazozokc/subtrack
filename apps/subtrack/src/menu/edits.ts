/**
 * "Add & Edit" sub-menu (add / edit / clone / import / trials / suggestions).
 */

import { select, input } from "../prompts.ts"
import { BACK, pickSubscription } from "./shared.ts"
import { handleAdd } from "../subscription/add.ts"
import { handleEdit } from "../subscription/edit.ts"
import { handleClone } from "../subscription/core.ts"
import { handleImport } from "../import-csv.ts"
import { handleTrialAdd, handleTrialList, handleTrialExpiring, handleTrialDelete } from "../trial.ts"
import { handleSuggestList, handleSuggestReview, handleSuggestDismiss } from "../suggest/suggest.ts"

export async function runAddMenu(): Promise<void> {
  while (true) {
    const action = await select({
      message: "add & edit",
      pageSize: 10,
      choices: [
        { name: "Add", description: "Add a new subscription", value: "add" },
        { name: "Edit", description: "Edit a subscription", value: "edit" },
        { name: "Clone", description: "Clone an existing subscription", value: "clone" },
        { name: "Import", description: "Import subscriptions from CSV", value: "import" },
        { name: "Trials", description: "Manage free trials", value: "trial" },
        { name: "Suggestions", description: "Review and manage suggestions", value: "suggest" },
        BACK,
      ],
    })

    switch (action) {
      case "add": await handleAdd({}); break
      case "edit": await handleEdit(); break
      case "clone": {
        const id = await pickSubscription("select subscription to clone")
        if (id !== null) await handleClone(id)
        break
      }
      case "import": {
        const file = await input({ message: "CSV file path:", validate: (v) => v.trim().length > 0 || "Path required" })
        await handleImport(file, {})
        break
      }
      case "trial": await runTrialMenu(); break
      case "suggest": await runSuggestMenu(); break
      case "back": return
    }
  }
}

async function runTrialMenu(): Promise<void> {
  while (true) {
    const action = await select({
      message: "trials",
      pageSize: 10,
      choices: [
        { name: "Add", description: "Register a free trial", value: "add" },
        { name: "List", description: "List all trials", value: "list" },
        { name: "Expiring", description: "Trials expiring within 7 days", value: "expiring" },
        { name: "Delete", description: "Delete trials", value: "delete" },
        BACK,
      ],
    })

    switch (action) {
      case "add": await handleTrialAdd({}); break
      case "list": handleTrialList(); break
      case "expiring": handleTrialExpiring(); break
      case "delete": await handleTrialDelete(); break
      case "back": return
    }
  }
}

async function runSuggestMenu(): Promise<void> {
  while (true) {
    const action = await select({
      message: "suggestions",
      pageSize: 10,
      choices: [
        { name: "List", description: "List pending suggestions", value: "list" },
        { name: "Review", description: "Review suggestions and add as subscriptions", value: "review" },
        { name: "Dismiss", description: "Dismiss a suggestion by id", value: "dismiss" },
        BACK,
      ],
    })

    switch (action) {
      case "list": handleSuggestList(); break
      case "review": await handleSuggestReview(); break
      case "dismiss": {
        const id = Number(await input({ message: "suggestion id (see suggestions list):", validate: (v) => (Number.isInteger(Number(v)) && Number(v) > 0) || "Valid id required" }))
        handleSuggestDismiss(id)
        break
      }
      case "back": return
    }
  }
}