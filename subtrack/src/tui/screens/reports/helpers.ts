import { OCCURRENCES_PER_YEAR, type Cycle } from "../../../types.ts"

export function monthlyFactor(cycle: string): number {
  return (OCCURRENCES_PER_YEAR[cycle as Cycle] ?? 12) / 12
}

export function yearlyFactor(cycle: string): number {
  return OCCURRENCES_PER_YEAR[cycle as Cycle] ?? 12
}

export function computeNextBill(day: number, from: Date, until: Date): Date | null {
  const year = from.getFullYear()
  const month = from.getMonth()
  const lastDay = new Date(year, month + 1, 0).getDate()
  const clampedDay = Math.min(day, lastDay)
  let d = new Date(year, month, clampedDay)
  if (d < from) {
    const nextLastDay = new Date(year, month + 2, 0).getDate()
    d = new Date(year, month + 1, Math.min(day, nextLastDay))
  }
  if (d <= until) return d
  return null
}
