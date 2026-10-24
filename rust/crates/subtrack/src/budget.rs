//! `budget` command — port of `budget.ts` (handleBudget / resolveBudget).

use crate::ansi;
use crate::config::{BudgetEntry, load_config};
use crate::date::period_factor;
use crate::db::{QueryOptions, Subscription, get_subscriptions};
use crate::fx::{FxRates, fetch_rates, try_convert};
use crate::json::quote_string;
use crate::logger::{self, info, log, warn};
use crate::parser::ParsedArgs;
use crate::price::{format_price, round_currency};

/// Resolved budget to compare against (port of `ResolvedBudget`).
struct ResolvedBudget {
    name: Option<String>,
    amount: f64,
    currency: String,
    categories: Option<Vec<String>>,
    period: Option<String>,
}

/// Port of `resolveBudget`. Returns `None` when no budget is configured.
fn resolve_budget(period: &str, name: Option<&str>) -> Option<ResolvedBudget> {
    let config = load_config();

    if let Some(name) = name {
        let entry: &BudgetEntry = config.budgets.iter().find(|b| b.name == name)?;
        return Some(ResolvedBudget {
            name: Some(entry.name.clone()),
            amount: entry.amount,
            currency: entry
                .currency
                .clone()
                .filter(|c| !c.is_empty())
                .unwrap_or_else(|| config.default_currency.clone()),
            categories: if entry.categories.is_empty() {
                None
            } else {
                Some(entry.categories.clone())
            },
            period: entry.period.clone(),
        });
    }

    if period == "yearly" {
        let amount = config
            .yearly_budget
            .or({
                if config.monthly_budget > 0.0 {
                    Some(config.monthly_budget * 12.0)
                } else {
                    None
                }
            })
            .unwrap_or(0.0);
        if amount > 0.0 {
            return Some(ResolvedBudget {
                name: None,
                amount,
                currency: config.default_currency,
                categories: None,
                period: None,
            });
        }
        return None;
    }

    if config.monthly_budget > 0.0 {
        return Some(ResolvedBudget {
            name: None,
            amount: config.monthly_budget,
            currency: config.default_currency,
            categories: None,
            period: None,
        });
    }
    None
}

/// Port of `convertTotals`: sum per-currency totals into a single target.
fn convert_totals(
    totals: &std::collections::BTreeMap<String, f64>,
    target: &str,
    rates: &FxRates,
) -> (f64, bool) {
    let mut sum = 0.0;
    let mut missing = false;
    for (ccy, total) in totals {
        if ccy == target {
            sum += total;
            continue;
        }
        match try_convert(*total, ccy, target, &rates.rates) {
            Some(v) => sum += v,
            None => missing = true,
        }
    }
    (sum, missing)
}

/// Port of `calcSubTotal` with an explicit target period.
fn calc_sub_total(
    subs: &[Subscription],
    rates: Option<&FxRates>,
    target_currency: Option<&str>,
    period: &str,
) -> std::collections::BTreeMap<String, f64> {
    subs.iter()
        .filter(|sub| crate::domain::not_cancelled(&sub.status))
        .fold(std::collections::BTreeMap::new(), |mut totals, sub| {
            let normalized = sub.price * period_factor(&sub.cycle, period);
            let (bucket, amount) = match (target_currency, rates) {
                (Some(tc), Some(r)) => try_convert(normalized, &sub.currency, tc, &r.rates)
                    .map_or((sub.currency.as_str(), normalized), |converted| {
                        (tc, converted)
                    }),
                _ => (sub.currency.as_str(), normalized),
            };
            *totals.entry(bucket.to_owned()).or_insert(0.0) += amount;
            totals
        })
}

/// Width of the budget meter in cells.
const METER_CELLS: usize = 20;

/// One-line `[████░░░] 41%` fill meter for the text output.
///
/// The bar saturates at 100% while the percentage keeps rising, so going
/// over budget still reads clearly. Colors follow the states: green below
/// three quarters, yellow below full, red at full or over.
fn budget_meter(spending: f64, budget: f64) -> String {
    let ratio = (spending / budget).max(0.0);
    let filled = (ratio.min(1.0) * METER_CELLS as f64).round() as usize;
    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(METER_CELLS - filled));
    let colored = if ratio < 0.75 {
        ansi::green(&bar)
    } else if ratio < 1.0 {
        ansi::yellow(&bar)
    } else {
        ansi::red(&bar)
    };
    format!("[{}] {}%", colored, (ratio * 100.0).round() as i64)
}

