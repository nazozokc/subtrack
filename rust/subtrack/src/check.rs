//! `check` command — port of `diagnostics.ts` (handleCheck).

use crate::date::today_string;
use crate::db::{QueryOptions, get_subscriptions};
use crate::json::quote_string;
use crate::logger::{self, log, success};
use crate::parser::ParsedArgs;

struct Finding {
    code: &'static str,
    severity: &'static str,
    message: String,
    id: Option<i64>,
}

/// Port of handleCheck. Returns 1 only for a read failure; `--strict` sets the
/// process exit code via `set_exit_code` (mirrors `process.exitCode = 1`).
pub fn run_check(parsed: &ParsedArgs) -> i32 {
    let json = parsed.bool("json") == Some(true);
    let strict = parsed.bool("strict") == Some(true);

    let subs = match get_subscriptions(&QueryOptions {
        include_archived: true,
        ..Default::default()
    }) {
        Ok(s) => s,
        Err(e) => {
            logger::fail(&format!("Failed to read subscriptions: {}", e));
            return 1;
        }
    };

    let today = today_string();
    let mut findings: Vec<Finding> = Vec::new();

    // Duplicate detection — first-appearance order (JS Map semantics).
    let mut order: Vec<String> = Vec::new();
    let mut groups: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    for sub in &subs {
        let key = sub.name.trim().to_lowercase();
        if !groups.contains_key(&key) {
            order.push(key.clone());
        }
        groups.entry(key).or_default().push(sub.name.clone());

        if sub.status == "active" && sub.billing_day.is_none() {
            findings.push(Finding {
                code: "missing-billing-day",
                severity: "warning",
                message: format!("{} has no billing day", sub.name),
                id: Some(sub.id),
            });
        }
        if let (Some(cs), Some(ce)) = (&sub.contract_start, &sub.contract_end)
            && cs.as_str() > ce.as_str()
        {
            findings.push(Finding {
                code: "invalid-contract-range",
                severity: "error",
                message: format!("{} contract start is after contract end", sub.name),
                id: Some(sub.id),
            });
        }
        if let Some(ce) = &sub.contract_end
            && ce.as_str() < today.as_str()
        {
            findings.push(Finding {
                code: "expired-contract",
                severity: "warning",
                message: format!("{} contract ended on {}", sub.name, ce),
                id: Some(sub.id),
            });
        }
    }

    for key in &order {
        let group = &groups[key];
        if !key.is_empty() && group.len() > 1 {
            findings.push(Finding {
                code: "duplicate-name",
                severity: "warning",
                message: format!("Duplicate subscription name: {}", group.join(", ")),
                id: None,
            });
        }
    }

    if json {
        logger::out_str(&findings_json(&findings));
        logger::out_str("\n");
    } else if findings.is_empty() {
        success("No problems found");
    } else {
        for f in &findings {
            let label = if f.severity == "error" {
                crate::ansi::red("error")
            } else {
                crate::ansi::yellow("warning")
            };
            log(&format!("{}: {}", label, f.message));
        }
    }

    if strict && !findings.is_empty() {
        crate::set_exit_code(1);
    }
    0
}

fn findings_json(findings: &[Finding]) -> String {
    if findings.is_empty() {
        return "[]".to_string();
    }
    let items: Vec<String> = findings.iter().map(finding_json).collect();
    format!("[\n{}\n]", items.join(",\n"))
}

fn finding_json(f: &Finding) -> String {
    let mut fields: Vec<String> = Vec::new();
    let kv = |k: &str, v: String| format!("{}: {}", quote_string(k), v);
    fields.push(kv("code", quote_string(f.code)));
    fields.push(kv("severity", quote_string(f.severity)));
    fields.push(kv("message", quote_string(&f.message)));
    if let Some(id) = f.id {
        fields.push(kv("id", id.to_string()));
    }
    format!("  {{\n    {}\n  }}", fields.join(",\n    "))
}
