//! Port of `suggest/parser/*.ts` — the `parseEmail` pipeline.
//!
//! `parse_email` tries the parsers in order (bank → credit card → service
//! receipt → wallet → generic) and returns the first non-null candidate.
//! JavaScript's `String.match(re)` semantics (first match, capture groups)
//! map to `Regex::captures`; `String.replace(re, "")` (non-global) maps to
//! `Regex::replace`. Word boundaries use `\b`, which is close
//! to JS for the ASCII words used by these parsers.
//!
//! The engine is `regex-lite` instead of the `regex` crate: every pattern in
//! this module (literals, `(?i)`, `\d`/`\s`, character classes, `{m,n}`
//! quantifiers, alternation, non-greedy repetition) is fully supported by
//! regex-lite and its engine is far lighter in the shipped binary. JavaScript
//! `\b` is ASCII-only while regex-lite's boundary derives from `\w`, so the
//! `guess_cycle` guards that mix Japanese words with `\b` are kept out of
//! boundary-sensitive positions (verified byte-exact against the golden
//! fixtures).

mod bank;
mod common;
mod credit_card;
mod generic;
mod receipt;
mod wallet;

#[cfg(test)]
mod tests;

pub use common::{Candidate, RawEmail};

/// Port of `parseEmail`: try each parser in order, first non-null wins.
pub fn parse_email(email: &RawEmail) -> Option<Candidate> {
    bank::parse_bank(email)
        .or_else(|| credit_card::parse_credit_card(email))
        .or_else(|| receipt::parse_receipt_service(email))
        .or_else(|| wallet::parse_wallet(email))
        .or_else(|| generic::parse_generic(email))
}
