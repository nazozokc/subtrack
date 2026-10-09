//! `list` command — port of `commands/core.ts` (listCommand.run) +
//! `subscription/core.ts` (handleList, spreadSubscription pipeline).

use crate::config::load_config;
use crate::date::{today, today_string};
use crate::db::{get_llm_usage_total, get_llm_usage_total_by_provider, get_subscriptions, QueryOptions};
use crate::display::{show_api_usage, spread_subscription};
use crate::fx::{fetch_converted_subs, Converted};
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

fn parse_num(s: &str) -> f64 {
    s.parse::<f64>().unwrap_or(f64::NAN)
}

/// Port of listCommand.run — validation + dispatch. Returns 1 on failure.
pub fn run_list(parsed: &ParsedArgs) -> i32 {
    // limit/offset must be integers
    let limit = parsed.str("limit").map(|s| parse_num(&s));
    if let Some(v) = limit {
        if v.is_nan() || v < 1.0 || v.fract() != 0.0 {
            fail("limit must be a positive integer");
            return 1;
        }
    }
    let offset = parsed.str("offset").map(|s| parse_num(&s));
    if let Some(v) = offset {
        if v.is_nan() || v < 0.0 || v.fract() != 0.0 {
            fail("offset must be a non-negative integer");
            return 1;
        }
    }
    // status must be one of the known values
    if let Some(s) = parsed.str("status") {
        if !["active", "paused", "cancelled", "archived"].contains(&s.as_str()) {
            fail("status must be one of: active, paused, cancelled, archived");
            return 1;
        }
    }
    // price bounds
    let min_price = parsed.str("min-price").map(|s| parse_num(&s));
    if let Some(v) = min_price {
        if v.is_nan() || v < 0.0 {
            fail("min-price must be a non-negative number");
            return 1;
        }
    }
    let max_price = parsed.str("max-price").map(|s| parse_num(&s));
    if let Some(v) = max_price {
        if v.is_nan() || v < 0.0 {
            fail("max-price must be a non-negative number");
            return 1;
        }
    }
    if let (Some(min), Some(max)) = (min_price, max_price) {
        if min > max {
            fail("min-price cannot be greater than max-price");
            return 1;
        }
    }

    let json = parsed.bool("json") == Some(true);
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
        desc: parsed.bool("desc") == Some(true),
        limit: limit.filter(|v| !v.is_nan()).map(|v| v as i64),
        offset: offset.filter(|v| !v.is_nan()).map(|v| v as i64),
        include_archived: parsed.bool("include-archived") == Some(true),
        status: parsed.str("status"),
        min_price,
        max_price,
        tags: tags.clone(),
    };

    let list_result = get_subscriptions(&options);
    let list = match list_result {
        Ok(l) => l,
        Err(e) => {
            fail(&format!("Failed to read subscriptions: {}", e));
            return 1;
        }
    };

    if json {
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
            api: parsed.bool("api") == Some(true),
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
    let mut display_list = list;
    let mut display_currency = options.currency.clone();

    // Currency conversion: when a target currency is requested but some
    // subscriptions are in other currencies, fetch rates and convert.
    if let Some(cu) = options.currency.as_deref() {
        let any_other = display_list.iter().any(|s| s.currency != cu);
        if any_other {
            info("Fetching the latest exchange rates...");
            match fetch_converted_subs(&display_list, cu) {
                Some(Converted { list, has_missing }) => {
                    success("Exchange rates updated");
                    display_list = list;
                    if has_missing {
                        warn("Some prices could not be converted (missing rate). They are shown in original currency.");
                    }
                }
                None => {
                    warn("Failed to fetch exchange rates; showing in original currencies");
                    display_currency = None;
                }
            }
        }
    }

    spread_subscription(
        &display_list,
        display_currency.as_deref(),
        options.show_notes,
        options.show_method,
        options.show_contract,
        options.show_vendor,
    );

    if options.api {
        let now = today();
        let month = now.month();
        let from = format!("{}-{:02}-01", now.year(), month);
        let to = today_string();
        let period_label = format!("{} {}", LONG_MONTHS[(month - 1) as usize], now.year());
        let total = get_llm_usage_total(&from, &to).unwrap_or(0);
        let by_provider = get_llm_usage_total_by_provider(&from, &to).unwrap_or_default();
        show_api_usage(total, &by_provider, &period_label)
    }
}