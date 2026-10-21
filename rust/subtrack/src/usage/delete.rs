use crate::logger::{self, info};
use crate::parser::ParsedArgs;

pub fn run_usage_delete(parsed: &ParsedArgs) -> i32 {
    let raw = parsed.str_array("id").unwrap_or_default();
    let mut ids: Vec<i64> = Vec::new();
    for v in &raw {
        match v.trim().parse::<i64>() {
            Ok(n) if n >= 1 => ids.push(n),
            _ => {
                logger::fail("Usage entry IDs must be positive integers");
                return 1;
            }
        }
    }
    if ids.is_empty() {
        let entries = match crate::db::get_llm_usage(None, None, None, None, Some(50), None) {
            Ok(v) => v,
            Err(e) => {
                logger::fail(&format!("Failed to read usage entries: {}", e));
                return 1;
            }
        };
        if entries.is_empty() {
            info("No usage entries found — try `subtrack usage add`");
            return 0;
        }
        let choices: Vec<crate::prompt::Choice> = entries
            .iter()
            .map(|u| {
                crate::prompt::Choice::new(
                    &format!(
                        "#{} {}/{} — {} on {}",
                        u.id,
                        u.provider,
                        u.model,
                        crate::price::format_usd_cost_f(u.cost, 4),
                        u.date
                    ),
                    &u.id.to_string(),
                )
            })
            .collect();
        let Some(selected) = crate::prompt::checkbox("select usage entries to delete", &choices)
        else {
            return 0;
        };
        if selected.is_empty() {
            info("Cancelled");
            return 0;
        }
        ids = selected
            .iter()
            .filter_map(|v| v.trim().parse::<i64>().ok())
            .collect();
        if crate::prompt::confirm(
            &format!(
                "Delete {} usage entr{}?",
                ids.len(),
                if ids.len() > 1 { "ies" } else { "y" }
            ),
            false,
        ) != Some(true)
        {
            info("Cancelled");
            return 0;
        }
    }
    let mut rc = 0;
    for id in ids {
        match crate::db::remove_llm_usage(id) {
            Ok(true) => logger::success(&format!("Deleted usage entry: {}", id)),
            Ok(false) => {
                logger::fail(&format!("Usage entry with id {} not found", id));
                rc = 1;
            }
            Err(e) => {
                logger::fail(&format!("Failed to delete usage entry: {}", e));
                rc = 1;
            }
        }
    }
    rc
}
