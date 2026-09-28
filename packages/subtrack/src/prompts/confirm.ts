/**
 * Self-contained `confirm` prompt: y/n keys with Enter resolving to the
 * configured default.
 */

import { cyan, dim, red } from "./ansi.ts"
import { ExitPromptError, fallback, resolveStreams } from "./core.ts"
import type { PromptStreams } from "./core.ts"
import { withRawMode } from "./keys.ts"
import { Renderer, finishPrompt } from "./renderer.ts"

export type ConfirmConfig = PromptStreams & {
  message: string
  default?: boolean
}

export async function confirm(config: ConfirmConfig): Promise<boolean> {
  const { stdin, stdout, interactive } = resolveStreams(config)
  if (!interactive) {
    // No user reply in a script: decline instead of silently answering the
    // default (a default-true confirm would otherwise write data unprompted).
    return fallback(config.default !== undefined, false)
  }

  const renderer = new Renderer(stdout)
  let error: string | null = null

  function render() {
    const hint =
      config.default === undefined
        ? dim("(y/n)")
        : config.default
          ? dim("(Y/n)")
          : dim("(y/N)")
    const line = `${cyan("?")} ${config.message} ${hint}`
    const errorLine = error !== null ? `\n${red(`✖ ${error}`)}` : ""
    renderer.render(line + errorLine)
  }

  return withRawMode(stdin, stdout, async ({ readKey }) => {
    for (;;) {
      render()
      const key = await readKey()
      if (key.name === "ctrl-c" || key.name === "escape") {
        renderer.clear()
        throw new ExitPromptError()
      }
      if (key.name === "eof") {
        renderer.clear()
        return fallback(config.default !== undefined, false)
      }
      if (key.name === "return") {
        if (config.default !== undefined) {
          renderer.clear()
          finishPrompt(stdout, config.message, config.default ? "yes" : "no")
          return config.default
        }
        continue
      }
      if (key.name === "y" || key.name === "Y") {
        renderer.clear()
        finishPrompt(stdout, config.message, "yes")
        return true
      }
      if (key.name === "n" || key.name === "N") {
        renderer.clear()
        finishPrompt(stdout, config.message, "no")
        return false
      }
      error = "Please answer y or n"
    }
  })
}