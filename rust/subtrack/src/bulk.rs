//! `bulk` commands — port of `bulk.ts` (`handleBulkStatus`,
//! `handleBulkDelete`, `handleBulkTagAdd`, `handleBulkTagRemove`).

use crate::db::{self, QueryOptions, SqlValue, Subscription};
use crate::logger::{self, info};
use crate::parser::ParsedArgs;
use crate::validation::is_valid_status;

struct Filters {
    tag: Option<String>,
    status: Option<String>,
    name: Option<String>,
}

impl Filters {
    fn from_args(parsed: &ParsedArgs) -> Filters {
        Filters {
            tag: parsed.str("tag").filter(|s| !s.trim().is_empty()),
            status: parsed.str("status").filter(|s| !s.trim().is_empty()),
            name: parsed.str("name").filter(|s| !s.trim().is_empty()),
        }
    }

    fn is_empty(&self) -> bool {
        self.tag.is_none() && self.status.is_none() && self.name.is_none()
    }
}

/// Port of `getFilteredSubscriptions`.
fn get_filtered(filters: &Filters) -> std::io::Result<Vec<Subscription>> {
    let mut list = db::get_subscriptions(&QueryOptions {
        include_archived: false,
        ..Default::default()
    })?;

    if let Some(tag) = &filters.tag {
        let tag = tag.trim().to_lowercase();
        list.retain(|s| s.tags.iter().any(|t| t.to_lowercase() == tag));
    }
    if let Some(status) = &filters.status {
        let status = status.trim().to_lowercase();
        list.retain(|s| s.status.to_lowercase() == status);
    }
    if let Some(name) = &filters.name {
        let pattern = name.trim().to_lowercase();
        list.retain(|s| s.name.to_lowercase().contains(&pattern));
    }

    Ok(list)
}

/// Port of `confirmAction` + `shouldProceed`. `force` short-circuits the
/// confirmation; otherwise the (non-interactive) prompt defaults to "no".
fn should_proceed(message: &str, count: usize, force: bool) -> bool {
    if count == 0 {
        info("No subscriptions match the filter");
        return false;
    }
    if force {
        return true;
    }
    // `confirm` falls back to its default (false) in a non-interactive shell.
    let _ = message;
    info("Cancelled");
    false
}

/// Port of `handleBulkStatus`.
pub fn run_bulk_status(parsed: &ParsedArgs) -> i32 {
    let target = parsed.str("set").unwrap_or_default();
    if target.is_empty() || !is_valid_status(&target) {
        logger::fail(&format!(
            "Invalid status \"{}\". Must be active, paused, or cancelled",
            target
        ));
        return 1;
    }

    let filters = Filters::from_args(parsed);
    if filters.is_empty() {
        // Interactive filter prompt — aborts cleanly without a TTY.
        return 0;
    }

    let list = match get_filtered(&filters) {
        Ok(v) => v,
        Err(e) => {
            logger::fail(&format!("Failed to load subscriptions: {}", e));
            return 1;
        }
    };

    if !should_proceed(
        &format!("Change status to \"{}\"", target),
        list.len(),
        parsed.bool("force") == Some(true),
    ) {
        return 0;
    }

    let conn = match db::open_db() {
        Ok(c) => c,
        Err(e) => {
            logger::fail(&format!("Failed to open database: {}", e));
            return 1;
        }
    };
    for sub in &list {
        if let Err(e) = db::update_subscription(
            &conn,
            sub.id,
            &[("status", SqlValue::Text(target.clone()))],
            false,
        ) {
            logger::fail(&format!("Failed to update subscription: {}", e));
            return 1;
        }
    }
    if let Err(e) = db::save_db(&conn) {
        logger::fail(&format!("Failed to save database: {}", e));
        return 1;
    }
    let _ = db::add_audit_log(
        &conn,
        "subscription.bulk_status",
        None,
        None,
        Some(&format!("{} subscriptions → \"{}\"", list.len(), target)),
        true,
    );
    logger::success(&format!(
        "Updated {} subscription{} to \"{}\"",
        list.len(),
        if list.len() > 1 { "s" } else { "" },
        target
    ));
    0
}

