use regex_lite::Regex;

use super::common::{Candidate, RawEmail, amount_ok, parse_num};

// ── Service receipt parser (port of `parser/receipt.ts`) ─────────────────

pub(super) struct ReceiptPattern {
    pub(super) label: &'static str,
    pub(super) detect: &'static str,
    pub(super) name_re: &'static str,
    pub(super) amount_re: &'static str,
    pub(super) cycle_re: &'static str,
}

pub(super) const RECEIPT_PATTERNS: &[ReceiptPattern] = &[
    ReceiptPattern {
        label: "Netflix",
        detect: r"(?i)netflix",
        name_re: r"(?i)Netflix",
        amount_re: r"(?i)(?:Amount|Total|Charged|Price)[^0-9]*([$¥€£]?)\s*([0-9]+(?:\.[0-9]{1,2})?)",
        cycle_re: r"(?i)month|monthly",
    },
    ReceiptPattern {
        label: "Spotify",
        detect: r"(?i)spotify",
        name_re: r"(?i)Spotify",
        amount_re: r"(?i)(?:Amount|Total|Price)[^0-9]*([$¥€£]?)\s*([0-9]+(?:\.[0-9]{1,2})?)",
        cycle_re: r"(?i)month|monthly",
    },
    ReceiptPattern {
        label: "Amazon",
        detect: r"(?i)amazon",
        name_re: r"(?i)Amazon\s+(Prime|Music|Kindle|AWS)?",
        amount_re: r"(?i)(?:Amount|Total|Charged|Price|金額)[^0-9]*([$¥€£]?)\s*([0-9]+(?:\.[0-9]{1,2})?)",
        cycle_re: r"(?i)(?:month|annual|year)",
    },
    ReceiptPattern {
        label: "Apple",
        detect: r"(?i)apple|icloud|app\s*store",
        name_re: r"(?i)(?:Apple|iCloud|Apple\s+Music|Apple\s+TV|Apple\s+Arcade|Apple\s+One|App\s+Store)",
        amount_re: r"(?i)(?:Amount|Total|Charged|Price|金額)[^0-9]*([$¥€£]?)\s*([0-9]+(?:\.[0-9]{1,2})?)",
        cycle_re: r"(?i)(?:month|annual|year)",
    },
    ReceiptPattern {
        label: "Google",
        detect: r"(?i)google",
        name_re: r"(?i)(?:Google\s+One|YouTube\s+(?:Premium|Music)|Google\s+Workspace|Google\s+Play)",
        amount_re: r"(?i)(?:Amount|Total|Charged|Price|金額)[^0-9]*([$¥€£]?)\s*([0-9]+(?:\.[0-9]{1,2})?)",
        cycle_re: r"(?i)(?:month|annual|year)",
    },
    ReceiptPattern {
        label: "Adobe",
        detect: r"(?i)adobe",
        name_re: r"(?i)Adobe\s+(Creative\s+Cloud|Photoshop|Illustrator|Lightroom|Acrobat|Premiere)",
        amount_re: r"(?i)(?:Amount|Total|Charged|Price|小計|合計)[^0-9]*([$¥€£]?)\s*([0-9]+(?:\.[0-9]{1,2})?)",
        cycle_re: r"(?i)(?:month|annual|year)",
    },
    ReceiptPattern {
        label: "Microsoft",
        detect: r"(?i)microsoft|office\s*365|m365|ms\s*365",
        name_re: r"(?i)(?:Microsoft\s+365|Office\s+365|Microsoft\s+Azure|Xbox|Microsoft\s+Teams)",
        amount_re: r"(?i)(?:Amount|Total|Charged|Price)[^0-9]*([$¥€£]?)\s*([0-9]+(?:\.[0-9]{1,2})?)",
        cycle_re: r"(?i)(?:month|annual|year)",
    },
    ReceiptPattern {
        label: "Notion",
        detect: r"(?i)notion",
        name_re: r"(?i)Notion",
        amount_re: r"(?i)(?:Amount|Total|Charged|Price)[^0-9]*([$¥€£]?)\s*([0-9]+(?:\.[0-9]{1,2})?)",
        cycle_re: r"(?i)(?:month|annual|year)",
    },
    ReceiptPattern {
        label: "Slack",
        detect: r"(?i)slack",
        name_re: r"(?i)Slack",
        amount_re: r"(?i)(?:Amount|Total|Charged|Price)[^0-9]*([$¥€£]?)\s*([0-9]+(?:\.[0-9]{1,2})?)",
        cycle_re: r"(?i)(?:month|annual|year)",
    },
    ReceiptPattern {
        label: "GitHub",
        detect: r"(?i)github",
        name_re: r"(?i)GitHub\s+(Pro|Team|Enterprise)?",
        amount_re: r"(?i)(?:Amount|Total|Charged|Price)[^0-9]*([$¥€£]?)\s*([0-9]+(?:\.[0-9]{1,2})?)",
        cycle_re: r"(?i)(?:month|annual|year)",
    },
    ReceiptPattern {
        label: "Figma",
        detect: r"(?i)figma",
        name_re: r"(?i)Figma",
        amount_re: r"(?i)(?:Amount|Total|Charged|Price)[^0-9]*([$¥€£]?)\s*([0-9]+(?:\.[0-9]{1,2})?)",
        cycle_re: r"(?i)(?:month|annual|year)",
    },
];

