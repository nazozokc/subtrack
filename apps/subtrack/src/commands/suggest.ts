// ── Suggest command: manage subscription suggestions ──

import { define } from "gunshi"
import { fail } from "../error.ts"

const suggestListCmd = define({
  name: "list",
  description: "List pending suggestions",
  args: {
    all: { type: "boolean", description: "Show all suggestions (including dismissed/added)" },
    json: { type: "boolean", short: "j", description: "Output as JSON" },
  },
  run: async (ctx) => {
    const { handleSuggestList } = await import("../suggest/suggest.ts")
    return handleSuggestList({ all: ctx.values.all, json: ctx.values.json })
  },
})

const suggestViewCmd = define({
  name: "view",
  description: "View full details of a suggestion",
  args: {
    id: { type: "positional", description: "Suggestion ID", required: true },
  },
  run: async (ctx) => {
    const id = Number(ctx.values.id)
    if (isNaN(id)) { fail("Invalid suggestion ID"); return }
    const { handleSuggestView } = await import("../suggest/suggest.ts")
    return handleSuggestView(id)
  },
})

const suggestAddCmd = define({
  name: "add",
  description: "Add a suggestion as a subscription (non-interactive)",
  args: {
    id: { type: "positional", description: "Suggestion ID", required: true },
  },
  run: async (ctx) => {
    const id = Number(ctx.values.id)
    if (isNaN(id)) { fail("Invalid suggestion ID"); return }
    const { handleSuggestAdd } = await import("../suggest/suggest.ts")
    return handleSuggestAdd(id)
  },
})

const suggestDismissCmd = define({
  name: "dismiss",
  description: "Dismiss a suggestion",
  args: {
    id: { type: "positional", description: "Suggestion ID", required: false },
    all: { type: "boolean", description: "Dismiss all pending suggestions" },
  },
  run: async (ctx) => {
    if (ctx.values.all) {
      const { handleSuggestDismissAll } = await import("../suggest/suggest.ts")
      return handleSuggestDismissAll()
    } else if (ctx.values.id) {
      const id = Number(ctx.values.id)
      if (isNaN(id)) { fail("Invalid suggestion ID"); return }
      const { handleSuggestDismiss } = await import("../suggest/suggest.ts")
      return handleSuggestDismiss(id)
    } else {
      fail("Specify a suggestion ID or use --all")
    }
  },
})

export const suggestCommand = define({
  name: "suggest",
  description: "Manage subscription suggestions",
  subCommands: {
    list: suggestListCmd,
    view: suggestViewCmd,
    add: suggestAddCmd,
    dismiss: suggestDismissCmd,
  },
  run: async () => {
    // Default: run interactive review
    const { handleSuggestReview } = await import("../suggest/suggest.ts")
    return handleSuggestReview()
  },
})