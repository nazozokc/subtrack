import type { SharedArgs } from "./types.ts"
import { formatPrice } from "./display.ts"
import ExcelJS from "exceljs"
import { calculateNextBilling } from "./upcoming.ts"

function escapeCsv(value: string): string {
  if (/^[=+\-@\t]/.test(value)) {
    value = "\t" + value
  }
  if (value.includes('"') || value.includes(",") || value.includes("\n")) {
    return `"${value.replace(/"/g, '""')}"`
  }
  return value
}

export function exportCsv(subs: SharedArgs[]): string {
  const header = "name,cycle,tags,price,currency,notes"
  const rows = subs.map((s) => {
    const tags = s.tags.map((t) => escapeCsv(t)).join(";")
    const name = escapeCsv(s.name)
    const notes = escapeCsv(s.notes ?? "")
    return `${name},${s.cycle},${tags},${s.price},${s.currency},${notes}`
  })
  return "\uFEFF" + [header, ...rows].join("\n")
}

export function exportJson(subs: SharedArgs[]): string {
  return JSON.stringify(subs, null, 2) + "\n"
}

function escapeMdCell(value: string): string {
  return value
    .replace(/\\/g, "\\\\")
    .replace(/\|/g, "\\|")
    .replace(/[\r\n]+/g, " ")
}

export function exportMd(subs: SharedArgs[]): string {
  const header = "| name | cycle | tags | price | currency | notes |"
  const separator = "| --- | --- | --- | --- | --- | --- |"
  const rows = subs.map((s) => {
    const tags = s.tags.map(escapeMdCell).join(", ") || "-"
    const price = formatPrice(s.price, s.currency)
    const notes = escapeMdCell(s.notes ?? "") || "-"
    return `| ${escapeMdCell(s.name)} | ${s.cycle} | ${tags} | ${price} | ${s.currency} | ${notes} |`
  })
  return [header, separator, ...rows].join("\n")
}

export async function exportExcel(subs: SharedArgs[]): Promise<Buffer> {
  const workbook = new ExcelJS.Workbook()
  const sheet = workbook.addWorksheet("Subscriptions")

  const headers = [
    "ID",
    "Name",
    "Price",
    "Currency",
    "Cycle",
    "Status",
    "Billing Day",
    "Payment Method",
    "Tags",
    "Notes",
    "Created At",
  ]

  const headerRow = sheet.addRow(headers)
  headerRow.eachCell((cell) => {
    cell.font = { bold: true, color: { argb: "FFFFFFFF" } }
    cell.fill = {
      type: "pattern",
      pattern: "solid",
      fgColor: { argb: "FF4472C4" },
    }
  })

  for (const s of subs) {
    const tags = s.tags.join(", ")
    sheet.addRow([
      s.id,
      s.name,
      s.price,
      s.currency,
      s.cycle,
      s.status,
      s.billingDay ?? "",
      s.paymentMethod ?? "",
      tags,
      s.notes ?? "",
      s.createdAt,
    ])
  }

  sheet.columns.forEach((column) => {
    let maxLength = 0
    if (column.eachCell) {
      column.eachCell({ includeEmpty: true }, (cell) => {
        const val = cell.value?.toString() ?? ""
        maxLength = Math.max(maxLength, val.length)
      })
    }
    column.width = Math.min(Math.max(maxLength + 2, 10), 50)
  })

  const buf = await workbook.xlsx.writeBuffer()
  return Buffer.from(buf)
}

function icsEscape(value: string): string {
  return value
    .replace(/\\/g, "\\\\")
    .replace(/;/g, "\\;")
    .replace(/,/g, "\\,")
    .replace(/\r?\n/g, "\\n")
}

function icsFold(line: string): string {
  const lines: string[] = []
  for (let i = 0; i < line.length; i += 75) {
    if (i === 0) {
      lines.push(line.slice(i, i + 75))
    } else {
      lines.push(" " + line.slice(i, i + 74))
    }
  }
  return lines.join("\r\n")
}

function icsFormatDate(d: Date): string {
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, "0")
  const day = String(d.getDate()).padStart(2, "0")
  return `${y}${m}${day}`
}

function cycleToRrule(cycle: SharedArgs["cycle"]): string {
  switch (cycle) {
    case "weekly":
      return "FREQ=WEEKLY"
    case "bi-weekly":
      return "FREQ=WEEKLY;INTERVAL=2"
    case "monthly":
      return "FREQ=MONTHLY"
    case "quarterly":
      return "FREQ=MONTHLY;INTERVAL=3"
    case "semi-annual":
      return "FREQ=MONTHLY;INTERVAL=6"
    case "yearly":
      return "FREQ=YEARLY"
  }
}

export function exportIcs(subs: SharedArgs[]): string {
  const now = new Date()
  now.setHours(0, 0, 0, 0)
  const nowStr = icsFormatDate(now)

  let cal = [
    "BEGIN:VCALENDAR",
    "VERSION:2.0",
    "PRODID:-//subtrack//subtrack//EN",
  ]

  for (const s of subs) {
    if (s.status === "cancelled") continue

    const nextDate = calculateNextBilling(s, now)
    const dtStart = icsFormatDate(nextDate)
    const uid = `${s.id}-${dtStart}@subtrack`
    const tags = s.tags.length > 0 ? `Tags: ${s.tags.join(", ")}` : ""
    const notes = s.notes ? `Notes: ${s.notes}` : ""
    const descParts = [tags, notes].filter(Boolean)
    const description = descParts.length > 0 ? descParts.join("\\n") : ""
    const summary = `${s.name} - ${formatPrice(s.price, s.currency)}/${s.cycle}`

    cal.push("BEGIN:VEVENT")
    cal.push(icsFold(`DTSTART;VALUE=DATE:${dtStart}`))
    cal.push(icsFold(`RRULE:${cycleToRrule(s.cycle)}`))
    cal.push(icsFold(`SUMMARY:${icsEscape(summary)}`))
    if (description) {
      cal.push(icsFold(`DESCRIPTION:${icsEscape(description)}`))
    }
    cal.push(icsFold(`DTSTAMP:${nowStr}T000000`))
    cal.push(icsFold(`UID:${uid}`))
    cal.push("END:VEVENT")
  }

  cal.push("END:VCALENDAR")
  return cal.join("\r\n") + "\r\n"
}
