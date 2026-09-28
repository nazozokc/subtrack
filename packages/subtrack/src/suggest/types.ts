/**
 * Types for the suggestion system.
 */

export type {
  Suggestion,
  SuggestionSource,
  SuggestionStatus,
} from "../types.ts"

/** Raw email input read from a local file. */
export type RawEmail = {
  id: string
  from: string | null
  subject: string | null
  date: Date | null
  textBody: string
}

/** Parsed suggestion candidate before DB insertion. */
export type SuggestionCandidate = {
  name: string
  price: number | null
  currency: string | null
  cycle: string | null
  vendorName?: string | null
  vendorUrl?: string | null
  planTier?: string | null
  paymentMethod?: string | null
  source: string
  sourceDetail?: string | null
  confidence: number
}

/** Options for the suggest command. */
export type SuggestListFlags = {
  all?: boolean
  json?: boolean
}

export type SuggestDismissFlags = {
  all?: boolean
}
