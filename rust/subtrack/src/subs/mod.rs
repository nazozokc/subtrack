//! Subscription command handlers — port of the non-interactive paths of
//! `subscription/add.ts`, `subscription/core.ts` (clone/archive),
//! `subscription/edit.ts`, `subscription-actions.ts` (pause/resume/renew)
//! and `cancel.ts`.
//!
//! Error paths (not-found / validation) and the DB write on the success path
//! are both implemented (`write_subscription` / `update_subscription` +
//! `save_db`).

mod add;
mod cancel;
mod clone;
mod delete;
mod edit;
mod status;

pub use add::run_add;
pub use cancel::run_cancel;
pub use clone::run_clone;
pub use delete::run_delete;
pub use edit::run_edit;
pub use status::{run_archive, run_pause, run_renew, run_resume, run_unarchive};

use crate::logger::{fail, info};
use crate::parser::ParsedArgs;

fn parse_id(value: Option<String>) -> Option<i64> {
    value.and_then(|v| v.trim().parse::<i64>().ok())
}

/// Resolve an id from the positional/flag, or prompt for a subscription.
pub(super) fn resolve_id(
    parsed: &ParsedArgs,
    message: &str,
    filter: fn(&crate::db::Subscription) -> bool,
) -> Option<i64> {
    if let Some(id) = parse_id(parsed.str("id")) {
        return Some(id);
    }
    let all = crate::db::get_subscriptions(&crate::db::QueryOptions {
        include_archived: true,
        ..Default::default()
    })
    .ok()?;
    let filtered: Vec<_> = all.into_iter().filter(filter).collect();
    if filtered.is_empty() {
        info("No matching subscriptions found");
        return None;
    }
    let choices: Vec<crate::prompt::Choice> = filtered
        .iter()
        .map(|s| {
            crate::prompt::Choice::new(
                &format!(
                    "#{} {} — {}/{}{}",
                    s.id,
                    s.name,
                    crate::price::format_price(s.price, &s.currency),
                    s.cycle,
                    if s.status == "active" {
                        String::new()
                    } else {
                        format!(" ({})", s.status)
                    }
                ),
                &s.id.to_string(),
            )
        })
        .collect();
    crate::prompt::select(message, &choices, None)?
        .parse::<i64>()
        .ok()
}

pub(super) fn not_cancelled(sub: &crate::db::Subscription) -> bool {
    sub.status != "cancelled"
}

pub(super) fn archived(sub: &crate::db::Subscription) -> bool {
    sub.status == "archived"
}

pub(super) fn not_archived(sub: &crate::db::Subscription) -> bool {
    sub.status != "archived"
}

/// Validate an optional notes/discount flag value (used by add side effects).
#[allow(dead_code)]
pub(super) fn apply_opt(parsed: &ParsedArgs, key: &str, f: fn(&str) -> Result<(), String>) -> i32 {
    if let Some(raw) = parsed.str(key)
        && !raw.trim().is_empty()
        && let Err(e) = f(raw.trim())
    {
        fail(&e);
        return 1;
    }
    0
}
