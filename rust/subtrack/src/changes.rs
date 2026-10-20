//! `changes` command — port of `diagnostics.ts` handleChanges.

use crate::ansi;
use crate::db::{get_all_price_changes, get_audit_logs_filtered};
use crate::json::quote_string;
use crate::logger::{self, info, log};
use crate::parser::ParsedArgs;

enum ChangeEntry {
    Audit {
        date: String,
        id: Option<i64>,
        details: String,
    },
    Price {
        date: String,
        id: i64,
        details: String,
    },
}

impl ChangeEntry {
    fn date(&self) -> &str {
        match self {
            ChangeEntry::Audit { date, .. } | ChangeEntry::Price { date, .. } => date,
        }
    }
}

pub fn run_changes(parsed: &ParsedArgs) -> i32 {
    let id = parsed.str("id").and_then(|i| i.parse::<i64>().ok());
    let from = parsed.str("from");
    let to = parsed.str("to");
    let limit = parsed
        .str("limit")
        .and_then(|l| l.parse::<usize>().ok())
        .unwrap_or(50);
    let json = parsed.bool("json") == Some(true);

    let mut result: Vec<ChangeEntry> = Vec::new();

    let audits = get_audit_logs_filtered(None, id, 100_000, 0, from.as_deref(), to.as_deref());
    for entry in audits {
        result.push(ChangeEntry::Audit {
            date: entry.created_at,
            id: entry.target_id,
            details: format!("{}: {}", entry.action, entry.details.unwrap_or_default()),
        });
    }

    let cutoff_from = from.clone().unwrap_or_default();
    let cutoff_to = to.clone().unwrap_or_default();
    let prices = get_all_price_changes()
        .unwrap_or_default()
        .into_iter()
        .filter(|e| {
            let date = e.changed_at.chars().take(10).collect::<String>();
            (id.is_none() || Some(e.subscription_id) == id)
                && (from.is_none() || date.as_str() >= cutoff_from.as_str())
                && (to.is_none() || date.as_str() <= cutoff_to.as_str())
        })
        .map(|e| {
            let old = e
                .old_price
                .map(|p| format!("{}", p))
                .unwrap_or_else(|| "—".to_string());
            let new = format!("{}", e.new_price);
            ChangeEntry::Price {
                date: e.changed_at,
                id: e.subscription_id,
                details: format!("{}: {} → {}", e.name, old, new),
            }
        })
        .collect::<Vec<ChangeEntry>>();
    result.extend(prices);

    // Stable sort by date descending (audit entries keep relative order on ties).
    let mut sortable: Vec<(&ChangeEntry, std::cmp::Reverse<String>)> = result
        .iter()
        .map(|e| {
            let d = e.date().to_string();
            (e, std::cmp::Reverse(d))
        })
        .collect();
    sortable.sort_by(|(_, a), (_, b)| b.0.cmp(&a.0));
    let mut merged: Vec<&ChangeEntry> = sortable.into_iter().map(|(e, _)| e).collect();
    merged.truncate(limit);

    if json {
        let mut out = String::from("[\n");
        for (i, e) in merged.iter().enumerate() {
            match e {
                ChangeEntry::Audit { date, id, details } => {
                    out.push_str("  {\n");
                    out.push_str("    \"type\": \"audit\",\n");
                    out.push_str(&format!("    \"date\": {},\n", quote_string(date)));
                    out.push_str(&format!(
                        "    \"id\": {},\n",
                        match id {
                            Some(i) => i.to_string(),
                            None => "null".to_string(),
                        }
                    ));
                    out.push_str(&format!("    \"details\": {}\n", quote_string(details)));
                }
                ChangeEntry::Price { date, id, details } => {
                    out.push_str("  {\n");
                    out.push_str("    \"type\": \"price\",\n");
                    out.push_str(&format!("    \"date\": {},\n", quote_string(date)));
                    out.push_str(&format!("    \"id\": {},\n", id));
                    out.push_str(&format!("    \"details\": {}\n", quote_string(details)));
                }
            }
            out.push_str(if i + 1 == merged.len() {
                "  }\n"
            } else {
                "  },\n"
            });
        }
        out.push_str("]\n");
        logger::out_str(&out);
        return 0;
    }

    if merged.is_empty() {
        info("No changes found");
        return 0;
    }
    for e in merged {
        let (date, details) = match e {
            ChangeEntry::Audit { date, details, .. } | ChangeEntry::Price { date, details, .. } => {
                (date, details)
            }
        };
        log(&format!("{}  {}", ansi::dim(date), details));
    }
    0
}
