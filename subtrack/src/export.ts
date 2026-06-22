import type { SharedArgs } from "./types.ts"
import { formatPrice } from "./display.ts"

/**
 * Escape a CSV field: wrap in quotes if it contains comma, double-quote, or newline.
 * Double-quotes inside the field are escaped as "".
 */
function escapeCsv(value: string): string {
  // CSV injection prevention: prefix dangerous leading characters with tab
  // to prevent spreadsheet formula execution (DDE, =, +, -, @)
  if (/^[=+\-@\t]/.test(value)) {
    value = "\t" + value
  }
  if (value.includes('"') || value.includes(",") || value.includes("\n")) {
    return `"${value.replace(/"/g, '""')}"`
  }
  return value
}

export function exportCsv(subs: SharedArgs[]): string {
  const header = "name,cycle,tags,price,currency"
  const rows = subs.map((s) => {
    const tags = s.tags.map((t) => escapeCsv(t)).join(";")
    const name = escapeCsv(s.name)
    return `${name},${s.cycle},${tags},${s.price},${s.currency}`
  })
  // BOM for Excel compatibility
  return "\uFEFF" + [header, ...rows].join("\n")
}

export function exportJson(subs: SharedArgs[]): string {
  return JSON.stringify(subs, null, 2) + "\n"
}

function escapeMdCell(value: string): string {
  // Escape backslash first, then pipe, and normalize newline/carriage-return characters
  // to prevent Markdown table structure breakage.
  return value
    .replace(/\\/g, "\\\\")
    .replace(/\|/g, "\\|")
    .replace(/[\r\n]+/g, " ")
}

export function exportMd(subs: SharedArgs[]): string {
  const header = "| name | cycle | tags | price | currency |"
  const separator = "| --- | --- | --- | --- | --- |"
  const rows = subs.map((s) => {
    const tags = s.tags.map(escapeMdCell).join(", ") || "-"
    const price = formatPrice(s.price, s.currency)
    return `| ${escapeMdCell(s.name)} | ${s.cycle} | ${tags} | ${price} | ${s.currency} |`
  })
  return [header, separator, ...rows].join("\n")
}
