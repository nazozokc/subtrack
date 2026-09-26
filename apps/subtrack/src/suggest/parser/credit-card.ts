/**
 * Credit card usage email parser.
 *
 * Targets:
 * - Rakuten Card (楽天カード)
 * - SMBC Card (三井住友カード)
 * - JCB
 * - American Express
 * - Visa/Mastercard generic
 * - Other Japanese credit card notifications
 */

import type { RawEmail, SuggestionCandidate } from "../types.ts"

type CardPattern = {
  name: string
  detect: RegExp
  namePattern: RegExp
  amountPattern: RegExp
}

const CARD_PATTERNS: CardPattern[] = [
  // Rakuten Card
  {
    name: "Rakuten Card",
    detect: /楽天カード|Rakuten Card/i,
    namePattern: /(?:ご利用先|加盟店)[：:]\s*(.+)/,
    amountPattern: /(?:ご利用金額|お支払金額)[：:]\s*[¥￥]?\s*([0-9,]+)/,
  },
  // SMBC Card / Vpass
  {
    name: "SMBC Card",
    detect: /三井住友|Vpass|SMBC Card/i,
    namePattern: /(?:ご利用先|加盟店名)[：:]\s*(.+)/,
    amountPattern: /(?:ご利用金額|お支払金額|ご利用額)[：:]\s*[¥￥]?\s*([0-9,]+)/,
  },
  // JCB
  {
    name: "JCB",
    detect: /JCB|ジェーシービー/i,
    namePattern: /(?:ご利用先|ショップ名)[：:]\s*(.+)/,
    amountPattern: /(?:ご利用金額|お支払金額)[：:]\s*[¥￥]?\s*([0-9,]+)/,
  },
  // American Express
  {
    name: "American Express",
    detect: /American Express|AMEX|アメックス/i,
    namePattern: /(?:Merchant|商家?)[：:]\s*(.+)/i,
    amountPattern: /(?:Amount|金額)[：:]\s*[¥$￥€£]?\s*([0-9,]+(?:\.[0-9]{1,2})?)/i,
  },
  // Generic Visa/Mastercard
  {
    name: "Credit Card",
    detect: /VISA|MasterCard|マスターカード|クレジットカード/i,
    namePattern: /(?:ご利用先|加盟店|Merchant|Vendor)[：:]\s*(.+)/i,
    amountPattern: /(?:ご利用金額|金額|Amount)[：:]\s*[¥￥$€£]?\s*([0-9,]+(?:\.[0-9]{1,2})?)/i,
  },
]

/**
 * Parse a credit card usage notification email.
 * Returns a high-confidence suggestion if patterns match.
 */
export function parseCreditCardEmail(email: RawEmail): SuggestionCandidate | null {
  const text = `${email.subject ?? ""}\n${email.textBody}`

  for (const pattern of CARD_PATTERNS) {
    if (!pattern.detect.test(text)) continue

    const nameMatch = text.match(pattern.namePattern)
    const amountMatch = text.match(pattern.amountPattern)

    if (!nameMatch || !amountMatch) continue

    let name = nameMatch[1].trim()
    // Repeatedly strip HTML-like tags to prevent bypass via nested tags
    let prev: string
    do {
      prev = name
      name = name.replace(/<[^>]+>/g, "")
    } while (name !== prev)
    name = name.trim()
    if (!name || name.length > 100) continue

    const rawAmount = amountMatch[1].replace(/,/g, "")
    // Determine currency from symbol in the matched text
    const currencySymbol = amountMatch[0].match(/[¥$€£]/)?.[0] ?? ""
    const currencyMap: Record<string, string> = { "$": "USD", "¥": "JPY", "€": "EUR", "£": "GBP" }
    const currency = currencySymbol ? (currencyMap[currencySymbol] ?? "JPY") : "JPY"
    // Subscription prices use major currency units (e.g. 19.99 USD).
    const amount = currency === "JPY"
      ? Math.round(Number(rawAmount))
      : Number(rawAmount)

    if (isNaN(amount) || amount <= 0 || amount > 99999999) continue

    return {
      name,
      price: amount,
      currency,
      cycle: "monthly",
      source: "email",
      sourceDetail: `Credit card notification (${pattern.name})`,
      confidence: 0.8,
    }
  }

  return null
}
