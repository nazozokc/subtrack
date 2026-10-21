use crate::db::get_subscription_by_id;
use crate::logger::{self, fail, info};
use crate::parser::ParsedArgs;

pub fn run_delete(parsed: &ParsedArgs) -> i32 {
    let mut ids: Vec<i64> = parsed
        .str_array("id")
        .unwrap_or_default()
        .iter()
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .collect();
    if ids.is_empty() {
        let all = match crate::db::get_subscriptions(&crate::db::QueryOptions {
            include_archived: true,
            ..Default::default()
        }) {
            Ok(v) => v,
            Err(e) => {
                logger::fail(&format!("Failed to read subscriptions: {}", e));
                return 1;
            }
        };
        if all.is_empty() {
            info("No subscriptions found — try `subtrack add`");
            return 0;
        }
        let choices: Vec<crate::prompt::Choice> = all
            .iter()
            .map(|s| {
                crate::prompt::Choice::new(
                    &format!(
                        "{} — {}/{}{}",
                        s.name,
                        crate::price::format_price(s.price, &s.currency),
                        s.cycle,
                        if s.tags.is_empty() {
                            String::new()
                        } else {
                            format!(" [{}]", s.tags.join(", "))
                        }
                    ),
                    &s.id.to_string(),
                )
            })
            .collect();
        let Some(selected) = crate::prompt::checkbox("select subscriptions to delete", &choices)
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
        let names: Vec<String> = all
            .iter()
            .filter(|s| ids.contains(&s.id))
            .map(|s| s.name.clone())
            .collect();
        if crate::prompt::confirm(
            &format!(
                "Delete {} subscription{}? ({})",
                names.len(),
                if names.len() > 1 { "s" } else { "" },
                names.join(", ")
            ),
            false,
        ) != Some(true)
        {
            info("Cancelled");
            return 0;
        }
    }
    let conn = match crate::db::open_db() {
        Ok(c) => c,
        Err(e) => {
            logger::fail(&format!("Failed to delete: {}", e));
            return 1;
        }
    };
    for id in ids {
        match get_subscription_by_id(id) {
            Ok(Some(sub)) => {
                if let Err(e) = crate::db::remove_subscription(&conn, id, true) {
                    logger::fail(&format!("Failed to delete: {}", e));
                    continue;
                }
                let _ = crate::db::add_audit_log(
                    &conn,
                    "subscription.delete",
                    Some("subscription"),
                    Some(id),
                    Some(&sub.name),
                    true,
                );
                logger::success(&format!("Deleted: {}", sub.name));
            }
            Ok(None) => {
                fail(&format!("Subscription with id {} not found", id));
            }
            Err(e) => {
                logger::fail(&format!("Failed to read subscription: {}", e));
            }
        }
    }
    0
}
