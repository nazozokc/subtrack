use super::bank::BANK_PATTERNS;
use super::credit_card::CARD_PATTERNS;
use super::receipt::{RECEIPT_KEYWORDS, RECEIPT_PATTERNS};
use super::wallet::WALLET_PATTERNS;
use regex_lite::Regex;

use super::*;

/// Every static pattern must compile: the parser functions use `.ok()?`,
/// so a non-compiling pattern would silently disable a whole parser.
#[test]
fn all_patterns_compile() {
    let mut patterns: Vec<&str> = Vec::new();
    for p in BANK_PATTERNS
        .iter()
        .chain(CARD_PATTERNS)
        .chain(WALLET_PATTERNS)
    {
        patterns.push(p.detect);
        patterns.push(p.name_re);
        patterns.push(p.amount_re);
    }
    for p in RECEIPT_PATTERNS {
        patterns.push(p.detect);
        patterns.push(p.name_re);
        patterns.push(p.amount_re);
        patterns.push(p.cycle_re);
    }
    patterns.push(RECEIPT_KEYWORDS);
    for pat in patterns {
        assert!(
            Regex::new(pat).is_ok(),
            "pattern failed to compile: {}",
            pat
        );
    }
}

/// Regression: a full bank-receipt email without recognizable field
/// labels must not yield any candidate ("Streamly" receipt shape).
#[test]
fn receipt_txt_fixture_yields_no_candidate() {
    let body = [
        "From: billing@example-bank.com",
        "To: user@example.com",
        "Subject: Your receipt from Streamly",
        "Date: Mon, 5 Oct 2026 09:14:22 +0000",
        "",
        "Streamly Premium Monthly",
        "Receipt #: R-2026-1005-4471",
        "",
        "Plan: Premium (Monthly)",
        "Billing period: Oct 5, 2026 - Nov 4, 2026",
        "Amount charged: USD 12.99",
        "Payment method: Visa ending in 4242",
        "",
        "Thank you for your payment.",
        "",
    ]
    .join("\n");
    let email = RawEmail {
        subject: None,
        from: None,
        body,
    };
    assert!(parse_email(&email).is_none());
}
