//! Command validation and effectful subscription, usage, and FX reads.

use super::{ApiUsage, Conversion, calculate, render};
use crate::date::get_period_date_range;
use crate::db::{
    QueryOptions, Subscription, get_llm_usage_total, get_llm_usage_total_by_provider,
    get_subscriptions,
};
use crate::logger::{self, info, warn};
use crate::parser::ParsedArgs;

fn active_subs() -> std::io::Result<Vec<Subscription>> {
    Ok(get_subscriptions(&QueryOptions::default())?
        .into_iter()
        .filter(|s| crate::domain::not_cancelled(&s.status))
        .collect())
}

/// Validate the period, read non-cancelled subscriptions, and render payment totals.
pub(super) fn run_payment(parsed: &ParsedArgs) -> i32 {
    let period = parsed
        .str("period")
        .unwrap_or_else(|| "monthly".to_string());
    if crate::domain::Cycle::parse_named(&period).is_none() {
        logger::fail(
            "period must be one of: weekly, bi-weekly, monthly, quarterly, semi-annual, yearly",
        );
        return 1;
    }

    // A failed read must not fall through to the empty-state or zero-total
    // rendering: fail() marks exit code 1, then we stop here.
    let subs = match active_subs() {
        Ok(s) => s,
        Err(e) => {
            logger::fail(&format!("Failed to read subscriptions: {}", e));
            return 1;
        }
    };

    let currency = parsed.str("currency");
    let api = parsed.flag("api");
    let method = parsed.flag("method");

    render_payment(
        &subs,
        &period,
        currency.as_deref(),
        api,
        method,
        parsed.flag("json"),
    );
    0
}

fn render_payment(
    subs: &[Subscription],
    period: &str,
    currency: Option<&str>,
    include_api: bool,
    by_method: bool,
    json: bool,
) {
    // Empty output must not query API usage or load/convert FX rates.
    if subs.is_empty() {
        if json {
            logger::out_str(&render::empty_json(period));
        } else {
            info("No subscriptions found — try `subtrack add`");
        }
        return;
    }

    let entries = calculate::entries(subs, period);
    let api = include_api.then(|| {
        let (from, to) = get_period_date_range(period);
        ApiUsage {
            total: get_llm_usage_total(&from, &to).unwrap_or(0),
            by_provider: get_llm_usage_total_by_provider(&from, &to).unwrap_or_default(),
        }
    });

    let conversion = currency.map(|target| {
        let rates = crate::fx::fetch_rates();
        let amounts: Vec<(f64, String)> = entries
            .iter()
            .map(|e| (e.converted_price, e.currency.clone()))
            .collect();
        let (converted, has_missing) = crate::fx::convert_amounts(&amounts, target, &rates);
        if has_missing {
            warn(if json {
                "Some prices could not be converted (missing rate); excluded from totals"
            } else {
                "Some prices could not be converted (missing rate)"
            });
        }
        let total = calculate::converted_total(&converted);
        // JSON reports raw API cents; only text converts API cost into the target.
        let api_total = if !json && let Some(api) = api.as_ref().filter(|usage| usage.total > 0) {
            let amount =
                crate::fx::try_convert(api.total as f64 / 100.0, "USD", target, &rates.rates);
            if amount.is_none() {
                warn("Could not convert API cost to target currency");
            }
            amount
        } else {
            None
        };
        Conversion {
            currency: target.to_string(),
            amounts: converted,
            total,
            api_total,
        }
    });

    if json {
        let total = conversion.as_ref().map_or_else(
            || calculate::json_total(&entries),
            |converted| converted.total,
        );
        let methods = if by_method {
            calculate::json_methods(
                &entries,
                currency,
                conversion.as_ref().map(|c| c.amounts.as_slice()),
            )
        } else {
            Vec::new()
        };
        logger::out_str(&render::json(
            subs,
            period,
            currency,
            api.as_ref(),
            &entries,
            total,
            methods,
        ));
    } else {
        // With --currency the text branch ignores --method entirely.
        let groups = if conversion.is_none() {
            calculate::currency_totals(subs, period)
        } else {
            Default::default()
        };
        let methods =
            (by_method && conversion.is_none()).then(|| calculate::text_methods(&entries));
        render::text(
            period,
            api.as_ref(),
            conversion.as_ref(),
            &groups,
            methods.as_ref(),
        );
    }
}
