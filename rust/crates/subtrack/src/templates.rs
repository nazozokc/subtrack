//! `template` command (reads) — port of `template.ts` handleTemplate("list").

use crate::config::load_config;
use crate::logger::{self, info, log};
use crate::parser::ParsedArgs;

pub fn run_template_list(parsed: &ParsedArgs) -> i32 {
    let _ = parsed;
    let config = load_config();
    let templates = config
        .raw
        .get("templates")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    if templates.is_empty() {
        info("No templates found");
        return 0;
    }
    for (key, template) in &templates {
        let price = template
            .get("price")
            .and_then(|p| p.as_f64())
            .map(|p| crate::json::number(p))
            .unwrap_or_else(|| "0".to_string());
        let currency = template
            .get("currency")
            .and_then(|c| c.as_str())
            .unwrap_or("USD")
            .to_string();
        let cycle = template
            .get("cycle")
            .and_then(|c| c.as_str())
            .unwrap_or("monthly")
            .to_string();
        log(&format!("{}: {} {}/{}", key, price, currency, cycle));
    }
    0
}

/// Receipt/template helper alias to keep the logger import stable.
#[allow(dead_code)]
fn _unused(_p: &ParsedArgs) {
    let _ = logger::success;
}