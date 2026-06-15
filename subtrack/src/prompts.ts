import { input, select } from "@inquirer/prompts"
import { consola } from "consola"
import type { Currency, Cycle } from "./db.ts"

export type { Currency, Cycle }

export const CURRENCY_CHOICES: { name: string; value: Currency }[] = [
  { name: "JPY (日本円)", value: "JPY" },
  { name: "USD (US Dollar)", value: "USD" },
  { name: "EUR (Euro)", value: "EUR" },
  { name: "GBP (British Pound)", value: "GBP" },
  { name: "AUD (Australian Dollar)", value: "AUD" },
  { name: "CAD (Canadian Dollar)", value: "CAD" },
  { name: "KRW (South Korean Won)", value: "KRW" },
  { name: "CNY (Chinese Yuan)", value: "CNY" },
  { name: "SGD (Singapore Dollar)", value: "SGD" },
  { name: "HKD (Hong Kong Dollar)", value: "HKD" },
]

export const CYCLE_CHOICES: { name: string; value: Cycle }[] = [
  { name: "weekly", value: "weekly" },
  { name: "bi-weekly", value: "bi-weekly" },
  { name: "monthly", value: "monthly" },
  { name: "quarterly", value: "quarterly" },
  { name: "semi-annual", value: "semi-annual" },
  { name: "yearly", value: "yearly" },
]

export function isValidCurrency(v: string): v is Currency {
  return CURRENCY_CHOICES.some((c) => c.value === v)
}

export function isValidCycle(v: string): v is Cycle {
  return CYCLE_CHOICES.some((c) => c.value === v)
}

export function validateName(v: string): string | true {
  if (!v.trim()) return "Name cannot be empty"
  if (v.length > 100) return "Name too long (max 100 chars)"
  return true
}

export function validatePrice(v: string): string | true {
  if (!v.trim()) return "Please enter a valid number"
  if (isNaN(Number(v)) || Number(v) < 0)
    return "Please enter a valid non-negative number"
  if (Number(v) > 99999999) return "Price too high (max 99,999,999)"
  return true
}

export function validateTags(v: string): string | true {
  if (!v.trim()) return true
  const tags = v.split(",").map((t) => t.trim()).filter(Boolean)
  if (tags.length > 10) return "Maximum 10 tags allowed"
  for (const tag of tags) {
    if (tag.length > 50) return `Tag too long: "${tag}" (max 50 chars)`
  }
  return true
}

export async function promptString(
  flag: string | undefined,
  message: string,
  validate: (v: string) => string | true,
): Promise<{ value: string; prompted: boolean } | null> {
  if (flag !== undefined) {
    const result = validate(flag)
    if (result !== true) {
      consola.error(result)
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
      consola.error(`Invalid "${flag}". Valid: ${validChoices(choices)}`)
      return null
    }
    return { value: flag, prompted: false }
  }
  return { value: await select({ message, choices }), prompted: true }
}
