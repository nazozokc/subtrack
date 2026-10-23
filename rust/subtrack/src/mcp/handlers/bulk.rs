use crate::db::{self, SqlValue, Subscription};
use crate::json::quote_string as js_str;
use crate::mcp::response::{Resp, arg_bool, arg_str};
use crate::mcp::schema::{MAX_BULK_AFFECTED, MAX_TAG_COUNT};

pub(super) fn handle_bulk_operations(
    args: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Resp {
    let action = arg_str(args, "action").unwrap_or("").to_string();
    if action == "delete" && arg_bool(args, "confirm") != Some(true) {
        return Resp::err("Bulk delete requires confirm: true");
    }

    let mut filter_tag: Option<String> = None;
    let mut filter_status: Option<String> = None;
    let mut filter_name: Option<String> = None;
    if let Some(t) = arg_str(args, "filter_tag") {
        filter_tag = Some(t.to_string());
    }
    if let Some(s) = arg_str(args, "filter_status") {
        filter_status = Some(s.to_string());
    }
    if let Some(n) = arg_str(args, "filter_name") {
        filter_name = Some(n.to_string());
    }

    let subs = match db::get_subscriptions(&db::QueryOptions::default()) {
        Ok(s) => s,
        Err(e) => return Resp::err(e.to_string()),
    };
    let mut matched: Vec<Subscription> = subs;
    if let Some(tag) = &filter_tag {
        let set: Vec<String> = tag.split(',').map(|t| t.trim().to_string()).collect();
        matched.retain(|s| s.tags.iter().any(|t| set.contains(t)));
    }
    if let Some(status) = &filter_status {
        matched.retain(|s| &s.status == status);
    }
    if let Some(name) = &filter_name {
        let lower = name.to_lowercase();
        matched.retain(|s| s.name.to_lowercase().contains(&lower));
    }

    let affected: Vec<Subscription> = if action == "status" {
        matched
            .into_iter()
            .filter(|s| crate::domain::not_cancelled(&s.status))
            .collect()
    } else {
        matched
    };
    let affected_ids: Vec<i64> = affected.iter().map(|s| s.id).collect();
    if affected_ids.len() > MAX_BULK_AFFECTED {
        return Resp::err(format!(
            "Bulk operation would affect {} subscriptions (max {})",
            affected_ids.len(),
            MAX_BULK_AFFECTED
        ));
    }

    let conn = match db::open_db() {
        Ok(c) => c,
        Err(e) => return Resp::err(e.to_string()),
    };
    let mut result_count = 0usize;
    let mut errors: Vec<String> = Vec::new();
    let report = |id: i64, errors: &mut Vec<String>, msg: String| {
        errors.push(format!("id {}: {}", id, msg));
    };

    match action.as_str() {
        "status" => {
            let target_status = arg_str(args, "status").unwrap_or("active").to_string();
            if !crate::validation::is_valid_status(&target_status) {
                return Resp::err(format!(
                    "Invalid status \"{}\". Use: active, paused, cancelled, archived",
                    target_status
                ));
            }
            for id in &affected_ids {
                match db::update_subscription(
                    &conn,
                    *id,
                    &[("status", SqlValue::Text(target_status.clone()))],
                    false,
                ) {
                    Ok(true) => result_count += 1,
                    Ok(false) => {}
                    Err(e) => report(*id, &mut errors, e.to_string()),
                }
            }
        }
        "delete" => {
            for id in &affected_ids {
                match db::remove_subscription(&conn, *id, false) {
                    Ok(true) => result_count += 1,
                    Ok(false) => {}
                    Err(e) => report(*id, &mut errors, e.to_string()),
                }
            }
        }
        "tag_add" => {
            let tag_name = arg_str(args, "tag_name").unwrap_or("").trim().to_string();
            if tag_name.is_empty() {
                return Resp::err("tag_name is required for tag_add action");
            }
            if tag_name.contains(',') {
                return Resp::err("tag_name must be a single tag");
            }
            if let Err(e) = crate::validation::validate_tags(&tag_name) {
                return Resp::err(format!("Invalid tag name: {}", e));
            }
            for s in &affected {
                if !s.tags.contains(&tag_name) {
                    if s.tags.len() >= MAX_TAG_COUNT {
                        report(
                            s.id,
                            &mut errors,
                            format!("Maximum {} tags allowed", MAX_TAG_COUNT),
                        );
                        continue;
                    }
                    let mut tags = s.tags.clone();
                    tags.push(tag_name.clone());
                    match db::update_subscription(&conn, s.id, &[], false) {
                        Ok(_) => match db::set_subscription_tags(&conn, s.id, &tags, false) {
                            Ok(_) => result_count += 1,
                            Err(e) => report(s.id, &mut errors, e.to_string()),
                        },
                        Err(e) => report(s.id, &mut errors, e.to_string()),
                    }
                }
            }
        }
        "tag_remove" => {
            let tag_name = arg_str(args, "tag_name").unwrap_or("").trim().to_string();
            if tag_name.is_empty() {
                return Resp::err("tag_name is required for tag_remove action");
            }
            if tag_name.contains(',') {
                return Resp::err("tag_name must be a single tag");
            }
            if let Err(e) = crate::validation::validate_tags(&tag_name) {
                return Resp::err(format!("Invalid tag name: {}", e));
            }
            for s in &affected {
                if s.tags.contains(&tag_name) {
                    let tags: Vec<String> =
                        s.tags.iter().filter(|t| **t != tag_name).cloned().collect();
                    match db::set_subscription_tags(&conn, s.id, &tags, false) {
                        Ok(_) => result_count += 1,
                        Err(e) => report(s.id, &mut errors, e.to_string()),
                    }
                }
            }
        }
        _ => {
            return Resp::err(format!(
                "Unknown bulk action: {}. Use: status, delete, tag_add, tag_remove",
                action
            ));
        }
    }

    if result_count > 0 {
        let _ = db::save_db(&conn);
    }

    let filters = format!(
        "{{{} }}",
        [
            filter_tag
                .as_ref()
                .map(|t| format!("\"tag\":{}", js_str(t))),
            filter_status
                .as_ref()
                .map(|s| format!("\"status\":{}", js_str(s))),
            filter_name
                .as_ref()
                .map(|n| format!("\"name\":{}", js_str(n))),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(",")
    );
    let filters = filters.replace("{} ", "{}").replace("{ }", "{}");
    let ids: Vec<String> = affected_ids.iter().map(|i| i.to_string()).collect();
    let errs: Vec<String> = errors.iter().map(|e| js_str(e)).collect();
    let body = format!(
        "\"action\":{},\"filters\":{},\"matchedCount\":{},\"affectedCount\":{},\"affectedIds\":[{}],\"errors\":[{}]",
        js_str(&action),
        filters,
        affected.len(),
        result_count,
        ids.join(","),
        errs.join(","),
    );
    Resp::ok(format!("{{{}}}", body))
}
