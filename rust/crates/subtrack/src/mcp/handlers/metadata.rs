use crate::db;
use crate::json::quote_string as js_str;
use crate::mcp::compact::{llm_usage_compact, subs_compact, trial_compact};
use crate::mcp::response::{Resp, arg_num, arg_present, arg_str};

pub(super) fn handle_get_trials(args: Option<&serde_json::Map<String, serde_json::Value>>) -> Resp {
    let expiring = arg_num(args, "expiring_soon").map(|n| n as i64);
    let entries = if let Some(days) = expiring {
        db::get_trials_expiring_soon(days)
    } else {
        db::get_trials()
    };
    match entries {
        Ok(list) => {
            let items: Vec<String> = list.iter().map(trial_compact).collect();
            Resp::ok(format!("[{}]", items.join(",")))
        }
        Err(e) => Resp::err(e.to_string()),
    }
}

pub(super) fn handle_list_tags(_args: Option<&serde_json::Map<String, serde_json::Value>>) -> Resp {
    match db::get_tags_with_count() {
        Ok(tags) => {
            let items: Vec<String> = tags
                .iter()
                .map(|(name, count)| format!("{{\"name\":{},\"count\":{}}}", js_str(name), count))
                .collect();
            Resp::ok(format!("[{}]", items.join(",")))
        }
        Err(e) => Resp::err(e.to_string()),
    }
}

pub(super) fn handle_get_tag_subscriptions(
    args: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Resp {
    let tag = match arg_str(args, "tag") {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return Resp::err("tag is required"),
    };
    let names: Vec<String> = tag
        .split(',')
        .map(|t| t.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if names.is_empty() {
        return Resp::err("tag is required");
    }
    let opts = db::QueryOptions {
        tags: names,
        include_archived: true,
        ..Default::default()
    };
    match db::get_subscriptions(&opts) {
        Ok(subs) => Resp::ok(subs_compact(&subs)),
        Err(e) => Resp::err(e.to_string()),
    }
}

pub(super) fn handle_get_usage_total(
    args: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Resp {
    let (from, to) = if arg_present(args, "from") && arg_present(args, "to") {
        (
            arg_str(args, "from").unwrap_or("").to_string(),
            arg_str(args, "to").unwrap_or("").to_string(),
        )
    } else {
        crate::date::get_period_date_range("monthly")
    };
    let total = db::get_llm_usage_total(&from, &to).unwrap_or(0);
    let (input_tokens, output_tokens) = db::get_llm_usage_token_total(&from, &to).unwrap_or((0, 0));
    let by_provider = db::get_llm_usage_total_by_provider(&from, &to).unwrap_or_default();
    let by_model = db::get_llm_usage_total_by_model(&from, &to).unwrap_or_default();

    let providers: Vec<String> = by_provider
        .iter()
        .map(|(p, t)| format!("{{\"provider\":{},\"total\":{}}}", js_str(p), t))
        .collect();
    let models: Vec<String> = by_model
        .iter()
        .map(|(m, p, t, i, o)| {
            format!(
                "{{\"model\":{},\"provider\":{},\"total\":{},\"inputTokens\":{},\"outputTokens\":{}}}",
                js_str(m),
                js_str(p),
                t,
                i,
                o
            )
        })
        .collect();
    let body = format!(
        "\"from\":{},\"to\":{},\"total\":{},\"tokens\":{{\"inputTokens\":{},\"outputTokens\":{}}},\"byProvider\":[{}],\"byModel\":[{}]",
        js_str(&from),
        js_str(&to),
        total,
        input_tokens,
        output_tokens,
        providers.join(","),
        models.join(","),
    );
    Resp::ok(format!("{{{}}}", body))
}

pub(super) fn handle_list_usage(args: Option<&serde_json::Map<String, serde_json::Value>>) -> Resp {
    let limit = arg_num(args, "limit").map(|n| n as i64).unwrap_or(100);
    match db::get_llm_usage(
        arg_str(args, "provider"),
        arg_str(args, "from"),
        arg_str(args, "to"),
        Some(0.0),
        Some(limit),
        None,
    ) {
        Ok(entries) => {
            let items: Vec<String> = entries.iter().map(llm_usage_compact).collect();
            Resp::ok(format!("[{}]", items.join(",")))
        }
        Err(e) => Resp::err(e.to_string()),
    }
}
