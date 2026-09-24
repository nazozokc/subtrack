import { test, expect } from "vitest"
import type { RawEmail, SuggestionCandidate } from "../suggest/types.ts"
import { parseBankEmail } from "../suggest/parser/bank.ts"
import { parseCreditCardEmail } from "../suggest/parser/credit-card.ts"
import { parseGenericEmail } from "../suggest/parser/generic.ts"
import { parseEmail } from "../suggest/parser/index.ts"

function email(body: string, subject: string | null = null): RawEmail {
  return { id: "1", from: "bank@example.com", subject, date: new Date(), textBody: body }
}

// ── Bank parser ──────────────────────────────────────────

test("bank: parses SMBC withdrawal emails", () => {
  const result = parseBankEmail(email("三井住友銀行からお知らせです。\nご利用先：Netflix\nご利用金額：¥1,990"))
  expect(result).not.toBeNull()
  expect(result?.name).toBe("Netflix")
  expect(result?.price).toBe(1990)
  expect(result?.currency).toBe("JPY")
  expect(result?.cycle).toBe("monthly")
  expect(result?.confidence).toBeCloseTo(0.85)
  expect(result?.sourceDetail).toContain("SMBC")
})

test("bank: parses Mizuho and Rakuten Bank emails", () => {
  const mizuho = parseBankEmail(email("みずほ銀行 お引落し先：Amazon\n引落：¥500"))
  expect(mizuho?.name).toBe("Amazon")
  expect(mizuho?.price).toBe(500)

  const rakuten = parseBankEmail(email("楽天銀行 明細：DAZN\n引落金額：¥2,700"))
  expect(rakuten?.name).toBe("DAZN")
  expect(rakuten?.price).toBe(2700)
})

test("bank: parses Yucho, PayPay and generic bank emails", () => {
  const yucho = parseBankEmail(email("ゆうちょ銀行\n払込先：NHK\n払込金額：¥1,300"))
  expect(yucho?.name).toBe("NHK")
  expect(yucho?.price).toBe(1300)

  const paypay = parseBankEmail(email("PayPay銀行 ご利用先：ChatGPT\n金額：¥3,000"))
  expect(paypay?.name).toBe("ChatGPT")
  expect(paypay?.price).toBe(3000)

  const genericBank = parseBankEmail(email("お引き落としのお知らせ ご利用先：Notion\nご利用金額：¥1,200"))
  expect(genericBank?.name).toBe("Notion")
  expect(genericBank?.price).toBe(1200)
  expect(genericBank?.sourceDetail).toContain("Generic Bank")
})

test("bank: strips HTML-like tags from the merchant name", () => {
  const result = parseBankEmail(email("みずほ銀行 お引落し先：<b>Amazon</b>\n引落：¥500"))
  expect(result?.name).toBe("Amazon")
})

test("bank: returns null for non-bank emails and missing fields", () => {
  expect(parseBankEmail(email("hello world, nothing to see here"))).toBeNull()
  // Amount present but no merchant name
  expect(parseBankEmail(email("みずほ銀行 引落金額：¥500"))).toBeNull()
})

// ── Credit card parser ───────────────────────────────────

test("card: parses Rakuten Card and SMBC Vpass emails at face value (JPY)", () => {
  const rakuten = parseCreditCardEmail(email("楽天カード ご利用先：ユニクロ\nご利用金額：¥3,990"))
  expect(rakuten?.name).toBe("ユニクロ")
  expect(rakuten?.price).toBe(3990)
  expect(rakuten?.currency).toBe("JPY")
  expect(rakuten?.confidence).toBeCloseTo(0.8)

  const vpass = parseCreditCardEmail(email("Vpass ご利用先：ローソン\nご利用金額：¥800"))
  expect(vpass?.name).toBe("ローソン")
  expect(vpass?.price).toBe(800)
})

test("card: parses JCB usage notifications", () => {
  const result = parseCreditCardEmail(email("JCBカードご利用のお知らせ ご利用先：セブンイレブン\nご利用金額：¥700"))
  expect(result?.name).toBe("セブンイレブン")
  expect(result?.price).toBe(700)
})

test("card: converts foreign currency amounts to minor units", () => {
  const usd = parseCreditCardEmail(email("American Express\nMerchant: Apple\nAmount: $19.99"))
  expect(usd?.name).toBe("Apple")
  expect(usd?.currency).toBe("USD")
  expect(usd?.price).toBe(1999)

  const eur = parseCreditCardEmail(email("American Express\nMerchant: Spotify\nAmount: €12.50"))
  expect(eur?.currency).toBe("EUR")
  expect(eur?.price).toBe(1250)
})

test("card: generic Visa/Mastercard pattern with symbol-prefixed amounts", () => {
  const visa = parseCreditCardEmail(email("VISAカードご利用のお知らせ\n加盟店：Amazon\n金額：$49.00"))
  expect(visa?.name).toBe("Amazon")
  expect(visa?.currency).toBe("USD")
  expect(visa?.price).toBe(4900)
})

test("card: returns null for non-card emails", () => {
  expect(parseCreditCardEmail(email("just a plain reminder"))).toBeNull()
})

// ── Generic fallback parser ──────────────────────────────

test("generic: extracts name, price and cycle from an unknown email", () => {
  const result = parseGenericEmail(
    email("Total: $9.99 monthly\nYour SomeCo invoice has arrived", "Your SomeCo invoice"),
  )
  expect(result).not.toBeNull()
  expect(result?.name).toBe("SomeCo")
  expect(result?.price).toBe(999) // $9.99 → cents
  expect(result?.currency).toBe("USD")
  expect(result?.cycle).toBe("monthly")
  expect(result?.confidence).toBeCloseTo(0.4)
})

test("generic: returns null when no price is found", () => {
  expect(parseGenericEmail(email("no amounts anywhere"))).toBeNull()
})

// ── Router (parseEmail) ──────────────────────────────────

test("router: dispatches to the bank parser for bank emails", () => {
  const result = parseEmail(email("みずほ銀行 お引落し先：Amazon 引落金額：¥500"))
  expect(result?.name).toBe("Amazon")
  expect(result?.confidence).toBeCloseTo(0.85)
})

test("router: dispatches to the card parser for card emails", () => {
  const result = parseEmail(email("VISAカードご利用のお知らせ\n加盟店：Amazon\n金額：$49.00"))
  expect(result?.name).toBe("Amazon")
  expect(result?.currency).toBe("USD")
  expect(result?.confidence).toBeCloseTo(0.8)
})

test("router: falls back to generic parsing", () => {
  const result = parseEmail(email("Total: $9.99 monthly", "Your SomeCo invoice"))
  expect(result?.name).toBe("SomeCo")
  expect(result?.confidence).toBeCloseTo(0.4)
})

test("router: returns null for unrecognizable emails", () => {
  expect(parseEmail(email("nothing here at all"))).toBeNull()
})

// Sanity: every parser result matches the candidate shape
function assertCandidateShape(c: SuggestionCandidate | null): void {
  expect(typeof c?.name).toBe("string")
  expect(typeof c?.price).toBe("number")
  expect(typeof c?.currency).toBe("string")
  expect(typeof c?.confidence).toBe("number")
  expect(c?.source).toBe("email")
}

test("candidates always carry the expected shape", () => {
  assertCandidateShape(parseBankEmail(email("楽天銀行 明細：DAZN 引落金額：¥2,700")))
  assertCandidateShape(parseCreditCardEmail(email("楽天カード ご利用先：ユニクロ ご利用金額：¥3,990")))
  assertCandidateShape(parseGenericEmail(email("Total: $9.99 monthly", "Your SomeCo invoice")))
})