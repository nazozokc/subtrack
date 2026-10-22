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

    /// Prompt for filters when none were supplied on the command line.
    fn prompt() -> Option<Filters> {
        let mut filters = Filters {
            tag: None,
            status: None,
            name: None,
        };
        let tag = crate::prompt::input("Tag filter (optional, empty to skip)", Some(""), None)?;
        if !tag.trim().is_empty() {
            filters.tag = Some(tag.trim().to_string());
        }
        let statuses = [
            crate::prompt::Choice::plain("any"),
            crate::prompt::Choice::plain("active"),
            crate::prompt::Choice::plain("paused"),
            crate::prompt::Choice::plain("cancelled"),
        ];
        let status = crate::prompt::select("Status filter", &statuses, Some(0))?;
        if status != "any" {
            filters.status = Some(status);
        }
        let name = crate::prompt::input("Name contains (optional, empty to skip)", Some(""), None)?;
        if !name.trim().is_empty() {
            filters.name = Some(name.trim().to_string());
        }
        Some(filters)
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
    let question = format!(
        "{} {} subscription{}?",
        message,
        count,
        if count > 1 { "s" } else { "" }
    );
    if crate::prompt::confirm(&question, false) == Some(true) {
        return true;
    }
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

    let mut filters = Filters::from_args(parsed);
    if filters.is_empty() {
        if !crate::prompt::interactive() {
            return 0;
        }
        match Filters::prompt() {
            Some(f) => filters = f,
            None => return 0,
        }
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
        parsed.flag("force"),
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
    let mut filters = Filters::from_args(parsed);
    if filters.is_empty() {
        if !crate::prompt::interactive() {
            return 0;
        }
        match Filters::prompt() {
            Some(f) => filters = f,
            None => return 0,
        }
    }

    let list = match get_filtered(&filters) {
        Ok(v) => v,
        Err(e) => {
            logger::fail(&format!("Failed to load subscriptions: {}", e));
            return 1;
        }
    };

    if !should_proceed("Delete", list.len(), parsed.flag("force")) {
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
    let raw_tag = match parsed.str("add") {
        Some(v) => v,
        None if crate::prompt::interactive() => {
            match crate::prompt::input("Tag name", None, None) {
                Some(v) => v,
                None => return 0,
            }
        }
        None => {
            logger::fail("Tag name is required");
            return 1;
        }
    };
    let tag_name = raw_tag.trim().to_string();
    if tag_name.is_empty() {
        logger::fail("Tag name is required");
        return 1;
    }

    let mut filters = Filters::from_args(parsed);
    if filters.is_empty() {
        if !crate::prompt::interactive() {
            return 0;
        }
        match Filters::prompt() {
            Some(f) => filters = f,
            None => return 0,
        }
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
        parsed.flag("force"),
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
    let raw_tag = match parsed.str("remove") {
        Some(v) => v,
        None if crate::prompt::interactive() => {
            match crate::prompt::input("Tag name", None, None) {
                Some(v) => v,
                None => return 0,
            }
        }
        None => {
            logger::fail("Tag name is required");
            return 1;
        }
    };
    let tag_name = raw_tag.trim().to_string();
    if tag_name.is_empty() {
        logger::fail("Tag name is required");
        return 1;
    }

    let mut filters = Filters::from_args(parsed);
    if filters.is_empty() {
        if !crate::prompt::interactive() {
            return 0;
        }
        match Filters::prompt() {
            Some(f) => filters = f,
            None => return 0,
        }
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
        parsed.flag("force"),
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
