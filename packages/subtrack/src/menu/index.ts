/**
 * Interactive main menu shown when running `subtrack` without a subcommand.
 * Covers every CLI command — organized into category sub-menus.
 * Handlers are called with empty flags so they run in their interactive mode
 * (consistent with the "interactive by default" tenet).
 *
 * Menu structure is split by category: sub-menu definitions live in
 * ./views.ts (view & reports), ./edits.ts (add & edit), ./manage.ts,
 * ./data.ts, and ./config.ts (config & system). The startup header
 * (financial overview) lives in ./header.ts.
 */

import { select } from "../prompts.ts"
import { showMenuHeader } from "./header.ts"
import { runViewMenu, runReportMenu } from "./views.ts"
import { runAddMenu } from "./edits.ts"
import { runManageMenu } from "./manage.ts"
import { runDataMenu } from "./data.ts"
import { runConfigMenu, runSystemMenu } from "./config.ts"

type MainChoice = "view" | "add" | "manage" | "report" | "data" | "config" | "system" | "quit"

export async function handleMenu(): Promise<void> {
  showMenuHeader()
  while (true) {
    const choice = await select<MainChoice>({
      message: "subtrack — choose a category",
      pageSize: 10,
      choices: [
        { name: "View & Search", description: "List, search, tag filter, upcoming, calendar, history, timeline, stats", value: "view" },
        { name: "Add & Edit", description: "Add, edit, clone, import CSV, trials, suggestions", value: "add" },
        { name: "Manage", description: "Delete, archive, bulk operations, tag management, cleanup", value: "manage" },
        { name: "Reports", description: "Summary, payment, analytics, compare, forecast, optimize, notify", value: "report" },
        { name: "Data", description: "Export, backup, restore, maintenance, audit, LLM usage", value: "data" },
        { name: "Config", description: "Configuration, filter profiles, currencies", value: "config" },
        { name: "System", description: "MCP server", value: "system" },
        { name: "Quit", description: "Exit subtrack", value: "quit" },
      ],
    })

    switch (choice) {
      case "view": await runViewMenu(); break
      case "add": await runAddMenu(); break
      case "manage": await runManageMenu(); break
      case "report": await runReportMenu(); break
      case "data": await runDataMenu(); break
      case "config": await runConfigMenu(); break
      case "system": await runSystemMenu(); break
      case "quit": return
    }
  }
}