pub(super) const RECEIPT_KEYWORDS: &str = r"(?i)receipt|invoice|payment confirmation|charge|billing|subscription|purchase|receipt|領収書|請求書|支払|購入|ご利用ありがとう|Thank you for your (purchase|payment|order)";

pub(super) fn parse_receipt_service(email: &RawEmail) -> Option<Candidate> {
    let subject = email.subject.as_deref().unwrap_or("");
    let text = format!("{}\n{}", subject, email.body);

    for p in RECEIPT_PATTERNS {
        let detect = Regex::new(p.detect).ok()?;
        if !detect.is_match(&text) {
            continue;
        }
        // Emulate `amazon(?! co uk)(?!\.co\.jp)`: Rust's regex crate has no
        // look-ahead, so require at least one "amazon" not followed by the
        // excluded country suffixes.
        if p.label == "Amazon" {
            let exc = Regex::new(r"(?i)^(?: co uk|\.co\.jp)").ok()?;
            let any_ok = detect
                .find_iter(&text)
                .any(|m| !exc.is_match(&text[m.end()..]));
            if !any_ok {
                continue;
            }
        }
        let name_re = Regex::new(p.name_re).ok()?;
        let amount_re = Regex::new(p.amount_re).ok()?;
        let name_match = name_re.find(&text);
        let Some(amount_match) = amount_re.captures(&text) else {
            continue;
        };
        let name = match name_match {
            Some(m) => m.as_str().trim().to_string(),
            None => p.label.to_string(),
        };
        if name.len() > 100 {
            continue;
        }
        let symbol = amount_match.get(1).map(|m| m.as_str()).unwrap_or("");
        let currency = match symbol {
            "$" => "USD",
            "¥" => "JPY",
            "€" => "EUR",
            "£" => "GBP",
            _ => "USD",
        }
        .to_string();
        let raw = parse_num(&amount_match[2]);
        let amount = if currency == "JPY" { raw.round() } else { raw };
        if amount.is_nan() || !amount_ok(amount) {
            continue;
        }
        let cycle_re = Regex::new(p.cycle_re).ok()?;
        let cycle = if cycle_re.is_match(&text) {
            Some("monthly".to_string())
        } else {
            None
        };
        return Some(Candidate {
            name,
            price: amount,
            currency,
            cycle,
            source: "email".to_string(),
            source_detail: format!("Service receipt ({})", p.label),
            confidence: 0.85,
        });
    }

    // Generic receipt detection: defer to the generic parser.
    let kw = Regex::new(RECEIPT_KEYWORDS).ok()?;
    let body_head: String = email.body.chars().take(500).collect();
    if kw.is_match(subject) || kw.is_match(&body_head) {
        return None;
    }
    None
}
