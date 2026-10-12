//! `review` command — port of `review.ts` (handleReview).

use crate::db::{get_subscriptions, get_trials_expiring_soon, QueryOptions};
use crate::logger;
use crate::parser::ParsedArgs;

struct Item {
    name: String,
    reason: String,
    fields: Vec<String>,
}

/// Parse a day-window flag. `Ok(None)` when absent; `Err(())` after `fail()`.
fn parse_days(parsed: &ParsedArgs, flag: &str) -> Result<Option<i64>, ()> {
    match parsed.str(flag) {
        None => Ok(None),
        Some(raw) => match raw.parse::<i64>() {
            Ok(n) if n >= 0 => Ok(Some(n)),
            _ => {
                logger::fail(&format!("{} must be a non-negative integer", flag));
                Err(())
            }
        },
    }
}

pub fn run_review(parsed: &ParsedArgs) -> i32 {
    let json_out = parsed.bool("json") == Some(true);

    let bill_days = match parse_days(parsed, "billDays") {
        Ok(v) => v.unwrap_or(7),
        Err(()) => return 0,
    };
    let contract_days = match parse_days(parsed, "contractDays") {
        Ok(v) => v.unwrap_or(60),
        Err(()) => return 0,
    };
    let trial_days = match parse_days(parsed, "trialDays") {
        Ok(v) => v.unwrap_or(30),
        Err(()) => return 0,
    };

    let active = match get_subscriptions(&QueryOptions {
        status: Some("active".to_string()),
        ..Default::default()
    }) {
        Ok(s) => s,
        Err(e) => {
            logger::fail(&e.to_string());
            return 0
        }
    };

    let now = crate::date::today();
    let contract_end = (now + chrono::Duration::days(contract_days))
        .format("%Y-%m-%d")
        .to_string();

    let mut items: Vec<Item> = Vec::new();
    for sub in &active {
        let mut reasons: Vec<String> = Vec::new();
        let next = crate::date::calculate_next_billing_inclusive(
            sub.billing_day,
            &sub.created_at,
            &sub.cycle,
            now,
        );
        let d = crate::date::days_until(next);
        if d >= 0 && d <= bill_days {
            reasons.push(format!("bill due {}", next.format("%Y-%m-%d")));
        }
        if let Some(end) = &sub.contract_end {
            if end.as_str() <= contract_end.as_str() {
                reasons.push(format!("contract ends {}", end));
            }
        }
        if !reasons.is_empty() {
            let reason = reasons.join(", ");
            let mut fields = common_fields("subscription", sub.id, &sub.name, &reason);
            fields.extend(
                crate::json::sub_fields(sub)
                    .into_iter()
                    .filter(|f| !f.starts_with("\"id\"") && !f.starts_with("\"name\"")),
            );
            items.push(Item {
                name: sub.name.clone(),
                reason,
                fields,
            });
        }
    }

    let trials = match get_trials_expiring_soon(trial_days) {
        Ok(t) => t,
        Err(e) => {
            logger::fail(&e.to_string());
            return 0
        }
    };
    for trial in &trials {
        let reason = format!("trial expires {}", trial.expires_at);
        let mut fields = common_fields("trial", trial.id, &trial.name, &reason);
        fields.extend(
            crate::json::trial_fields(trial)
                .into_iter()
                .filter(|f| !f.starts_with("\"id\"") && !f.starts_with("\"name\"")),
        );
        items.push(Item {
            name: trial.name.clone(),
            reason,
            fields,
        });
    }

    if json_out {
        let rendered: Vec<String> = items
            .iter()
            .map(|it| format!("  {{\n    {}\n  }}", it.fields.join(",\n    ")))
            .collect();
        logger::out_str(&format!("[\n{}\n]\n", rendered.join(",\n")));
        return 0
    }

    if items.is_empty() {
        logger::info("Nothing needs review");
        return 0
    }

    // The reference prints the first item and then blocks on an interactive
    // select; with a non-TTY stdin the prompt aborts (ExitPromptError), which
    // the runner treats as a clean exit. Reproduce that here.
    let item = &items[0];
    logger::log(&format!("{} — {}", crate::ansi::bold(&item.name), item.reason));
    0
}

fn common_fields(kind: &str, id: i64, name: &str, reason: &str) -> Vec<String> {
    let kv = |k: &str, v: String| format!("{}: {}", crate::json::quote_string(k), v);
    vec![
        kv("kind", crate::json::quote_string(kind)),
        kv("id", id.to_string()),
        kv("name", crate::json::quote_string(name)),
        kv("reason", crate::json::quote_string(reason)),
    ]
}
