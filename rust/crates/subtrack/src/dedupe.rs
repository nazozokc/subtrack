//! `dedupe` command (reads) — port of `dedupe.ts` (handleDedupe).

use crate::ansi;
use crate::db::{get_subscriptions, QueryOptions, Subscription};
use crate::json::quote_string;
use crate::logger::{self, fail, info, log};
use crate::parser::ParsedArgs;
use crate::price::format_price;

/// Lowercase + strip all non-letter/non-digit characters.
fn normalize_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphabetic() || c.is_numeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Levenshtein edit distance with rolling arrays.
fn levenshtein(a: &str, b: &str) -> usize {
    if a == b {
        return 0;
    }
    let m = a.chars().count();
    let n = b.chars().count();
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

fn similarity(a: &str, b: &str) -> f64 {
    let na = normalize_name(a);
    let nb = normalize_name(b);
    if na == nb {
        return 1.0;
    }
    if na.is_empty() || nb.is_empty() {
        return 0.0;
    }
    let dist = levenshtein(&na, &nb) as f64;
    let max = na.chars().count().max(nb.chars().count()) as f64;
    1.0 - dist / max
}

pub fn run_dedupe(parsed: &ParsedArgs) -> i32 {
    let threshold = parsed
        .str("threshold")
        .and_then(|t| t.parse::<f64>().ok())
        .unwrap_or(0.8);
    if threshold < 0.0 || threshold > 1.0 {
        fail("threshold must be between 0 and 1");
        return 1;
    }
    let json = parsed.bool("json") == Some(true);

    let subs = match get_subscriptions(&QueryOptions::default()) {
        Ok(s) => s,
        Err(e) => {
            logger::fail(&format!("Failed to read subscriptions: {}", e));
            return 1;
        }
    };
    let subs: Vec<Subscription> = subs.into_iter().filter(|s| s.status != "cancelled").collect();

    // Find pairs i < j.
    let mut pairs: Vec<(usize, usize, f64, bool)> = Vec::new();
    for i in 0..subs.len() {
        for j in (i + 1)..subs.len() {
            let a = &subs[i];
            let b = &subs[j];
            let name_score = similarity(&a.name, &b.name);
            let vendor_url_match = a.vendor_url.is_some()
                && b.vendor_url.is_some()
                && a.vendor_url == b.vendor_url;
            let score = if vendor_url_match {
                name_score.max(0.9)
            } else {
                name_score
            };
            if score >= threshold {
                pairs.push((i, j, score, vendor_url_match));
            }
        }
    }
    pairs.sort_by(|x, y| y.2.partial_cmp(&x.2).unwrap_or(std::cmp::Ordering::Equal));

    if json {
        let mut out = String::from("[\n");
        for (k, (i, j, score, vend)) in pairs.iter().enumerate() {
            let a = &subs[*i];
            let b = &subs[*j];
            out.push_str("  {\n");
            out.push_str("    \"a\": {\n");
            out.push_str(&format!("      \"id\": {},\n", a.id));
            out.push_str(&format!("      \"name\": {},\n", quote_string(&a.name)));
            out.push_str(&format!("      \"price\": {},\n", crate::json::number(a.price)));
            out.push_str(&format!("      \"currency\": {}\n", quote_string(&a.currency)));
            out.push_str("    },\n");
            out.push_str("    \"b\": {\n");
            out.push_str(&format!("      \"id\": {},\n", b.id));
            out.push_str(&format!("      \"name\": {},\n", quote_string(&b.name)));
            out.push_str(&format!("      \"price\": {},\n", crate::json::number(b.price)));
            out.push_str(&format!("      \"currency\": {}\n", quote_string(&b.currency)));
            out.push_str("    },\n");
            out.push_str(&format!("    \"score\": {},\n", crate::json::number(trim_score(*score))));
            out.push_str(&format!("    \"vendorUrlMatch\": {}\n", vend));
            out.push_str(if k + 1 == pairs.len() { "  }\n" } else { "  },\n" });
        }
        out.push_str("]\n");
        logger::out_str(&out);
        return 0;
    }

    if pairs.is_empty() {
        info("No duplicate subscriptions found");
        return 0;
    }

    log(&ansi::bold(&format!("Potential duplicates (threshold: {}):", threshold)));
    log("");
    for (i, j, score, vend) in &pairs {
        let a = &subs[*i];
        let b = &subs[*j];
        let score_str = ansi::cyan(&format!("{}%", (score * 100.0).round() as i64));
        if *vend {
            log(&format!(
                "  {}  {}/{}",
                ansi::bold(&format!("#{} {}", a.id, a.name)),
                format_price(a.price, &a.currency),
                a.cycle
            ));
            log(&format!(
                "  {}  {}/{}  {} {}",
                ansi::bold(&format!("#{} {}", b.id, b.name)),
                format_price(b.price, &b.currency),
                b.cycle,
                score_str,
                ansi::dim(" [same vendor URL]"),
            ));
        } else {
            log(&format!(
                "  {}  {}/{}",
                ansi::bold(&format!("#{} {}", a.id, a.name)),
                format_price(a.price, &a.currency),
                a.cycle
            ));
            log(&format!(
                "  {}  {}/{}  {}",
                ansi::bold(&format!("#{} {}", b.id, b.name)),
                format_price(b.price, &b.currency),
                b.cycle,
                score_str,
            ));
        }
        log("");
    }
    info("Merge with: subtrack dedupe merge <keepId> <removeId>");
    0
}

/// Port of `Number(p.score.toFixed(3))`.
fn trim_score(score: f64) -> f64 {
    let scaled = (score * 1000.0).round() / 1000.0;
    scaled
}