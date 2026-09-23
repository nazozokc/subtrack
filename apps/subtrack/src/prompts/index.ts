/**
 * Self-contained prompt implementations (`@inquirer/prompts` replacement).
 * Zero runtime dependencies — only Node built-ins.
 */
export { input } from "./input.ts"
export type { InputConfig } from "./input.ts"
export { confirm } from "./confirm.ts"
export type { ConfirmConfig } from "./confirm.ts"
export { select, checkbox } from "./select.ts"
export type { SelectConfig, CheckboxConfig } from "./select.ts"
export { search } from "./search.ts"
export type { SearchConfig } from "./search.ts"
export type { Choice, PromptStreams } from "./core.ts"