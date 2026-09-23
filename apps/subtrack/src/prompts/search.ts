/**
 * Self-contained `search` prompt: debounced async source lookup with a
 * loading indicator, arrow-key navigation, and enter to select.
 */

import { POINTER, cyan, dim, stripAnsi, truncate } from "./ansi.ts"
import { visibleWindow } from "./choices.ts"
import type { Choice } from "./core.ts"
import { ExitPromptError, resolveStreams } from "./core.ts"
import type { PromptStreams } from "./core.ts"
import { withRawMode } from "./keys.ts"
import { Renderer, finishPrompt } from "./renderer.ts"

export type SearchConfig<T> = PromptStreams & {
  message: string
  source: (query: string) => Promise<Choice<T>[]> | Choice<T>[]
  pageSize?: number
}

const DEFAULT_PAGE_SIZE = 20
const DEBOUNCE_MS = 100

export async function search<const T>(config: SearchConfig<T>): Promise<T> {
  const { stdin, stdout, interactive } = resolveStreams(config)
  if (!interactive) throw new ExitPromptError()

  const renderer = new Renderer(stdout)
  const pageSize = config.pageSize ?? DEFAULT_PAGE_SIZE
  const columns = ((config.stdout ?? process.stdout) as { columns?: number }).columns ?? 80

  let query = ""
  let items: Choice<T>[] = []
  let active = 0
  let loading = false
  let requestId = 0
  let debounceTimer: ReturnType<typeof setTimeout> | null = null

  const displayName = (item: Choice<T>) => item.name ?? String(item.value)

  function render() {
    const lines: string[] = [`\r${cyan("?")} ${config.message} ${query}`]
    if (loading) {
      lines.push(`  ${dim("Searching…")}`)
    } else if (items.length === 0) {
      lines.push(`  ${dim("No matching results")}`)
    } else {
      const { start, end } = visibleWindow(active, items.length, pageSize)
      for (let i = start; i < end && i < items.length; i++) {
        const item = items[i]!
        const pointer = i === active ? cyan(POINTER) : " "
        const description = item.description ? ` ${dim(item.description)}` : ""
        lines.push(truncate(`${pointer} ${displayName(item)}${description}`, columns))
      }
    }
    renderer.render(lines.join("\n"))
  }

  async function reload() {
    const id = ++requestId
    loading = true
    render()
    let result: Choice<T>[]
    try {
      result = await config.source(query)
    } catch {
      result = []
    }
    if (id !== requestId) return // stale response, a newer search superseded it
    items = result
    if (active >= items.length) active = items.length > 0 ? items.length - 1 : 0
    loading = false
    render()
  }

  function scheduleReload() {
    render()
    if (debounceTimer !== null) clearTimeout(debounceTimer)
    debounceTimer = setTimeout(() => {
      debounceTimer = null
      void reload()
    }, DEBOUNCE_MS)
  }

  function invalidate() {
    requestId++
    if (debounceTimer !== null) {
      clearTimeout(debounceTimer)
      debounceTimer = null
    }
  }

  const outcome = await withRawMode(stdin, stdout, async ({ readKey }) => {
    await reload() // initial fetch with empty query
    for (;;) {
      const key = await readKey()
      if (key.name === "ctrl-c" || key.name === "escape") {
        renderer.clear()
        throw new ExitPromptError()
      }
      if (key.name === "eof") {
        renderer.clear()
        return undefined
      }
      if (key.name === "up" || key.name === "down") {
        if (items.length === 0) continue
        active += key.name === "up" ? -1 : 1
        if (active < 0) active = items.length - 1
        else if (active >= items.length) active = 0
        render()
        continue
      }
      if (key.name === "return") {
        const item = items[active]
        if (!item) continue
        renderer.clear()
        return { value: item.value, name: stripAnsi(displayName(item)) }
      }
      if (key.name === "backspace") {
        if (query.length === 0) continue
        query = query.slice(0, -1)
        active = 0
        scheduleReload()
        continue
      }
      if (key.str !== undefined && !key.ctrl && key.str.length === 1) {
        query += key.str
        active = 0
        scheduleReload()
      }
    }
  })

  if (outcome === undefined) {
    invalidate()
    throw new ExitPromptError()
  }
  invalidate()
  finishPrompt(stdout, config.message, outcome.name)
  return outcome.value
}