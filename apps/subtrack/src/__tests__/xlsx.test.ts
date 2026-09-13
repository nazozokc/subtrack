import { describe, test, expect } from "vitest"
import { inflateRawSync } from "node:zlib"
import { generateXlsx } from "@subtrack/lib/xlsx"
import { exportExcel } from "../export.ts"
import type { SharedArgs } from "../types.ts"

/** Read the stored data of every zip entry by walking local headers. */
function readParts(buf: Buffer): { name: string; data: string }[] {
  const parts: { name: string; data: string }[] = []
  let offset = 0
  while (offset < buf.length) {
    const sig = buf.readUInt32LE(offset)
    if (sig !== 0x04034b50) break // central directory follows
    const nameLen = buf.readUInt16LE(offset + 26)
    const extraLen = buf.readUInt16LE(offset + 28)
    const compSize = buf.readUInt32LE(offset + 18)
    const name = buf.subarray(offset + 30, offset + 30 + nameLen).toString("utf8")
    const comp = buf.subarray(offset + 30 + nameLen + extraLen, offset + 30 + nameLen + extraLen + compSize)
    parts.push({ name, data: inflateRawSync(comp).toString("utf8") })
    offset += 30 + nameLen + extraLen + compSize
  }
  return parts
}

describe("generateXlsx", () => {
  const buf = generateXlsx({
    headers: ["ID", "Name", "Price", "Currency"],
    rows: [
      [1, "Netflix", 1900, "JPY"],
      [2, "GitHub Copilot", 10, "USD"],
    ],
    columnWidths: [10, 20, 12, 12],
  })

  test("produces a valid zip archive", () => {
    expect(buf.subarray(0, 4)).toEqual(Buffer.from([0x50, 0x4b, 0x03, 0x04]))
    expect(readParts(buf).map((p) => p.name)).toEqual([
      "[Content_Types].xml",
      "_rels/.rels",
      "xl/workbook.xml",
      "xl/_rels/workbook.xml.rels",
      "xl/styles.xml",
      "xl/worksheets/sheet1.xml",
    ])
  })

  test("sheet1 contains header row with style and inline strings", () => {
    const sheet = readParts(buf).find((p) => p.name === "xl/worksheets/sheet1.xml")!.data
    expect(sheet).toContain('<c r="A1" t="inlineStr" s="1"><is><t>ID</t></is></c>')
    expect(sheet).toContain('<c r="B2" t="inlineStr"><is><t>Netflix</t></is></c>')
    expect(sheet).toContain('<c r="C2"><v>1900</v></c>')
    expect(sheet).toContain('<row r="3">')
  })

  test("styles.xml declares header style (bold, white on blue)", () => {
    const styles = readParts(buf).find((p) => p.name === "xl/styles.xml")!.data
    expect(styles).toContain("<b/>")
    expect(styles).toContain('<color rgb="FFFFFFFF"/>')
    expect(styles).toContain('<fgColor rgb="FF4472C4"/>')
  })

  test("workbook references sheet rId1 and the Subscriptions sheet name", () => {
    const wb = readParts(buf).find((p) => p.name === "xl/workbook.xml")!.data
    expect(wb).toContain('<sheet name="Subscriptions" sheetId="1" r:id="rId1"/>')
  })
})

describe("exportExcel", () => {
  test("produces a workbook buffer from subscriptions", () => {
    const sub: SharedArgs = {
      id: 1,
      name: "Netflix",
      price: 1900,
      currency: "JPY",
      cycle: "monthly",
      status: "active",
      billingDay: 1,
      tags: ["video"],
      createdAt: "2024-01-01",
      notes: null,
      paymentMethod: null,
      contractStart: null,
      contractEnd: null,
      autoRenewal: true,
      vendorName: null,
      vendorUrl: null,
      planTier: null,
      discountAmount: null,
      discountType: null,
    }
    const buf = exportExcel([sub])
    const parts = readParts(buf)
    const sheet = parts.find((p) => p.name === "xl/worksheets/sheet1.xml")!.data
    expect(sheet).toContain("<t>Netflix</t>")
    expect(sheet).toContain("<t>video</t>")
  })
})