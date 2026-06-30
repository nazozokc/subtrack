import { consola } from "consola"
import pc from "picocolors"
import { getSubscriptions } from "./db.ts"
import { formatPrice } from "./display.ts"
import type { SharedArgs } from "./types.ts"

export type CalendarOptions = {
  month?: number
  year?: number
  json?: boolean
}

export type CalendarEntry = {
  day: number
  subs: { name: string; price: number; currency: string; status: string; id: number }[]
}

function daysInMonth(year: number, month: number): number {
  return new Date(year, month, 0).getDate()
}

function clampDay(day: number, year: number, month: number): number {
  return Math.min(day, daysInMonth(year, month))
}

export function calcCalendarEntries(month: number, year: number): CalendarEntry[] {
  const subs = getSubscriptions()
  const active = subs.filter((s) => s.status !== "cancelled" && s.billingDay != null)

  const dayMap = new Map<number, CalendarEntry["subs"]>()

  for (const sub of active) {
    const day = clampDay(sub.billingDay!, year, month)
    if (!dayMap.has(day)) {
      dayMap.set(day, [])
    }
    dayMap.get(day)!.push({
      name: sub.name,
      price: sub.price,
      currency: sub.currency,
      status: sub.status,
      id: sub.id,
    })
  }

  const entries: CalendarEntry[] = []
  for (const [day, subs_] of dayMap) {
    entries.push({ day, subs: subs_ })
  }
  entries.sort((a, b) => a.day - b.day)

  return entries
}

const MONTH_NAMES = [
  "January", "February", "March", "April", "May", "June",
  "July", "August", "September", "October", "November", "December",
]

export function showCalendar(options: CalendarOptions): void {
  const now = new Date()
  const rawMonth = options.month ?? now.getMonth() + 1
  const rawYear = options.year ?? now.getFullYear()

  const month = rawMonth >= 1 && rawMonth <= 12 ? rawMonth : now.getMonth() + 1
  const year = rawYear >= 1 ? rawYear : now.getFullYear()

  if (options.json) {
    const entries = calcCalendarEntries(month, year)
    process.stdout.write(JSON.stringify(entries, null, 2) + "\n")
    return
  }

  const entries = calcCalendarEntries(month, year)
  const entryMap = new Map(entries.map((e) => [e.day, e.subs]))

  consola.log(pc.bold(`      ${MONTH_NAMES[month - 1]} ${year}`))
  consola.log(pc.dim(" Su Mo Tu We Th Fr Sa"))

  const firstDay = new Date(year, month - 1, 1).getDay()
  const totalDays = daysInMonth(year, month)

  let line = ""
  for (let i = 0; i < firstDay; i++) {
    line += "   "
  }

  let eventCount = 0
  for (let day = 1; day <= totalDays; day++) {
    const dayEntries = entryMap.get(day)
    if (dayEntries) {
      const n = dayEntries.length
      eventCount += n
      const label = n > 1 ? `${String(day).padStart(1)}${superscript(n)}` : String(day).padStart(2)
      line += ` ${pc.green(pc.bold(label))}`
    } else {
      line += ` ${String(day).padStart(2)}`
    }

    if ((firstDay + day) % 7 === 0) {
      consola.log(line)
      line = ""
    }
  }

  if (line !== "") {
    consola.log(line)
  }

  consola.log("")

  if (entries.length === 0) {
    consola.info("No billing events this month")
    return
  }

  consola.log(pc.bold("Billing events:"))
  consola.log("")

  const currencyTotals: Record<string, number> = {}
  let totalSubs = 0

  for (const entry of entries) {
    for (const sub of entry.subs) {
      const statusStyle =
        sub.status === "active" ? pc.green : sub.status === "paused" ? pc.yellow : pc.dim
      consola.log(
        `  ${pc.cyan(`Day ${String(entry.day).padStart(2)}`)}  ${pc.bold(sub.name)}  ${formatPrice(sub.price, sub.currency)}  ${statusStyle(sub.status)}`,
      )
      currencyTotals[sub.currency] = (currencyTotals[sub.currency] ?? 0) + sub.price
      totalSubs++
    }
  }

  if (totalSubs > 0) {
    consola.log("")
    const parts = Object.entries(currencyTotals)
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([ccy, total]) => formatPrice(Math.round(total), ccy))
    consola.log(`  ${pc.bold("Total:")} ${parts.join(" + ")} (${totalSubs} event${totalSubs > 1 ? "s" : ""})`)
  }
}

function superscript(n: number): string {
  const supMap: Record<string, string> = {
    "0": "\u2070", "1": "\u00B9", "2": "\u00B2", "3": "\u00B3",
    "4": "\u2074", "5": "\u2075", "6": "\u2076", "7": "\u2077",
    "8": "\u2078", "9": "\u2079",
  }
  return String(n).split("").map((c) => supMap[c] ?? c).join("")
}