/// Port of `handleBulkDelete`.
pub fn run_bulk_delete(parsed: &ParsedArgs) -> i32 {
    let filters = Filters::from_args(parsed);
    if filters.is_empty() {
        return 0;
    }

    let list = match get_filtered(&filters) {
        Ok(v) => v,
        Err(e) => {
            logger::fail(&format!("Failed to load subscriptions: {}", e));
            return 1;
        }
    };

    if !should_proceed("Delete", list.len(), parsed.bool("force") == Some(true)) {
        return 0;
    }

    let conn = match db::open_db() {
        Ok(c) => c,
        Err(e) => {
            logger::fail(&format!("Failed to open database: {}", e));
            return 1;
        }
    };
    for sub in &list {
        if let Err(e) = db::remove_subscription(&conn, sub.id, false) {
            logger::fail(&format!("Failed to delete subscription: {}", e));
            return 1;
        }
    }
    if let Err(e) = db::save_db(&conn) {
        logger::fail(&format!("Failed to save database: {}", e));
        return 1;
    }
    let _ = db::add_audit_log(
        &conn,
        "subscription.bulk_delete",
        None,
        None,
        Some(&format!("{} subscriptions deleted", list.len())),
        true,
    );
    logger::success(&format!(
        "Deleted {} subscription{}",
        list.len(),
        if list.len() > 1 { "s" } else { "" }
    ));
    0
}

/// Port of `handleBulkTagAdd`.
pub fn run_bulk_tag_add(parsed: &ParsedArgs) -> i32 {
    let Some(raw_tag) = parsed.str("add") else {
        logger::fail("Tag name is required");
        return 1;
    };
    let tag_name = raw_tag.trim().to_string();
    if tag_name.is_empty() {
        logger::fail("Tag name is required");
        return 1;
    }

    let filters = Filters::from_args(parsed);
    if filters.is_empty() {
        return 0;
    }

    let list = match get_filtered(&filters) {
        Ok(v) => v,
        Err(e) => {
            logger::fail(&format!("Failed to load subscriptions: {}", e));
            return 1;
        }
    };

    if !should_proceed(
        &format!("Add tag \"{}\" to", tag_name),
        list.len(),
        parsed.bool("force") == Some(true),
    ) {
        return 0;
    }

    let conn = match db::open_db() {
        Ok(c) => c,
        Err(e) => {
            logger::fail(&format!("Failed to open database: {}", e));
            return 1;
        }
    };
    for sub in &list {
        let mut new_tags = sub.tags.clone();
        if !new_tags.iter().any(|t| t == &tag_name) {
            new_tags.push(tag_name.clone());
        }
        if let Err(e) = db::set_subscription_tags(&conn, sub.id, &new_tags, false) {
            logger::fail(&format!("Failed to update subscription: {}", e));
            return 1;
        }
    }
    if let Err(e) = db::save_db(&conn) {
        logger::fail(&format!("Failed to save database: {}", e));
        return 1;
    }
    let _ = db::add_audit_log(
        &conn,
        "subscription.bulk_tag_add",
        None,
        None,
        Some(&format!(
            "Tag \"{}\" added to {} subscriptions",
            tag_name,
            list.len()
        )),
        true,
    );
    logger::success(&format!(
        "Added tag \"{}\" to {} subscription{}",
        tag_name,
        list.len(),
        if list.len() > 1 { "s" } else { "" }
    ));
    0
}

/// Port of `handleBulkTagRemove`.
pub fn run_bulk_tag_remove(parsed: &ParsedArgs) -> i32 {
    let Some(raw_tag) = parsed.str("remove") else {
        logger::fail("Tag name is required");
        return 1;
    };
    let tag_name = raw_tag.trim().to_string();
    if tag_name.is_empty() {
        logger::fail("Tag name is required");
        return 1;
    }

    let filters = Filters::from_args(parsed);
    if filters.is_empty() {
        return 0;
    }

    let list = match get_filtered(&filters) {
        Ok(v) => v,
        Err(e) => {
            logger::fail(&format!("Failed to load subscriptions: {}", e));
            return 1;
        }
    };

    if !should_proceed(
        &format!("Remove tag \"{}\" from", tag_name),
        list.len(),
        parsed.bool("force") == Some(true),
    ) {
        return 0;
    }

    let conn = match db::open_db() {
        Ok(c) => c,
        Err(e) => {
            logger::fail(&format!("Failed to open database: {}", e));
            return 1;
        }
    };
    for sub in &list {
        let new_tags: Vec<String> = sub
            .tags
            .iter()
            .filter(|t| *t != &tag_name)
            .cloned()
            .collect();
        if let Err(e) = db::set_subscription_tags(&conn, sub.id, &new_tags, false) {
            logger::fail(&format!("Failed to update subscription: {}", e));
            return 1;
        }
    }
    if let Err(e) = db::save_db(&conn) {
        logger::fail(&format!("Failed to save database: {}", e));
        return 1;
    }
    logger::success(&format!(
        "Removed tag \"{}\" from {} subscription{}",
        tag_name,
        list.len(),
        if list.len() > 1 { "s" } else { "" }
    ));
    0
}
