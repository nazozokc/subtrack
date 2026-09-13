/**
 * Minimal XLSX generator — writes an Excel-compatible .xlsx buffer using
 * hand-built ZIP and inline strings (no shared strings table needed).
 * Zero runtime dependencies.
 */

import { deflateRawSync } from "node:zlib"

// ── CRC32 ─────────────────────────────────────────────────

const CRC_TABLE = new Uint32Array(256)
for (let n = 0; n < 256; n++) {
  let c = n
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1
  CRC_TABLE[n] = c >>> 0
}

function crc32(buf: Buffer): number {
  let c = 0xffffffff
  for (const b of buf) c = CRC_TABLE[(c ^ b) & 0xff] ^ (c >>> 8)
  return (c ^ 0xffffffff) >>> 0
}

// ── ZIP primitives ─────────────────────────────────────────

function utf8(s: string): Buffer { return Buffer.from(s, "utf-8") }
function u16(v: number): Buffer { const b = Buffer.allocUnsafe(2); b.writeUInt16LE(v); return b }
function u32(v: number): Buffer { const b = Buffer.allocUnsafe(4); b.writeUInt32LE(v); return b }

const DOS_TIME = 0x0000   // 00:00:00
const DOS_DATE = 0x5821   // 2024-01-01

function localHeader(name: string, crc: number, comp: number, uncomp: number): Buffer {
  return Buffer.concat([
    u32(0x04034b50),
    u16(20),           // version needed
    u16(0),            // flags
    u16(8),            // method: deflate
    u16(DOS_TIME), u16(DOS_DATE),
    u32(crc), u32(comp), u32(uncomp),
    u16(utf8(name).length), u16(0),
    utf8(name),
  ])
}

function centralDir(name: string, crc: number, comp: number, uncomp: number, offset: number): Buffer {
  return Buffer.concat([
    u32(0x02014b50),
    u16(20),           // version made by (host=unix)
    u16(20),           // version needed
    u16(0),            // flags
    u16(8),            // method: deflate
    u16(DOS_TIME), u16(DOS_DATE),
    u32(crc), u32(comp), u32(uncomp),
    u16(utf8(name).length), u16(0), u16(0), u16(0), u16(0),
    u32(0),            // external attrs
    u32(offset),
    utf8(name),
  ])
}

function eocd(count: number, cdSize: number, cdOffset: number): Buffer {
  return Buffer.concat([
    u32(0x06054b50),
    u16(0), u16(0), u16(count), u16(count),
    u32(cdSize), u32(cdOffset), u16(0),
  ])
}

// ── XML helpers ────────────────────────────────────────────

function esc(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;")
}

/** Convert a 1-based column index to an Excel column letter: 1→A … 26→Z, 27→AA. */
function colLetter(n: number): string {
  let s = ""
  while (n > 0) {
    const rem = (n - 1) % 26
    s = String.fromCharCode(65 + rem) + s
    n = Math.floor((n - 1) / 26)
  }
  return s
}

// ── XLSX generation ────────────────────────────────────────

export type XlsxOptions = {
  headers: readonly string[]
  rows: readonly (string | number)[][]
  columnWidths: readonly number[]
  sheetName?: string
}

export function generateXlsx(opts: XlsxOptions): Buffer {
  const { headers, rows, columnWidths, sheetName = "Subscriptions" } = opts
  const allData = [headers, ...rows]

  const parts: { name: string; data: string }[] = []

  // [Content_Types].xml
  parts.push({
    name: "[Content_Types].xml",
    data:
      `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n` +
      `<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">` +
      `<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>` +
      `<Default Extension="xml" ContentType="application/xml"/>` +
      `<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>` +
      `<Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>` +
      `<Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>` +
      `</Types>`,
  })

  // _rels/.rels
  parts.push({
    name: "_rels/.rels",
    data:
      `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n` +
      `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">` +
      `<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>` +
      `</Relationships>`,
  })

  // xl/workbook.xml
  parts.push({
    name: "xl/workbook.xml",
    data:
      `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n` +
      `<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">` +
      `<sheets><sheet name="${esc(sheetName)}" sheetId="1" r:id="rId1"/></sheets></workbook>`,
  })

  // xl/_rels/workbook.xml.rels
  parts.push({
    name: "xl/_rels/workbook.xml.rels",
    data:
      `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n` +
      `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">` +
      `<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>` +
      `<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>` +
      `</Relationships>`,
  })

  // xl/styles.xml — 2 XFs: 0=normal, 1=header (bold white on blue)
  parts.push({
    name: "xl/styles.xml",
    data:
      `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n` +
      `<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">` +
      `<fonts count="2">` +
      `<font><sz val="11"/><name val="Calibri"/></font>` +
      `<font><b/><color rgb="FFFFFFFF"/><sz val="11"/><name val="Calibri"/></font>` +
      `</fonts>` +
      `<fills count="3">` +
      `<fill><patternFill patternType="none"/></fill>` +
      `<fill><patternFill patternType="gray125"/></fill>` +
      `<fill><patternFill patternType="solid"><fgColor rgb="FF4472C4"/><bgColor indexed="64"/></patternFill></fill>` +
      `</fills>` +
      `<borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders>` +
      `<cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>` +
      `<cellXfs count="2">` +
      `<xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/>` +
      `<xf numFmtId="0" fontId="1" fillId="2" borderId="0" xfId="0" applyFont="1" applyFill="1"/>` +
      `</cellXfs>` +
      `</styleSheet>`,
  })

  // xl/worksheets/sheet1.xml
  const colsDef = columnWidths
    .map((w, i) => `<col min="${i + 1}" max="${i + 1}" width="${Math.round(w)}.0" customWidth="1"/>`)
    .join("")

  const rowsXml = allData.map((row, r) => {
    const isHeader = r === 0
    const cells = row.map((val, c) => {
      const ref = `${colLetter(c + 1)}${r + 1}`
      const xf = isHeader ? ' s="1"' : ""
      if (typeof val === "number") {
        return `<c r="${ref}"${xf}><v>${val}</v></c>`
      }
      return `<c r="${ref}" t="inlineStr"${xf}><is><t>${esc(String(val ?? ""))}</t></is></c>`
    })
    return `<row r="${r + 1}">${cells.join("")}</row>`
  })

  parts.push({
    name: "xl/worksheets/sheet1.xml",
    data:
      `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n` +
      `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">` +
      `<cols>${colsDef}</cols>` +
      `<sheetData>${rowsXml.join("")}</sheetData>` +
      `<sheetViews><sheetView workbookViewId="0"/></sheetViews>` +
      `</worksheet>`,
  })

  // ── Build ZIP archive ──────────────────────────────────────
  const bufs: Buffer[] = []
  const entries: { crc: number; comp: number; uncomp: number; offset: number; name: string }[] = []

  for (const part of parts) {
    const raw = utf8(part.data)
    const comp = deflateRawSync(raw)
    const crc = crc32(raw)
    const offset = bufs.reduce((sum, b) => sum + b.length, 0)
    bufs.push(localHeader(part.name, crc, comp.length, raw.length), comp)
    entries.push({ name: part.name, crc, comp: comp.length, uncomp: raw.length, offset })
  }

  const cdOffset = bufs.reduce((sum, b) => sum + b.length, 0)
  const cdBufs = entries.map((e) => centralDir(e.name, e.crc, e.comp, e.uncomp, e.offset))
  const cdSize = cdBufs.reduce((sum, b) => sum + b.length, 0)
  bufs.push(...cdBufs)
  bufs.push(eocd(entries.length, cdSize, cdOffset))

  return Buffer.concat(bufs)
}