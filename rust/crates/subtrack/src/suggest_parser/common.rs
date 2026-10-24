use regex_lite::Regex;

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
pub(super) fn strip_tags(name: &str) -> String {
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

pub(super) fn amount_ok(amount: f64) -> bool {
    amount > 0.0 && amount <= 99_999_999.0
}

pub(super) fn parse_num(raw: &str) -> f64 {
    raw.replace(',', "").parse::<f64>().unwrap_or(f64::NAN)
}

/// Pattern bundle shared by the bank / card / wallet parsers.
pub(super) struct SimplePattern {
    pub(super) label: &'static str,
    pub(super) detect: &'static str,
    pub(super) name_re: &'static str,
    pub(super) amount_re: &'static str,
}
