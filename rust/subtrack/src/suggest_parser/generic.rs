use regex_lite::Regex;

use super::common::{Candidate, RawEmail, amount_ok, parse_num};

// ── Generic parser (port of `generic.ts`) ────────────────────────────────

fn extract_name(email: &RawEmail) -> String {
    let subject = email.subject.as_deref().unwrap_or("");
    let prefix =
        Regex::new(r"(?i)^(?:Re|Fwd?|Thank you for your|Receipt|Invoice|Payment|Your)\s*").unwrap();
    let keyword = Regex::new(r"(?i)receipt|payment|invoice|confirmation|notification").unwrap();
    let brackets = Regex::new(r"[\[\]【】「」『』]").unwrap();
    let cleaned = prefix.replace(subject, "").to_string();
    let cleaned = keyword.replace(&cleaned, "").to_string();
    let cleaned = brackets.replace_all(&cleaned, "").trim().to_string();

    if cleaned.len() >= 2 && cleaned.len() <= 80 {
        return cleaned;
    }

    let from = email.from.as_deref().unwrap_or("");
    if let Some(caps) = Regex::new(r"@([^.]+)").unwrap().captures(from) {
        let domain = &caps[1];
        let mut chars = domain.chars();
        if let Some(first) = chars.next() {
            return format!("{}{}", first.to_uppercase(), chars.as_str());
        }
    }

    "Unknown Service".to_string()
}

fn extract_price(text: &str) -> Option<(f64, String)> {
    // (regex, capture index of amount, currency for symbol-less patterns)
    let simple: &[(&str, &str)] = &[
        (r"¥\s*([0-9,]+)", "JPY"),
        (r"([0-9,]+)\s*円", "JPY"),
        (r"\$\s*([0-9]+(?:\.[0-9]{1,2})?)", "USD"),
        (r"€\s*([0-9]+(?:\.[0-9]{1,2})?)", "EUR"),
        (r"£\s*([0-9]+(?:\.[0-9]{1,2})?)", "GBP"),
    ];
    for (re, ccy) in simple {
        if let Some(caps) = Regex::new(re).unwrap().captures(text) {
            let raw = parse_num(&caps[1]);
            if amount_ok(raw) {
                let amount = if *ccy == "JPY" { raw.round() } else { raw };
                return Some((amount, ccy.to_string()));
            }
            // JS continues checking only if the guard fails? In TS the guard
            // wraps the return, so an out-of-range match falls through to the
            // next pattern.
        }
    }

    let kw = Regex::new(
        r"(?i)(?:amount|total|price|charge|cost|payment|金額|価格|料金)\s*[:：]?\s*([¥$€£])\s*([0-9,]+(?:\.[0-9]{1,2})?)",
    )
    .unwrap();
    if let Some(caps) = kw.captures(text) {
        let symbol = &caps[1];
        let currency = match symbol {
            "$" => "USD",
            "¥" => "JPY",
            "€" => "EUR",
            "£" => "GBP",
            _ => "USD",
        }
        .to_string();
        let raw = parse_num(&caps[2]);
        if amount_ok(raw) {
            let amount = if currency == "JPY" { raw.round() } else { raw };
            return Some((amount, currency));
        }
    }

    None
}

fn guess_cycle(text: &str) -> Option<String> {
    let checks: &[(&str, &str)] = &[
        (r"(?i)\b(monthly|per month|mo|month|月額|毎月)\b", "monthly"),
        (
            r"(?i)\b(yearly|annual|per year|yr|年額|年間|毎年)\b",
            "yearly",
        ),
        (r"(?i)\b(weekly|per week|毎週)\b", "weekly"),
        (r"(?i)\b(quarterly|四半期)\b", "quarterly"),
    ];
    for (re, cycle) in checks {
        if Regex::new(re).unwrap().is_match(text) {
            return Some(cycle.to_string());
        }
    }
    None
}

pub(super) fn parse_generic(email: &RawEmail) -> Option<Candidate> {
    let subject = email.subject.as_deref().unwrap_or("");
    let text = format!("{}\n{}", subject, email.body);

    let (amount, currency) = extract_price(&text)?;
    let name = extract_name(email);
    let cycle = guess_cycle(&text);

    let source_detail = if email.subject.is_some() {
        format!("Parsed from email: {}", subject)
    } else {
        "Parsed from email".to_string()
    };

    Some(Candidate {
        name,
        price: amount,
        currency,
        cycle,
        source: "email".to_string(),
        source_detail,
        confidence: 0.4,
    })
}
