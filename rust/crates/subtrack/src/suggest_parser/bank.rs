use regex_lite::Regex;

use super::common::{Candidate, RawEmail, SimplePattern, amount_ok, parse_num, strip_tags};

// ── Bank withdrawal parser (port of `bank.ts`) ───────────────────────────

pub(super) const BANK_PATTERNS: &[SimplePattern] = &[
    SimplePattern {
        label: "SMBC",
        detect: r"(?i)三井住友|SMBC",
        name_re: r"ご利用先[：:]\s*(.+)",
        amount_re: r"ご利用金額[：:]\s*[¥￥]?\s*([0-9,]+)",
    },
    SimplePattern {
        label: "Mizuho",
        detect: r"(?i)みずほ|Mizuho",
        name_re: r"お引落し(?:先|内訳)[：:]\s*(.+)",
        amount_re: r"(?:お支払|引落)[金額]?[：:]\s*[¥￥]?\s*([0-9,]+)",
    },
    SimplePattern {
        label: "Rakuten Bank",
        detect: r"(?i)楽天(?:銀行)?",
        name_re: r"(?:ご利用先|明細)[：:]\s*(.+)",
        amount_re: r"(?:金額|引落金額)[：:]\s*[¥￥]?\s*([0-9,]+)",
    },
    SimplePattern {
        label: "Yucho",
        detect: r"(?i)ゆうちょ|Yucho|Japan Post",
        name_re: r"(?:払込先|ご利用先|内容)[：:]\s*(.+)",
        amount_re: r"(?:金額|払込金額)[：:]\s*[¥￥]?\s*([0-9,]+)",
    },
    SimplePattern {
        label: "PayPay Bank",
        detect: r"(?i)PayPay銀行",
        name_re: r"(?:ご利用先|明細)[：:]\s*(.+)",
        amount_re: r"(?:金額|引落金額)[：:]\s*[¥￥]?\s*([0-9,]+)",
    },
    SimplePattern {
        label: "Generic Bank",
        detect: r"(?i)お引(?:き)?落とし|引落|口座引落|自動払込",
        name_re: r"(?:ご利用先|サービス名|明細|内容)[：:]\s*(.+)",
        amount_re: r"(?:金額|ご利用金額|お支払金額|引落金額)[：:]\s*[¥￥]?\s*([0-9,]+)",
    },
];

pub(super) fn parse_bank(email: &RawEmail) -> Option<Candidate> {
    let text = format!("{}\n{}", email.subject.as_deref().unwrap_or(""), email.body);
    for p in BANK_PATTERNS {
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
        let amount = parse_num(&am[1]);
        if amount.is_nan() || !amount_ok(amount) {
            continue;
        }
        return Some(Candidate {
            name,
            price: amount,
            currency: "JPY".to_string(),
            cycle: Some("monthly".to_string()),
            source: "email".to_string(),
            source_detail: format!("Bank withdrawal email ({})", p.label),
            confidence: 0.85,
        });
    }
    None
}