pub fn run_budget(parsed: &ParsedArgs) -> i32 {
    let period = parsed
        .str("period")
        .unwrap_or_else(|| "monthly".to_string());
    let period = if period == "yearly" {
        "yearly"
    } else {
        "monthly"
    };
    let check = parsed.flag("check");
    let json = parsed.flag("json");
    let name = parsed.str("name").filter(|n| !n.is_empty());
    let currency = parsed.str("currency").filter(|c| !c.is_empty());

    let budget = resolve_budget(period, name.as_deref());
    let Some(budget) = budget else {
        if json {
            logger::out_str(&format!(
                "{{\n  \"set\": false,\n  \"period\": {},\n  \"budgetName\": {}\n}}\n",
                quote_string(period),
                name.as_deref()
                    .map(quote_string)
                    .unwrap_or_else(|| "null".to_string())
            ));
            return 0;
        }
        if let Some(name) = &name {
            info(&format!(
                "Budget \"{}\" not found — check: subtrack config set budgets ...",
                name
            ));
            return 0;
        }
        let config = load_config();
        if period == "monthly" && config.yearly_budget.unwrap_or(0.0) > 0.0 {
            info(
                "Monthly budget not set — use --period yearly to compare against yearlyBudget, \
                 or set: subtrack config set monthlyBudget <amount>",
            );
            return 0;
        }
        info(if period == "yearly" {
            "No budget set. Use: subtrack config set yearlyBudget <amount>"
        } else {
            "No budget set. Use: subtrack config set monthlyBudget <amount>"
        });
        return 0;
    };

    // Named budgets may carry their own period (e.g. yearly vs monthly compare).
    let compare_period = budget.period.clone().unwrap_or_else(|| period.to_string());
    let compare_period: &str = if compare_period == "yearly" {
        "yearly"
    } else {
        "monthly"
    };

    // FX rates fetched when any conversion may be needed.
    let rates = if currency.is_some() || !budget.currency.is_empty() {
        Some(fetch_rates())
    } else {
        None
    };

    // Named budgets can filter by categories (tags).
    let subs = get_subscriptions(&QueryOptions::default()).unwrap_or_default();
    let filtered: Vec<Subscription> = match &budget.categories {
        Some(cats) if !cats.is_empty() => subs
            .into_iter()
            .filter(|s| s.tags.iter().any(|t| cats.contains(t)))
            .collect(),
        _ => subs,
    };

    let totals = calc_sub_total(
        &filtered,
        rates.as_ref(),
        currency.as_deref(),
        compare_period,
    );

    let period_label = if compare_period == "yearly" {
        "year"
    } else {
        "month"
    };
    let period_name = if compare_period == "yearly" {
        "Yearly"
    } else {
        "Monthly"
    };

    // Determine a single comparable (currency, spending) pair.
    let display_currency: Option<String>;
    let spending: f64;
    let mut budget_display = budget.amount;

    if let (Some(target), Some(r)) = (&currency, &rates) {
        display_currency = Some(target.clone());
        spending = totals.values().sum();
        // Convert budget into display currency for a fair comparison.
        match try_convert(budget.amount, &budget.currency, target, &r.rates) {
            Some(v) => budget_display = v,
            None => warn(&format!(
                "Cannot convert budget from {} to {} — missing rate",
                budget.currency, target
            )),
        }
    } else if let (Some(r), true) = (&rates, !budget.currency.is_empty()) {
        let (sum, missing) = convert_totals(&totals, &budget.currency, r);
        if missing {
            warn("Some prices could not be converted (missing rate)");
        }
        display_currency = Some(budget.currency.clone());
        spending = sum;
    } else {
        info("Cannot compare against budget — multiple currencies. Use --currency to convert.");
        return 0;
    }
    let currency_str = display_currency.as_deref().unwrap_or("USD");

    let remaining = budget_display - spending;
    let over = remaining < 0.0;

    if json {
        logger::out_str(&format!(
            "{{\n  \"set\": true,\n  \"period\": {},\n  \"budgetName\": {},\n  \"budget\": {},\n  \"budgetCurrency\": {},\n  \"spending\": {},\n  \"currency\": {},\n  \"remaining\": {},\n  \"over\": {}\n}}\n",
            quote_string(compare_period),
            budget
                .name
                .as_deref()
                .map(quote_string)
                .unwrap_or_else(|| "null".to_string()),
            crate::json::number(round_currency(budget_display)),
            quote_string(currency_str),
            crate::json::number(round_currency(spending)),
            quote_string(currency_str),
            crate::json::number(round_currency(remaining)),
            if over { "true" } else { "false" }
        ));
        return 0;
    }

    let budget_label = match &budget.name {
        Some(n) => format!("Budget ({})", n),
        None => "Budget".to_string(),
    };
    log(&format!(
        "{} spending: {}/{}",
        period_name,
        ansi::bold(&ansi::yellow(&format_price(
            round_currency(spending),
            currency_str
        ))),
        period_label
    ));
    let mut budget_line = format!(
        "{}: {}/{}",
        budget_label,
        ansi::bold(&ansi::yellow(&format_price(
            round_currency(budget_display),
            currency_str
        ))),
        period_label
    );
    if budget.currency != currency_str {
        budget_line.push_str(&format!(" ({})", budget.currency));
    }
    log(&budget_line);
    // Visual fill meter — cosmetic only; the JSON path keeps raw numbers.
    if budget_display > 0.0 {
        log(&budget_meter(spending, budget_display));
    }
    if over {
        log(&format!(
            "Over budget: {}",
            ansi::red(&format_price(round_currency(-remaining), currency_str))
        ));
    } else {
        log(&format!(
            "Remaining: {}",
            ansi::green(&format_price(round_currency(remaining), currency_str))
        ));
    }
    if let Some(cats) = &budget.categories
        && !cats.is_empty()
    {
        log(&ansi::dim(&format!(
            "(filtered by categories: {})",
            cats.join(", ")
        )));
    }

    if check && over {
        return 1;
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_meter() {
        let m = budget_meter(0.0, 40.0);
        assert!(m.contains("░".repeat(METER_CELLS).as_str()), "meter: {m}");
        assert!(m.contains("0%"), "meter: {m}");
    }

    #[test]
    fn quarter_meter_fills_five_cells() {
        let m = budget_meter(10.0, 40.0);
        // 25% → 5 fill cells
        assert!(m.contains("█".repeat(5).as_str()), "meter: {m}");
        assert!(m.contains("░".repeat(15).as_str()), "meter: {m}");
        assert!(m.contains("25%"), "meter: {m}");
    }

    #[test]
    fn over_budget_saturates_and_reports_higher_percent() {
        let m = budget_meter(125.0, 100.0);
        assert!(m.contains("█".repeat(METER_CELLS).as_str()), "meter: {m}");
        assert!(m.contains("125%"), "meter: {m}");
        // no empty cells
        assert!(!m.contains('░'), "meter: {m}");
    }
}
