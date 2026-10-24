use regex_lite::Regex;

use super::common::{Candidate, RawEmail, SimplePattern, amount_ok, parse_num, strip_tags};

// ── Wallet parser (port of `wallet.ts`) ──────────────────────────────────

pub(super) const WALLET_PATTERNS: &[SimplePattern] = &[
    SimplePattern {
        label: "PayPal",
        detect: r"(?i)paypal",
        name_re: r"(?i)(?:You sent a payment to|Payment to|Sent to|送金先|支払先)[^0-9]*([A-Za-z0-9\s\.]+?)(?:\s*$|\s*(?:for|から|に|$))",
        amount_re: r"(?i)(?:Amount|Total|Payment|金額|合計)[^0-9]*([$¥€£]?)\s*([0-9]+(?:\.[0-9]{1,2})?)",
    },
    SimplePattern {
        label: "Apple Pay",
        detect: r"(?i)Apple\s*Pay",
        name_re: r"(?i)(?:Merchant|商家?|Business)[：:]\s*(.+)",
        amount_re: r"(?i)(?:Amount|Total|金額)[：:]?\s*([$¥€£]?)\s*([0-9]+(?:\.[0-9]{1,2})?)",
    },
    SimplePattern {
        label: "Google Pay",
        detect: r"(?i)Google\s*Pay",
        name_re: r"(?i)(?:Merchant|商家?|Paid to)[：:]\s*(.+)",
        amount_re: r"(?i)(?:Amount|Total|金額)[：:]?\s*([$¥€£]?)\s*([0-9]+(?:\.[0-9]{1,2})?)",
    },
    SimplePattern {
        label: "Stripe",
        detect: r"(?i)stripe",
        name_re: r"(?i)(?:Merchant|商家?|Business|Amount to)[：:]\s*(.+)",
        amount_re: r"(?i)(?:Amount|Total|金額)[：:]?\s*([$¥€£]?)\s*([0-9]+(?:\.[0-9]{1,2})?)",
    },
];

pub(super) fn parse_wallet(email: &RawEmail) -> Option<Candidate> {
    let text = format!("{}\n{}", email.subject.as_deref().unwrap_or(""), email.body);
    for p in WALLET_PATTERNS {
        let detect = Regex::new(p.detect).ok()?;
        if !detect.is_match(&text) {
            continue;
        }
        let name_re = Regex::new(p.name_re).ok()?;
        let amount_re = Regex::new(p.amount_re).ok()?;
        let (Some(nm), Some(am)) = (name_re.captures(&text), amount_re.captures(&text)) else {
            continue;
        };
        let name = strip_tags(&nm[1]);
        if name.is_empty() || name.len() > 100 {
            continue;
        }
        let symbol = am.get(1).map(|m| m.as_str()).unwrap_or("");
        let currency = match symbol {
            "$" => "USD",
            "¥" => "JPY",
            "€" => "EUR",
            "£" => "GBP",
            _ => "USD",
        }
        .to_string();
        let raw = parse_num(&am[2]);
        let amount = if currency == "JPY" { raw.round() } else { raw };
        if amount.is_nan() || !amount_ok(amount) {
            continue;
        }
        return Some(Candidate {
            name,
            price: amount,
            currency,
            cycle: Some("monthly".to_string()),
            source: "email".to_string(),
            source_detail: format!("Wallet notification ({})", p.label),
            confidence: 0.7,
        });
    }
    None
}
