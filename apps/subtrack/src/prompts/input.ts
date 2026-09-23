/**
 * Self-contained `input` prompt: typed text with optional default and a
 * validate-loop that re-renders errors until validation passes.
 */

import {
  ExitPromptError,
  Renderer,
  cyan,
  red,
  fallback,
  finishPrompt,
  resolveStreams,
  withRawMode,
} from "./core.ts"
import type { PromptStreams } from "./core.ts"

export type InputConfig = PromptStreams & {
  message: string
  default?: string
  validate?: (value: string) => string | true
}

export async function input(config: InputConfig): Promise<string> {
  const { stdin, stdout, interactive } = resolveStreams(config)
  if (!interactive) {
    return fallback(config.default !== undefined, config.default ?? "")
  }

  const renderer = new Renderer(stdout)
  let value = config.default ?? ""
  let error: string | null = null

  function render() {
    const line = `\r${cyan("?")} ${config.message} ${value}`
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
        if (value !== "") {
          finishPrompt(stdout, config.message, value)
          return value
        }
        return fallback(config.default !== undefined, config.default ?? "")
      }
      if (key.name === "return") {
        const validation = config.validate ? config.validate(value) : true
        if (validation !== true) {
          error = validation
          continue
        }
        renderer.clear()
        finishPrompt(stdout, config.message, value)
        return value
      }
      if (key.name === "backspace") {
        value = value.slice(0, -1)
        error = null
        continue
      }
      if (key.name === "right" || key.name === "left" || key.name === "up" || key.name === "down") {
        continue
      }
      if (key.str !== undefined && !key.ctrl && key.str.length === 1) {
        value += key.str
        error = null
      }
    }
  })
}