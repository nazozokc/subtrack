/**
 * Self-contained `select` and `checkbox` prompts. Raw-mode arrow-key
 * navigation with optional typeahead filtering (select), space-toggling
 * (checkbox), pageSize scrolling, and loop/wrap control.
 */

import {
  CHECKED,
  ExitPromptError,
  POINTER,
  UNCHECKED,
  Renderer,
  cyan,
  dim,
  finishPrompt,
  resolveStreams,
  stripAnsi,
  truncate,
  withRawMode,
} from "./core.ts"
import type { Choice, PromptStreams } from "./core.ts"

/** A choice may be a plain value (used as both name and value) or a full `{name, value}` object. */
export type Choices<T> = ReadonlyArray<Choice<T> | T>

export type SelectConfig<T> = PromptStreams & {
  message: string
  choices: Choices<T>
  loop?: boolean
  pageSize?: number
  default?: NoInfer<T>
}

export type CheckboxConfig<T> = PromptStreams & {
  message: string
  choices: Choices<T>
  loop?: boolean
  pageSize?: number
}

const DEFAULT_PAGE_SIZE = 7

type ListItem<T> = { choice: Choice<T>; index: number }

function normalizeChoices<T>(choices: Choices<T>): Choice<T>[] {
  return choices.map((item) => {
    if (typeof item === "object" && item !== null && "value" in item) {
      return {
        value: item.value,
        name: item.name ?? String(item.value),
        description: (item as Choice<T>).description,
      } as Choice<T>
    }
    return { value: item as T, name: String(item) }
  })
}

function filterItems<T>(choices: Choice<T>[], query: string): ListItem<T>[] {
  const q = query.trim().toLowerCase()
  if (!q) return choices.map((choice, index) => ({ choice, index }))
  return choices
    .map((choice, index) => ({ choice, index }))
    .filter(({ choice }) => {
      const name = (choice.name ?? "").toLowerCase()
      const description = choice.description
        ? stripAnsi(choice.description).toLowerCase()
        : ""
      return name.includes(q) || description.includes(q)
    })
}

function renderList<T>(
  message: string,
  items: ListItem<T>[],
  opts: {
    active: number
    checked: ReadonlySet<number>
    multi: boolean
    pageSize: number
    columns: number
  },
): string {
  const { active, checked, multi, pageSize, columns } = opts
  const visible = Math.min(pageSize, items.length)
  const start =
    items.length <= visible
      ? 0
      : Math.min(Math.max(active - visible + 1, 0), items.length - visible)

  const lines: string[] = [`\r${cyan("?")} ${message}`]
  if (items.length === 0) {
    lines.push(`  ${dim("No matching choices")}`)
    return lines.join("\n")
  }
  for (let i = start; i < start + visible && i < items.length; i++) {
    const item = items[i]!
    const pointer = i === active ? cyan(POINTER) : " "
    const row = multi
      ? `${pointer} ${checked.has(item.index) ? cyan(CHECKED) : dim(UNCHECKED)} ${item.choice.name}`
      : `${pointer} ${item.choice.name}${item.choice.description ? ` ${dim(item.choice.description)}` : ""}`
    lines.push(truncate(row, columns))
  }
  return lines.join("\n")
}

function resolveColumns(config: PromptStreams): number {
  const stdout = config.stdout ?? process.stdout
  return ((stdout as { columns?: number }).columns ?? 80) - 2
}

async function runList<T>(
  config: {
    message: string
    choices: Choice<T>[]
    loop: boolean
    pageSize: number
    columns: number
    initialIndex: number
    stdin: NodeJS.ReadStream
    stdout: NodeJS.WriteStream
  },
  multi: boolean,
): Promise<
  | { eof: true; selected: T[]; names: string[] }
  | { eof: false; selected: T[]; names: string[] }
