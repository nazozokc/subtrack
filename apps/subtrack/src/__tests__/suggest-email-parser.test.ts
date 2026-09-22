import { test, expect, describe } from "vitest"
import { parseEmailContent } from "../suggest/email-parser.ts"

describe("suggest email-parser: parseEmailContent", () => {
  test("treats plain text as body with null metadata", () => {
    const raw = "Monthly charge: $9.99 for premium access."
    const result = parseEmailContent(raw, "abc")
    expect(result.id).toBe("abc")
    expect(result.from).toBeNull()
    expect(result.subject).toBeNull()
    expect(result.date).toBeNull()
    expect(result.textBody).toBe(raw)
  })

  test("parses EML headers (From / Subject / Date / body)", () => {
    const raw = [
      "From: Netflix <info@netflix.com>",
      "Subject: Your receipt",
      "Date: Mon, 15 Jun 2026 10:00:00 +0900",
      "",
      "Payment received. Your subscription continues.",
    ].join("\n")
    const result = parseEmailContent(raw, "1")
    expect(result.id).toBe("1")
    expect(result.from).toBe("Netflix <info@netflix.com>")
    expect(result.subject).toBe("Your receipt")
    expect(result.date?.toISOString()).toBe("2026-06-15T01:00:00.000Z")
    expect(result.textBody).toBe("Payment received. Your subscription continues.")
  })

  test("decodes MIME encoded-word subject (UTF-8 base64)", () => {
    const raw = [
      "From: x@example.com",
      "Subject: =?UTF-8?B?TmV0ZmxpeA==?=",
      "Date: Mon, 15 Jun 2026 10:00:00 +0900",
      "",
      "Body",
    ].join("\n")
    const result = parseEmailContent(raw, "2")
    expect(result.subject).toBe("Netflix")
  })

  test("decodes MIME encoded-word subject (ISO-2022-JP base64)", () => {
    const raw = [
      "From: x@example.com",
      "Subject: =?ISO-2022-JP?B?TmV0ZmxpeA==?=",
      "Date: Mon, 15 Jun 2026 10:00:00 +0900",
      "",
      "Body",
    ].join("\n")
    const result = parseEmailContent(raw, "3")
    expect(result.subject).toBe("Netflix")
  })

  test("decodes MIME encoded-word subject (Q encoding)", () => {
    const raw = [
      "From: x@example.com",
      "Subject: =?UTF-8?Q?Netflix_real_charge?=",
      "",
      "Body",
    ].join("\n")
    const result = parseEmailContent(raw, "4")
    expect(result.subject).toBe("Netflix real charge")
  })

  test("extracts text/plain part from multipart body", () => {
    const raw = [
      "From: x@example.com",
      "Subject: Multi-part receipt",
      'Content-Type: multipart/alternative; boundary="BOUNDARY"',
      "",
      "--BOUNDARY",
      "Content-Type: text/html; charset=UTF-8",
      "",
      "<p>HTML version</p>",
      "--BOUNDARY",
      "Content-Type: text/plain; charset=UTF-8",
      "",
      "Thank you for your payment of $9.99. Your subscription is active.",
      "--BOUNDARY--",
    ].join("\n")
    const result = parseEmailContent(raw, "5")
    expect(result.subject).toBe("Multi-part receipt")
    expect(result.textBody).toBe("Thank you for your payment of $9.99. Your subscription is active.")
  })

  test("decodes base64-encoded body", () => {
    const encoded = Buffer.from("Your subscription has been renewed.", "utf-8").toString("base64")
    const raw = [
      "From: x@example.com",
      "Subject: Base64 receipt",
      "",
      "Content-Type: text/plain; charset=UTF-8",
      "Content-Transfer-Encoding: base64",
      "",
      encoded,
    ].join("\n")
    const result = parseEmailContent(raw, "6")
    expect(result.textBody).toBe("Your subscription has been renewed.")
  })

  test("decodes quoted-printable body (=FF hex escapes)", () => {
    const raw = [
      "From: x@example.com",
      "Subject: QP receipt",
      "",
      "Your caf=E9 total was =2410 per month.",
    ].join("\n")
    const result = parseEmailContent(raw, "7")
    expect(result.textBody).toBe("Your café total was $10 per month.")
  })

  test("joins header continuation lines with a space", () => {
    const raw = [
      "From: Netflix <info@netflix.com>",
      "Subject: Your",
      " subscription receipt",
      "Date: Mon, 15 Jun 2026 10:00:00 +0900",
      "",
      "Body",
    ].join("\n")
    const result = parseEmailContent(raw, "8")
    expect(result.from).toBe("Netflix <info@netflix.com>")
    expect(result.subject).toBe("Your subscription receipt")
  })

  test("returns null date for unparseable Date header", () => {
    const raw = [
      "From: x@example.com",
      "Subject: Broken date",
      "Date: not-a-real-date",
      "",
      "Body",
    ].join("\n")
    const result = parseEmailContent(raw, "9")
    expect(result.subject).toBe("Broken date")
    expect(result.date).toBeNull()
  })
})