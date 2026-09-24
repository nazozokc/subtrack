/**
 * "Config" sub-menu (config / profiles / currencies) and "System" (MCP server).
 */

import { select, input, confirm } from "../prompts.ts"
import { BACK } from "./shared.ts"
import { handleConfigList, handleConfigGet, handleConfigSet, handleConfigReset } from "../config.ts"
import { handleProfile } from "../profile.ts"
import { handleCurrencyList } from "../currency.ts"

export async function runConfigMenu(): Promise<void> {
  while (true) {
    const action = await select({
      message: "config",
      pageSize: 10,
      choices: [
        { name: "Config", description: "List / get / set / reset configuration", value: "config" },
        { name: "Profiles", description: "Save / switch / list / show / delete filter profiles", value: "profile" },
        { name: "Currencies", description: "List supported currencies", value: "currency" },
        BACK,
      ],
    })

    switch (action) {
      case "config": await runConfigSubMenu(); break
      case "profile": await runProfileMenu(); break
      case "currency": handleCurrencyList(); break
      case "back": return
    }
  }
}

async function runConfigSubMenu(): Promise<void> {
  while (true) {
    const action = await select({
      message: "configuration",
      pageSize: 10,
      choices: [
        { name: "List", description: "Show all configuration keys", value: "list" },
        { name: "Get", description: "Show a configuration value", value: "get" },
        { name: "Set", description: "Set a configuration value", value: "set" },
        { name: "Reset", description: "Reset configuration to defaults", value: "reset" },
        BACK,
      ],
    })

    switch (action) {
      case "list": handleConfigList(); break
      case "get": {
        const key = await input({ message: "config key:", validate: (v) => v.trim().length > 0 || "Key required" })
        handleConfigGet(key.trim())
        break
      }
      case "set": {
        const key = await input({ message: "config key:", validate: (v) => v.trim().length > 0 || "Key required" })
        const value = await input({ message: `value for "${key.trim()}":` })
        handleConfigSet(key.trim(), value.trim())
        break
      }
      case "reset": {
        const ok = await confirm({ message: "Reset all configuration to defaults?", default: false })
        if (ok) await handleConfigReset()
        break
      }
      case "back": return
    }
  }
}

async function runProfileMenu(): Promise<void> {
  while (true) {
    const action = await select({
      message: "filter profiles",
      pageSize: 10,
      choices: [
        { name: "List", description: "List saved profiles", value: "list" },
        { name: "Save", description: "Save the current filter as a profile", value: "save" },
        { name: "Switch", description: "Switch to a profile", value: "switch" },
        { name: "Show", description: "Show the active profile", value: "show" },
        { name: "Delete", description: "Delete a profile", value: "delete" },
        BACK,
      ],
    })

    switch (action) {
      case "list": await handleProfile("list"); break
      case "save": await handleProfile("save"); break
      case "switch": {
        const name = await input({ message: "profile name:", validate: (v) => v.trim().length > 0 || "Name required" })
        await handleProfile("switch", name.trim())
        break
      }
      case "show": await handleProfile("show"); break
      case "delete": {
        const name = await input({ message: "profile name:", validate: (v) => v.trim().length > 0 || "Name required" })
        await handleProfile("delete", name.trim())
        break
      }
      case "back": return
    }
  }
}

export async function runSystemMenu(): Promise<void> {
  while (true) {
    const action = await select({
      message: "system",
      pageSize: 10,
      choices: [
        { name: "MCP server", description: "Start MCP server (blocks until exit)", value: "mcp" },
        BACK,
      ],
    })

    switch (action) {
      case "mcp": {
        const ok = await confirm({
          message: "Start MCP server? The menu will be blocked until the server exits.",
          default: false,
        })
        if (ok) {
          // Lazy import to avoid loading the MCP server at menu startup
          const { startMcpServer } = await import("../mcp.ts")
          await startMcpServer()
        }
        break
      }
      case "back": return
    }
  }
}