> {
  const { stdin, stdout } = config
  const renderer = new Renderer(stdout)
  let query = ""
  let items = filterItems(config.choices, query)
  let active = Math.min(config.initialIndex, Math.max(items.length - 1, 0))
  const checked = new Set<number>()

  function clampActive() {
    if (items.length === 0) active = 0
    else if (active >= items.length) active = items.length - 1
  }

  function move(direction: -1 | 1) {
    if (items.length === 0) return
    active += direction
    if (active < 0) active = config.loop ? items.length - 1 : 0
    else if (active >= items.length) active = config.loop ? 0 : items.length - 1
  }

  function render() {
    renderer.render(
      renderList(config.message, items, {
        active,
        checked,
        multi,
        pageSize: config.pageSize,
        columns: config.columns,
      }),
    )
  }

  const outcome = await withRawMode(stdin, stdout, async ({ readKey }) => {
    for (;;) {
      render()
      const key = await readKey()
      if (key.name === "ctrl-c" || key.name === "escape") {
        renderer.clear()
        throw new ExitPromptError()
      }
      if (key.name === "eof") {
        renderer.clear()
        return { eof: true as const, selected: [], names: [] }
      }
      if (key.name === "up" || key.name === "down") {
        move(key.name === "up" ? -1 : 1)
        continue
      }
      if (key.name === "return") {
        renderer.clear()
        if (multi) {
          const selected = config.choices
            .map((choice, index) => ({ choice, index }))
            .filter(({ index }) => checked.has(index))
          return {
            eof: false as const,
            selected: selected.map(({ choice }) => choice.value),
            names: selected.map(({ choice }) => stripAnsi(choice.name ?? "")),
          }
        }
        const item = items[active]
        if (!item) continue
        return {
          eof: false as const,
          selected: [item.choice.value],
          names: [stripAnsi(item.choice.name ?? "")],
        }
      }
      if (key.name === " " && multi) {
        const item = items[active]
        if (item) {
          if (checked.has(item.index)) checked.delete(item.index)
          else checked.add(item.index)
        }
        continue
      }
      if (key.name === "backspace") {
        query = query.slice(0, -1)
        items = filterItems(config.choices, query)
        clampActive()
        continue
      }
      if (key.str !== undefined && !key.ctrl && key.str.length === 1) {
        query += key.str
        items = filterItems(config.choices, query)
        active = 0
        clampActive()
      }
    }
  })

  return outcome
}

export async function select<const T>(config: SelectConfig<T>): Promise<T> {
  const { stdin, stdout, interactive } = resolveStreams(config)
  const choices = normalizeChoices(config.choices)
  if (!interactive) {
    const choice = choices.find((c) => c.value === config.default)
    if (choice) return choice.value
    throw new ExitPromptError()
  }

  const initialIndex = choices.findIndex((c) => c.value === config.default)
  const { eof, selected, names } = await runList<T>(
    {
      message: config.message,
      choices,
      loop: config.loop !== false,
      pageSize: config.pageSize ?? DEFAULT_PAGE_SIZE,
      columns: resolveColumns(config),
      initialIndex: initialIndex === -1 ? 0 : initialIndex,
      stdin,
      stdout,
    },
    false,
  )
  if (eof) {
    const choice = choices.find((c) => c.value === config.default)
    if (choice) return choice.value
    throw new ExitPromptError()
  }
  const value = selected[0] as T | undefined
  if (value === undefined) throw new ExitPromptError()
  finishPrompt(stdout, config.message, names[0] ?? "")
  return value
}

export async function checkbox<const T>(config: CheckboxConfig<T>): Promise<T[]> {
  const { stdin, stdout, interactive } = resolveStreams(config)
  if (!interactive) throw new ExitPromptError()

  const { eof, selected, names } = await runList<T>(
    {
      message: config.message,
      choices: normalizeChoices(config.choices),
      loop: config.loop !== false,
      pageSize: config.pageSize ?? DEFAULT_PAGE_SIZE,
      columns: resolveColumns(config),
      initialIndex: 0,
      stdin,
      stdout,
    },
    true,
  )
  if (eof) throw new ExitPromptError()
  finishPrompt(stdout, config.message, names.length > 0 ? names.join(", ") : "none")
  return selected
}