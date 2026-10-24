use super::response::Resp;
use super::schema::validate_tool_call;

mod analytics;
mod bulk;
mod metadata;
mod subscriptions;
mod summary;

use analytics::*;
use bulk::*;
use metadata::*;
use subscriptions::*;
use summary::*;

pub(super) fn call_tool(
    name: &str,
    args: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Resp {
    if let Some(err) = validate_tool_call(name, args) {
        return Resp::err(err);
    }
    match name {
        "list_subscriptions" => handle_list_subscriptions(args),
        "get_subscription" => handle_get_subscription(args),
        "search_subscriptions" => handle_search_subscriptions(args),
        "add_subscription" => handle_add_subscription(args),
        "delete_subscription" => handle_delete_subscription(args),
        "get_summary" => handle_get_summary(args),
        "get_upcoming" => handle_get_upcoming(args),
        "get_calendar" => handle_get_calendar(args),
        "export_data" => handle_export_data(args),
        "edit_subscription" => handle_edit_subscription(args),
        "get_history" => handle_get_history(args),
        "get_analytics" => handle_get_analytics(args),
        "get_forecast" => handle_get_forecast(args),
        "compare" => handle_compare(args),
        "bulk_operations" => handle_bulk_operations(args),
        "get_trials" => handle_get_trials(args),
        "list_tags" => handle_list_tags(args),
        "get_tag_subscriptions" => handle_get_tag_subscriptions(args),
        "get_usage_total" => handle_get_usage_total(args),
        "list_usage" => handle_list_usage(args),
        _ => Resp::err(format!("Unknown tool: {}", name)),
    }
}
