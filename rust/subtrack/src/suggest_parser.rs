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
//! this file (literals, `(?i)`, `\d`/`\s`, character classes, `{m,n}`
//! quantifiers, alternation, non-greedy repetition) is fully supported by
//! regex-lite and its engine is far lighter in the shipped binary. JavaScript
//! `\b` is ASCII-only while regex-lite's boundary derives from `\w`, so the
//! `guess_cycle` guards that mix Japanese words with `\b` are kept out of
//! boundary-sensitive positions (verified byte-exact against the golden
//! fixtures).

use regex_lite::Regex;

/// Raw email input read from a local file (port of `RawEmail`).
pub struct RawEmail {
    pub subject: Option<String>,
    pub from: Option<String>,
    pub body: String,
}

/// Parsed suggestion candidate (port of `SuggestionCandidate`).
/// Field order matches the TS object literal so JSON output is identical.
pub struct Candidate {
    pub name: String,
    pub price: f64,
    pub currency: String,
    pub cycle: Option<String>,
    pub source: String,
    pub source_detail: String,
    pub confidence: f64,
}

impl Candidate {
    /// `JSON.stringify(candidate, null, 2)` with JS number/string formatting.
    /// `base` is the indentation of the opening/closing brace; fields are
    /// indented two spaces further.
    pub fn to_json(&self, base: &str) -> String {
        let f = format!("{}  ", base);
        let mut s = String::new();
        s.push_str(&format!("{}{{\n", base));
        let fields = [
            format!(
                "{}{}: {}",
                f,
                "\"name\"",
                crate::json::quote_string(&self.name)
            ),
            format!("{}{}: {}", f, "\"price\"", crate::json::number(self.price)),
            format!(
                "{}{}: {}",
                f,
                "\"currency\"",
                crate::json::quote_string(&self.currency)
            ),
            format!(
                "{}{}: {}",
                f,
                "\"cycle\"",
                match &self.cycle {
                    Some(c) => crate::json::quote_string(c),
                    None => "null".to_string(),
                }
            ),
            format!(
                "{}{}: {}",
                f,
                "\"source\"",
                crate::json::quote_string(&self.source)
            ),
            format!(
                "{}{}: {}",
                f,
                "\"sourceDetail\"",
                crate::json::quote_string(&self.source_detail)
            ),
            format!(
                "{}{}: {}",
                f,
                "\"confidence\"",
                crate::json::number(self.confidence)
            ),
        ];
        s.push_str(&fields.join(",\n"));
        s.push_str(&format!("\n{}}}", base));
        s
    }
}

/// Repeatedly strip HTML-like tags (nested-tag bypass protection).
fn strip_tags(name: &str) -> String {
    let re = Regex::new(r"<[^>]+>").unwrap();
    let mut cur = name.to_string();
    loop {
        let next = re.replace_all(&cur, "").to_string();
        if next == cur {
            break;
        }
        cur = next;
    }
    cur.trim().to_string()
}

fn amount_ok(amount: f64) -> bool {
    amount > 0.0 && amount <= 99_999_999.0
}

fn parse_num(raw: &str) -> f64 {
    raw.replace(',', "").parse::<f64>().unwrap_or(f64::NAN)
}

// ── Bank withdrawal parser (port of `bank.ts`) ───────────────────────────

struct SimplePattern {
    label: &'static str,
    detect: &'static str,
    name_re: &'static str,
    amount_re: &'static str,
}

const BANK_PATTERNS: &[SimplePattern] = &[
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

fn parse_bank(email: &RawEmail) -> Option<Candidate> {
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

// ── Credit card parser (port of `credit-card.ts`) ────────────────────────

const CARD_PATTERNS: &[SimplePattern] = &[
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

fn parse_credit_card(email: &RawEmail) -> Option<Candidate> {
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

// ── Service receipt parser (port of `parser/receipt.ts`) ─────────────────

struct ReceiptPattern {
    label: &'static str,
    detect: &'static str,
    name_re: &'static str,
    amount_re: &'static str,
    cycle_re: &'static str,
}

const RECEIPT_PATTERNS: &[ReceiptPattern] = &[
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

const RECEIPT_KEYWORDS: &str = r"(?i)receipt|invoice|payment confirmation|charge|billing|subscription|purchase|receipt|領収書|請求書|支払|購入|ご利用ありがとう|Thank you for your (purchase|payment|order)";

fn parse_receipt_service(email: &RawEmail) -> Option<Candidate> {
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

// ── Wallet parser (port of `wallet.ts`) ──────────────────────────────────

const WALLET_PATTERNS: &[SimplePattern] = &[
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

fn parse_wallet(email: &RawEmail) -> Option<Candidate> {
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

fn parse_generic(email: &RawEmail) -> Option<Candidate> {
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

/// Port of `parseEmail`: try each parser in order, first non-null wins.
pub fn parse_email(email: &RawEmail) -> Option<Candidate> {
    parse_bank(email)
        .or_else(|| parse_credit_card(email))
        .or_else(|| parse_receipt_service(email))
        .or_else(|| parse_wallet(email))
        .or_else(|| parse_generic(email))
}

#[cfg(test)]
mod tests {
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
}
