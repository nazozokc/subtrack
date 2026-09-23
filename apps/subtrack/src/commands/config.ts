// ── Config commands ───────────────────────────────────
import { define } from "gunshi"
import { consola } from "@subtrack/lib/logger"

const configListCmd = define({
  name: "list",
  description: "List all config values",
  run: async () => {
    const { handleConfigList } = await import("../config.ts")
    return handleConfigList()
  },
})

const configGetCmd = define({
  name: "get",
  description: "Get a config value",
  args: { key: { type: "positional", description: "Config key" } },
  run: async (ctx) => {
    const { handleConfigGet } = await import("../config.ts")
    return handleConfigGet(ctx.values.key)
  },
})

const configSetCmd = define({
  name: "set",
  description: "Set a config value",
  args: {
    key: { type: "positional", description: "Config key" },
    value: { type: "positional", description: "Config value" },
  },
  run: async (ctx) => {
    const { handleConfigSet } = await import("../config.ts")
    return handleConfigSet(ctx.values.key, ctx.values.value)
  },
})

const configResetCmd = define({
  name: "reset",
  description: "Reset config to defaults",
  run: async () => {
    const { handleConfigReset } = await import("../config.ts")
    return handleConfigReset()
  },
})

export const configCommand = define({
  name: "config",
  description: "Manage configuration",
  subCommands: {
    list: configListCmd,
    get: configGetCmd,
    set: configSetCmd,
    reset: configResetCmd,
  },
  run: () => consola.info("Usage: subtrack config list|get|set|reset"),
})