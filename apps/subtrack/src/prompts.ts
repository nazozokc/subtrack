import { checkbox, confirm, input, search, select } from "./prompts/index.ts"
import { fail } from "./error.ts"
import { isValidCycle, validateCycleDays, CUSTOM_CYCLE, CUSTOM_CYCLE_CHOICE, CYCLE_CHOICES } from "./validation.ts"
import type { Cycle } from "./types.ts"

// Pure validators and choice constants live in validation.ts so
// non-interactive command paths never load the prompt machinery.
export {
  isValidCurrency, isValidCycle, validateCycleDays, isValidStatus,
  validateBillingDay, validateName, validatePrice, validateNotes,
  validatePaymentMethod, validateDateString, validateVendorName,
  validateVendorUrl, validatePlanTier, validateDiscountValue,
  validateDiscountType, validateAutoRenewal, validateTrialName,
  validateExpiresAt, validateTags, validateTokens, validateDate,
  validateModelName,
  CURRENCY_CHOICES, STATUS_CHOICES, CUSTOM_CYCLE, CUSTOM_CYCLE_CHOICE,
  CYCLE_CHOICES,
} from "./validation.ts"

export { checkbox, confirm, input, search, select }
export type { Choice, PromptStreams } from "./prompts/index.ts"

export const LLM_PROVIDER_CHOICES: { name: string; value: string }[] = [
  { name: "OpenAI", value: "openai" },
  { name: "Anthropic", value: "anthropic" },
  { name: "Google AI (Gemini)", value: "google-ai" },
  { name: "Mistral AI", value: "mistral" },
  { name: "Groq", value: "groq" },
  { name: "Together AI", value: "together" },
  { name: "DeepSeek", value: "deepseek" },
  { name: "Cohere", value: "cohere" },
  { name: "Other...", value: "__other__" },
]

export async function promptString(
  flag: string | undefined,
  message: string,
  validate: (v: string) => string | true,
): Promise<{ value: string; prompted: boolean } | null> {
  if (flag !== undefined) {
    const result = validate(flag)
    if (result !== true) {
      fail(result)
      return null
    }
    return { value: flag, prompted: false }
  }
  return { value: await input({ message, validate }), prompted: true }
}

export function validChoices<T>(choices: { value: T }[]): string {
  return choices.map((c) => c.value).join(", ")
}

export async function promptSelect<T extends string>(
  flag: string | undefined,
  message: string,
  choices: { name: string; value: T }[],
  isValid: (v: string) => v is T,
): Promise<{ value: T; prompted: boolean } | null> {
  if (flag !== undefined) {
    if (!isValid(flag)) {
      fail(`Invalid "${flag}". Valid: ${validChoices(choices)}`)
      return null
    }
    return { value: flag, prompted: false }
  }
  return { value: await select({ message, choices }), prompted: true }
}

/**
 * Prompt for a billing cycle: pick a named preset, or choose the custom
 * entry and type a day count (e.g. 3 → cycle "3d" for a 3-day trial cycle).
 * Flag values follow the same rule and accept "Nd" directly.
 */
export async function promptCycle(
  flag: string | undefined,
  message = "cycle",
  io?: { stdin?: NodeJS.ReadStream; stdout?: NodeJS.WriteStream },
): Promise<{ value: Cycle; prompted: boolean } | null> {
  if (flag !== undefined) {
    if (!isValidCycle(flag)) {
      fail(`Invalid cycle "${flag}". Valid: ${validChoices(CYCLE_CHOICES)}, or "Nd" (days)`)
      return null
    }
    return { value: flag, prompted: false }
  }
  const choice = await select<Cycle | typeof CUSTOM_CYCLE>({
    message,
    choices: [...CYCLE_CHOICES, CUSTOM_CYCLE_CHOICE],
    ...io,
  })
  if (choice !== CUSTOM_CYCLE) return { value: choice, prompted: true }
  const days = await input({ message: "days per cycle (e.g. 3):", validate: validateCycleDays, ...io })
  return { value: `${Number(days.trim())}d`, prompted: true }
}