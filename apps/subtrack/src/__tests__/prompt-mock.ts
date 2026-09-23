/**
 * Shared vi.mock factory for `../prompts.ts`.
 *
 * Returns the real module spread with the five prompt functions replaced by
 * `vi.fn()` instances, plus `promptString`/`promptSelect` replicas that route
 * through the mocked `input`/`select` (so interactive handler flows can be
 * driven from tests the same way they were with `@inquirer/prompts`).
 */
import { vi } from "vitest"
import { fail } from "../error.ts"
import type * as PromptModule from "../prompts.ts"

export function createPromptMock(actual: typeof PromptModule) {
  const input = vi.fn()
  const select = vi.fn()
  const confirm = vi.fn()
  const checkbox = vi.fn()
  const search = vi.fn()
  return {
    ...actual,
    input,
    select,
    confirm,
    checkbox,
    search,
    promptString: vi.fn(
      async (
        flag: string | undefined,
        message: string,
        validate: (v: string) => string | true,
      ) => {
        if (flag !== undefined) {
          const result = validate(flag)
          if (result !== true) {
            fail(result)
            return null
          }
          return { value: flag, prompted: false }
        }
        return { value: await input({ message, validate }), prompted: true }
      },
    ),
    promptSelect: vi.fn(
      async <T extends string>(
        flag: string | undefined,
        message: string,
        choices: { name: string; value: T }[],
        isValid: (v: string) => v is T,
      ) => {
        if (flag !== undefined) {
          if (!isValid(flag)) {
            fail(`Invalid "${flag}". Valid: ${actual.validChoices(choices)}`)
            return null
          }
          return { value: flag, prompted: false }
        }
        return { value: await select({ message, choices }), prompted: true }
      },
    ),
  }
}