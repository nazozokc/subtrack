/**
 * Choice-list logic shared by `select` and `checkbox`: normalization of the
 * loose `Choices<T>` input, query filtering for typeahead, and the visible
 * window math for paged rendering. Pure functions — no I/O.
 */

import { stripAnsi } from "./ansi.ts"
import type { Choice } from "./core.ts"

/** A choice may be a plain value (used as both name and value) or a full `{name, value}` object. */
export type Choices<T> = ReadonlyArray<Choice<T> | T>

export type ListItem<T> = { choice: Choice<T>; index: number }

export function normalizeChoices<T>(choices: Choices<T>): Choice<T>[] {
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

export function filterItems<T>(choices: Choice<T>[], query: string): ListItem<T>[] {
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

/**
 * Slide a `pageSize`-sized window over `count` items so `active` stays
 * visible. Returns inclusive-exclusive [start, end).
 */
export function visibleWindow(active: number, count: number, pageSize: number): { start: number; end: number } {
  const visible = Math.min(pageSize, count)
  const start =
    count <= visible ? 0 : Math.min(Math.max(active - visible + 1, 0), count - visible)
  return { start, end: start + visible }
}