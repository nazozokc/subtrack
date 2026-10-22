//! `list` command — port of `commands/core.ts` (listCommand.run) +
//! `subscription/core.ts` (handleList, spreadSubscription pipeline).

use crate::config::load_config;
use crate::date::{today, today_string};
use crate::db::{
    QueryOptions, get_llm_usage_total, get_llm_usage_total_by_provider, get_subscriptions,
};
use crate::display::{show_api_usage, spread_subscription};
use crate::domain::{Status, f64_arg, int_arg};
use crate::fx::{Converted, fetch_converted_subs};
use crate::json::list_json;
use crate::logger::{fail, info, out_str, success, warn};
use crate::parser::ParsedArgs;
use chrono::Datelike;

const LONG_MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Port of listCommand.run — validation + dispatch. Returns 1 on failure.
pub fn run_list(parsed: &ParsedArgs) -> i32 {
    // limit/offset must be integers
    let limit = match int_arg(
        parsed,
        "limit",
        |v| v >= 1.0,
        "limit must be a positive integer",
    ) {
        Err(()) => return 1,
        Ok(limit) => limit,
    };
    let offset = match int_arg(
        parsed,
        "offset",
        |v| v >= 0.0,
        "offset must be a non-negative integer",
    ) {
        Err(()) => return 1,
        Ok(offset) => offset,
    };
    let status = match parsed.str("status") {
        Some(raw) => match Status::parse(&raw) {
            Ok(status) => Some(status),
            Err(_) => {
                fail("status must be one of: active, paused, cancelled, archived");
                return 1;
            }
        },
        None => None,
    };
    let min_price = match f64_arg(
        parsed,
        "min-price",
        |v| v >= 0.0,
        "min-price must be a non-negative number",
    ) {
        Err(()) => return 1,
        Ok(v) => v,
    };
    let max_price = match f64_arg(
        parsed,
        "max-price",
        |v| v >= 0.0,
        "max-price must be a non-negative number",
    ) {
        Err(()) => return 1,
        Ok(v) => v,
    };
    if let (Some(min), Some(max)) = (min_price, max_price)
        && min > max
    {
        fail("min-price cannot be greater than max-price");
        return 1;
    }

    let tags: Vec<String> = parsed
        .str("tags")
        .map(|t| {
            t.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let options = QueryOptions {
        sort: parsed.str("sort"),
        desc: parsed.flag("desc"),
        limit,
        offset,
        include_archived: parsed.flag("include-archived"),
        status: status.map(|s| s.to_string()),
        min_price,
        max_price,
        tags,
    };

    let list = match get_subscriptions(&options) {
        Ok(l) => l,
        Err(e) => {
            fail(&format!("Failed to read subscriptions: {e}"));
            return 1;
        }
    };

    if parsed.flag("json") {
        out_str(&list_json(&list));
        out_str("\n");
        return 0;
    }

    let cfg = load_config();
    let show_notes = parsed.bool("notes").unwrap_or(cfg.list_show_notes);
    let show_method = parsed.bool("method").unwrap_or(cfg.list_show_method);
    let show_contract = parsed.bool("contract").unwrap_or(false);
    let show_vendor = parsed.bool("vendor").unwrap_or(false);

    handle_list(
        list,
        HandleOptions {
            currency: parsed.str("currency"),
            show_notes,
            show_method,
            show_contract,
            show_vendor,
            api: parsed.flag("api"),
        },
    );
    0
}

pub struct HandleOptions {
    pub currency: Option<String>,
    pub show_notes: bool,
    pub show_method: bool,
    pub show_contract: bool,
    pub show_vendor: bool,
    pub api: bool,
}

/// Port of handleList + showApiUsage dispatch (`subscription/core.ts`).
pub fn handle_list(list: Vec<crate::db::Subscription>, options: HandleOptions) {
    let HandleOptions {
        currency,
        show_notes,
        show_method,
        show_contract,
        show_vendor,
        api,
    } = options;
    let mut display_list = list;
    let mut display_currency = currency;

    // Currency conversion: when a target currency is requested but some
    // subscriptions are in other currencies, fetch rates and convert.
    if let Some(cu) = display_currency
        .as_deref()
        .filter(|cu| display_list.iter().any(|s| s.currency != *cu))
    {
        info("Fetching the latest exchange rates...");
        match fetch_converted_subs(&display_list, cu) {
            Some(Converted { list, has_missing }) => {
                success("Exchange rates updated");
                display_list = list;
                if has_missing {
                    warn(
                        "Some prices could not be converted (missing rate). They are shown in original currency.",
                    );
                }
            }
            None => {
                warn("Failed to fetch exchange rates; showing in original currencies");
                display_currency = None;
            }
        }
    }

    spread_subscription(
        &display_list,
        display_currency.as_deref(),
        show_notes,
        show_method,
        show_contract,
        show_vendor,
    );

    if api {
        let now = today();
        let from = format!("{}-{:02}-01", now.year(), now.month());
        let to = today_string();
        let period_label = format!("{} {}", LONG_MONTHS[now.month() as usize - 1], now.year());
        let total = get_llm_usage_total(&from, &to).unwrap_or(0);
        let by_provider = get_llm_usage_total_by_provider(&from, &to).unwrap_or_default();
        show_api_usage(total, &by_provider, &period_label)
    }
}
