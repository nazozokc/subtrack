use regex_lite::Regex;

use super::common::{Candidate, RawEmail, SimplePattern, amount_ok, parse_num, strip_tags};

// ── Credit card parser (port of `credit-card.ts`) ────────────────────────

pub(super) const CARD_PATTERNS: &[SimplePattern] = &[
    SimplePattern {
        label: "Rakuten Card",
        detect: r"(?i)楽天カード|Rakuten Card",
        name_re: r"(?:ご利用先|加盟店)[：:]\s*(.+)",
        amount_re: r"(?:ご利用金額|お支払金額)[：:]\s*[¥￥]?\s*([0-9,]+)",
    },
    SimplePattern {
        label: "SMBC Card",
        detect: r"(?i)三井住友|Vpass|SMBC Card",
        name_re: r"(?:ご利用先|加盟店名)[：:]\s*(.+)",
        amount_re: r"(?:ご利用金額|お支払金額|ご利用額)[：:]\s*[¥￥]?\s*([0-9,]+)",
    },
    SimplePattern {
        label: "JCB",
        detect: r"(?i)JCB|ジェーシービー",
        name_re: r"(?:ご利用先|ショップ名)[：:]\s*(.+)",
        amount_re: r"(?:ご利用金額|お支払金額)[：:]\s*[¥￥]?\s*([0-9,]+)",
    },
    SimplePattern {
        label: "American Express",
        detect: r"(?i)American Express|AMEX|アメックス",
        name_re: r"(?i)(?:Merchant|商家?)[：:]\s*(.+)",
        amount_re: r"(?i)(?:Amount|金額)[：:]\s*[¥$￥€£]?\s*([0-9,]+(?:\.[0-9]{1,2})?)",
    },
    SimplePattern {
        label: "Credit Card",
        detect: r"(?i)VISA|MasterCard|マスターカード|クレジットカード",
        name_re: r"(?i)(?:ご利用先|加盟店|Merchant|Vendor)[：:]\s*(.+)",
        amount_re: r"(?i)(?:ご利用金額|金額|Amount)[：:]\s*[¥￥$€£]?\s*([0-9,]+(?:\.[0-9]{1,2})?)",
    },
];

pub(super) fn parse_credit_card(email: &RawEmail) -> Option<Candidate> {
    let text = format!("{}\n{}", email.subject.as_deref().unwrap_or(""), email.body);
    let symbol_re = Regex::new(r"[¥$€£]").unwrap();
    for p in CARD_PATTERNS {
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
        let currency_symbol = symbol_re
            .find(&am[0])
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        let currency = match currency_symbol.as_str() {
            "$" => "USD",
            "¥" => "JPY",
            "€" => "EUR",
            "£" => "GBP",
            _ => "JPY",
        }
        .to_string();
        let raw = parse_num(&am[1]);
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
            source_detail: format!("Credit card notification ({})", p.label),
            confidence: 0.8,
        });
    }
    None
}
