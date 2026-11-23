//! Daily expense ledger: `subtrack expense ...`.
//!
//! One row per day, with line items stored as JSON in `expenses.items`.
//! Commands here only validate input, call the DB layer, and render output.

use std::collections::BTreeMap;

use crate::db::{
    Expense, ExpenseItem, NewExpenseItem, append_expense_items, delete_expense_day,
    delete_expense_item, get_expense_groups, get_expense_item, get_expense_totals, get_expenses,
    update_expense_item,
};
use crate::i18n::tr;
use crate::json::{number, quote_string};
use crate::logger::{self, info};
use crate::parser::ParsedArgs;
use crate::prompt;
use crate::table::{ColumnConfig, TableBuilder, calc_column_widths, table_style, zebra_row};

const METHODS: [&str; 4] = ["cash", "card", "bank", "other"];
const GROUPS: [&str; 3] = ["category", "method", "day"];
const MAX_AMOUNT: f64 = 99_999_999.0;

fn t(text: &'static str) -> String {
    tr(text).into_owned()
}

fn fail(msg: &str) -> i32 {
    logger::fail(msg);
    1
}

// ── Validation / normalization ──────────────────────────────

fn parse_amount(value: &str) -> Result<f64, String> {
    let raw = value.trim();
    let amount = raw
        .parse::<f64>()
        .map_err(|_| format!("Invalid amount: {raw}"))?;
    if !amount.is_finite() || amount <= 0.0 || amount > MAX_AMOUNT {
        return Err(format!(
            "Amount must be greater than 0 and at most {}",
            crate::price::group_whole(MAX_AMOUNT as i64)
        ));
    }
    Ok(amount)
}

fn normalize_currency(value: &str) -> Result<String, String> {
    let code = value.trim().to_uppercase();
    if crate::validation::is_valid_currency(&code) {
        Ok(code)
    } else {
        Err(format!("Invalid currency code: {value}"))
    }
}

fn normalize_category(value: &str) -> Result<String, String> {
    let category = value.trim().to_lowercase();
    if category.is_empty() {
        return Err("Category must not be empty".to_string());
    }
    if category.chars().count() > 50 {
        return Err("Category must be at most 50 characters".to_string());
    }
    Ok(category)
}

fn normalize_method(value: &str) -> Result<String, String> {
    let method = value.trim().to_lowercase();
    if METHODS.contains(&method.as_str()) {
        Ok(method)
    } else {
        Err(format!(
            "Invalid method: {value} (choose from {})",
            METHODS.join(", ")
        ))
    }
}

fn normalize_note(value: &str) -> Option<String> {
    let note = value.trim();
    if note.is_empty() {
        None
    } else {
        Some(note.to_string())
    }
}

fn parse_date(value: &str) -> Result<String, String> {
    let raw = value.trim();
    if crate::date::to_date(raw).is_some() {
        Ok(raw.to_string())
    } else {
        Err(format!("Invalid date: {raw} (expected YYYY-MM-DD)"))
    }
}

fn parse_month(value: &str) -> Result<(String, String), String> {
    let raw = value.trim();
    let mut parts = raw.split('-');
    let year = parts
        .next()
        .and_then(|v| v.parse::<i32>().ok())
        .ok_or_else(|| format!("Invalid month: {raw} (expected YYYY-MM)"))?;
    let month = parts
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .ok_or_else(|| format!("Invalid month: {raw} (expected YYYY-MM)"))?;
    if parts.next().is_some() || !(1..=12).contains(&month) {
        return Err(format!("Invalid month: {raw} (expected YYYY-MM)"));
    }
    let last = crate::date::days_in_month(year, month);
    Ok((
        format!("{year:04}-{month:02}-01"),
        format!("{year:04}-{month:02}-{last:02}"),
    ))
}

fn current_month() -> Result<(String, String), String> {
    let today = crate::date::today_string();
    parse_month(&today[..7])
}

fn parse_item_id(value: &str) -> Result<i64, String> {
    value
        .trim()
        .parse::<i64>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(|| format!("Invalid item ID: {value}"))
}

fn resolve_list_period(parsed: &ParsedArgs) -> Result<(String, String), String> {
    if let Some(date) = parsed.str("date") {
        let date = parse_date(&date)?;
        return Ok((date.clone(), date));
    }
    resolve_period(parsed, false)
}

fn resolve_total_period(parsed: &ParsedArgs) -> Result<(String, String), String> {
    resolve_period(parsed, true)
}

fn resolve_period(parsed: &ParsedArgs, default_month: bool) -> Result<(String, String), String> {
    let from = parsed.str("from").map(|v| parse_date(&v)).transpose()?;
    let to = parsed.str("to").map(|v| parse_date(&v)).transpose()?;
    if from.is_some() || to.is_some() {
        return Ok((
            from.unwrap_or_else(|| "0000-01-01".to_string()),
            to.unwrap_or_else(|| "9999-12-31".to_string()),
        ));
    }
    if let Some(month) = parsed.str("month") {
        return parse_month(&month);
    }
    if default_month {
        return current_month();
    }
    let today = crate::date::today_string();
    Ok((today.clone(), today))
}

// Prompt validators must be plain functions (`prompt::Validator`).
fn validate_amount_prompt(value: &str) -> Result<(), String> {
    parse_amount(value).map(|_| ())
}

fn validate_currency_prompt(value: &str) -> Result<(), String> {
    normalize_currency(value).map(|_| ())
}

fn validate_category_prompt(value: &str) -> Result<(), String> {
    normalize_category(value).map(|_| ())
}

// ── Item collection (flags or interactive loop) ─────────────

/// `Ok(None)` means the user cancelled an interactive prompt.
fn collect_items(
    parsed: &ParsedArgs,
    default_currency: &str,
) -> Result<Option<Vec<NewExpenseItem>>, String> {
    let mut amount_flag = parsed.str("amount");
    let mut currency_flag = parsed.str("currency");
    let mut category_flag = parsed.str("category");
    let mut method_flag = parsed.str("method");
    let mut note_flag = parsed.str("note");
    let mut previous: Option<(String, String, String)> = None;
    let mut items = Vec::new();

    loop {
        let amount_raw = match amount_flag.take() {
            Some(v) => v,
            None if prompt::interactive() => {
                match prompt::input(&t("Amount"), None, Some(validate_amount_prompt)) {
                    Some(v) => v,
                    None => return Ok(None),
                }
            }
            None => return Err("Provide --amount or run interactively".to_string()),
        };
        let amount = parse_amount(&amount_raw)?;

        let currency_default = currency_flag
            .take()
            .or_else(|| previous.as_ref().map(|p| p.0.clone()))
            .unwrap_or_else(|| default_currency.to_string());
        let currency_raw = if prompt::interactive() {
            match prompt::input(
                &t("Currency"),
                Some(&currency_default),
                Some(validate_currency_prompt),
            ) {
                Some(v) => v,
                None => return Ok(None),
            }
        } else {
            currency_default
        };
        let currency = normalize_currency(&currency_raw)?;

        let category_default = category_flag
            .take()
            .or_else(|| previous.as_ref().map(|p| p.1.clone()))
            .unwrap_or_else(|| "other".to_string());
        let category_raw = if prompt::interactive() {
            match prompt::input(
                &t("Category"),
                Some(&category_default),
                Some(validate_category_prompt),
            ) {
                Some(v) => v,
                None => return Ok(None),
            }
        } else {
            category_default
        };
        let category = normalize_category(&category_raw)?;

        let method_default = method_flag
            .take()
            .or_else(|| previous.as_ref().map(|p| p.2.clone()))
            .unwrap_or_else(|| "other".to_string());
        let method_raw = if prompt::interactive() {
            let choices: Vec<prompt::Choice> = METHODS
                .iter()
                .map(|method| prompt::Choice::plain(method))
                .collect();
            let default_index = METHODS
                .iter()
                .position(|m| *m == method_default)
                .unwrap_or(0);
            match prompt::select(&t("Method"), &choices, Some(default_index)) {
                Some(v) => v,
                None => return Ok(None),
            }
        } else {
            method_default
        };
        let method = normalize_method(&method_raw)?;

        let note_default = note_flag.take().unwrap_or_default();
        let note_raw = if prompt::interactive() {
            // An empty default makes an empty answer valid (skip the note).
            match prompt::input(&t("Note"), Some(note_default.as_str()), None) {
                Some(v) => v,
                None => return Ok(None),
            }
        } else {
            note_default
        };

        previous = Some((currency.clone(), category.clone(), method.clone()));
        items.push(NewExpenseItem {
            amount,
            currency,
            category,
            method,
            note: normalize_note(&note_raw),
        });

        if !prompt::interactive() {
            break;
        }
        if prompt::confirm(&t("Add another item?"), false) != Some(true) {
            break;
        }
    }
    Ok(Some(items))
}

// ── Rendering helpers ───────────────────────────────────────

fn day_totals(day: &Expense) -> Vec<(String, f64)> {
    let mut map: BTreeMap<String, f64> = BTreeMap::new();
    for item in &day.items {
        *map.entry(item.currency.clone()).or_default() += item.amount;
    }
    map.into_iter().collect()
}

fn format_totals(totals: &[(String, f64)]) -> String {
    totals
        .iter()
        .map(|(currency, amount)| crate::price::format_price(*amount, currency))
        .collect::<Vec<_>>()
        .join(" · ")
}

fn totals_json(totals: &[(String, f64)]) -> String {
    totals
        .iter()
        .map(|(currency, amount)| {
            format!(
                "{{\"currency\": {}, \"amount\": {}}}",
                quote_string(currency),
                number(*amount)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn item_json(item: &ExpenseItem) -> String {
    format!(
        "      {{\n        \"id\": {},\n        \"amount\": {},\n        \"currency\": {},\n        \"category\": {},\n        \"method\": {},\n        \"note\": {}\n      }}",
        item.id,
        number(item.amount),
        quote_string(&item.currency),
        quote_string(&item.category),
        quote_string(&item.method),
        item.note
            .as_deref()
            .map(quote_string)
            .unwrap_or_else(|| "null".to_string()),
    )
}

fn render_table(
    headers: &[&str],
    rows: &[Vec<String>],
    min_widths: Vec<i64>,
    max_widths: Vec<i64>,
    aligns: &[&str],
) {
    if rows.is_empty() {
        return;
    }
    let headers: Vec<String> = headers.iter().map(|s| s.to_string()).collect();
    let config = ColumnConfig {
        headers: headers.clone(),
        min_widths,
        max_widths,
        min_avail: Some(40),
    };
    let widths = calc_column_widths(rows, &config, crate::ui::term_width(), 40);
    let theme = crate::config::display_theme();
    let col_aligns: Vec<String> = aligns.iter().map(|s| s.to_string()).collect();

    let mut body = TableBuilder::new(table_style(&theme));
    body.col_widths = widths;
    body.head = headers;
    body.word_wrap = true;
    body.wrap_on_word_boundary = true;
    body.col_aligns = col_aligns;
    for (i, row) in rows.iter().enumerate() {
        if i % 2 == 0 {
            body.push(zebra_row(row, theme.zebra))
        } else {
            body.push(row.clone())
        }
    }
    logger::log(&body.to_string());
}

fn convert_totals(totals: &[(String, f64)], target: &str) -> Result<Vec<(String, f64)>, String> {
    let rates = crate::fx::fetch_rates();
    let mut converted: BTreeMap<String, f64> = BTreeMap::new();
    for (currency, amount) in totals {
        let value = crate::fx::try_convert(*amount, currency, target, &rates.rates).ok_or_else(
            || {
                format!(
                    "No FX rate for {currency} -> {target}; use a supported currency or omit --currency"
                )
            },
        )?;
        *converted.entry(target.to_string()).or_default() += value;
    }
    Ok(converted.into_iter().collect())
}

fn convert_groups(
    groups: &[(String, String, f64)],
    target: &str,
) -> Result<Vec<(String, String, f64)>, String> {
    let rates = crate::fx::fetch_rates();
    let mut out = Vec::new();
    for (key, currency, amount) in groups {
        let value = crate::fx::try_convert(*amount, currency, target, &rates.rates).ok_or_else(
            || {
                format!(
                    "No FX rate for {currency} -> {target}; use a supported currency or omit --currency"
                )
            },
        )?;
        out.push((key.clone(), target.to_string(), value));
    }
    Ok(out)
}

// ── Handlers ────────────────────────────────────────────────

pub fn run_expense_add(parsed: &ParsedArgs) -> i32 {
    let mut date = crate::date::today_string();
    if let Some(raw) = parsed.str("date") {
        date = match parse_date(&raw) {
            Ok(v) => v,
            Err(msg) => return fail(&msg),
        };
    }
    let default_currency = crate::config::load_config().default_currency;
    let items = match collect_items(parsed, &default_currency) {
        Ok(Some(items)) => items,
        Ok(None) => {
            info(&t("Cancelled"));
            return 0;
        }
        Err(msg) => return fail(&msg),
    };
    match append_expense_items(&date, &items) {
        Ok(()) => {
            logger::success(&t("Expense added"));
            0
        }
        Err(e) => fail(&format!("Failed to save expense: {e}")),
    }
}

pub fn run_expense_list(parsed: &ParsedArgs) -> i32 {
    let all = parsed.flag("all");
    let (from, to) = match resolve_list_period(parsed) {
        Ok(period) => period,
        Err(msg) => return fail(&msg),
    };
    let days = match get_expenses(Some(&from), Some(&to), all) {
        Ok(days) => days,
        Err(e) => return fail(&format!("Failed to read expenses: {e}")),
    };

    if parsed.flag("json") {
        if days.is_empty() {
            logger::out_str("[]\n");
            return 0;
        }
        let bodies: Vec<String> = days
            .iter()
            .map(|day| {
                let items = day.items.iter().map(item_json).collect::<Vec<_>>().join(",\n");
                format!(
                    "  {{\n    \"id\": {},\n    \"date\": {},\n    \"items\": [\n{}\n    ],\n    \"totals\": [{}]\n  }}",
                    day.id,
                    quote_string(&day.date),
                    items,
                    totals_json(&day_totals(day)),
                )
            })
            .collect();
        logger::out_str(&format!("[\n{}\n]\n", bodies.join(",\n")));
        return 0;
    }

    if days.is_empty() {
        info(&t("No expenses found"));
        return 0;
    }
    let rows: Vec<Vec<String>> = days
        .iter()
        .map(|day| {
            vec![
                format!("#{}", day.id),
                day.date.clone(),
                day.items.len().to_string(),
                format_totals(&day_totals(day)),
            ]
        })
        .collect();
    render_table(
        &["#", "Date", "Items", "By currency"],
        &rows,
        vec![5, 12, 7, 24],
        vec![9, 12, 9, 60],
        &["left", "left", "right", "right"],
    );
    0
}

pub fn run_expense_total(parsed: &ParsedArgs) -> i32 {
    let (from, to) = match resolve_total_period(parsed) {
        Ok(period) => period,
        Err(msg) => return fail(&msg),
    };
    let group = parsed
        .str("group")
        .unwrap_or_else(|| "category".to_string());
    if !GROUPS.contains(&group.as_str()) {
        return fail(&format!(
            "Invalid --group: {group} (choose from {})",
            GROUPS.join(", ")
        ));
    }
    let target = match parsed.str("currency") {
        Some(raw) => match normalize_currency(&raw) {
            Ok(v) => Some(v),
            Err(msg) => return fail(&msg),
        },
        None => None,
    };

    let mut totals = match get_expense_totals(&from, &to) {
        Ok(v) => v,
        Err(e) => return fail(&format!("Failed to read expense totals: {e}")),
    };
    let mut groups = match get_expense_groups(&from, &to, &group) {
        Ok(v) => v,
        Err(e) => return fail(&format!("Failed to read expense groups: {e}")),
    };
    if let Some(target) = &target {
        totals = match convert_totals(&totals, target) {
            Ok(v) => v,
            Err(msg) => return fail(&msg),
        };
        groups = match convert_groups(&groups, target) {
            Ok(v) => v,
            Err(msg) => return fail(&msg),
        };
    }

    if parsed.flag("json") {
        let groups_json = groups
            .iter()
            .map(|(key, currency, amount)| {
                format!(
                    "{{\"currency\": {}, \"key\": {}, \"amount\": {}}}",
                    quote_string(currency),
                    quote_string(key),
                    number(*amount)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        logger::out_str(&format!(
            "{{\n  \"period\": {{\n    \"from\": {},\n    \"to\": {}\n  }},\n  \"totals\": [{}],\n  \"groups\": [{}]\n}}\n",
            quote_string(&from),
            quote_string(&to),
            totals_json(&totals),
            groups_json,
        ));
        return 0;
    }

    logger::log(&format!("{}: {} → {}", t("Period"), from, to));
    if totals.is_empty() {
        info(&t("No expenses found"));
        return 0;
    }
    for (currency, amount) in &totals {
        logger::log(&format!(
            "{}: {}",
            t("Total"),
            crate::price::format_price(*amount, currency)
        ));
    }
    let label = match group.as_str() {
        "method" => "Method",
        "day" => "Date",
        _ => "Category",
    };
    let rows: Vec<Vec<String>> = groups
        .iter()
        .map(|(key, currency, amount)| {
            vec![
                key.clone(),
                currency.clone(),
                crate::price::format_price(*amount, currency),
            ]
        })
        .collect();
    render_table(
        &[label, "Currency", "Amount"],
        &rows,
        vec![14, 10, 16],
        vec![30, 10, 22],
        &["left", "left", "right"],
    );
    0
}

pub fn run_expense_delete(parsed: &ParsedArgs) -> i32 {
    let mut targets = parsed.str_array("target").unwrap_or_default();
    if targets.is_empty() && prompt::interactive() {
        let days = match get_expenses(None, None, true) {
            Ok(v) => v,
            Err(e) => return fail(&format!("Failed to read expenses: {e}")),
        };
        if days.is_empty() {
            info(&t("No expenses found"));
            return 0;
        }
        let choices: Vec<prompt::Choice> = days
            .iter()
            .map(|day| {
                prompt::Choice::new(
                    &format!(
                        "#{} {} — {}",
                        day.id,
                        day.date,
                        format_totals(&day_totals(day))
                    ),
                    &day.id.to_string(),
                )
            })
            .collect();
        match prompt::checkbox("select expense days to delete", &choices) {
            Some(selected) => targets = selected,
            None => return 0,
        }
        if targets.is_empty() {
            info(&t("Cancelled"));
            return 0;
        }
    }
    if targets.is_empty() {
        return fail("Specify a day ID or date (e.g. `expense delete 2026-11-23`)");
    }
    if !parsed.flag("force")
        && prompt::interactive()
        && prompt::confirm(&format!("Delete {} day(s)?", targets.len()), false) != Some(true)
    {
        info(&t("Cancelled"));
        return 0;
    }
    let mut rc = 0;
    for selector in targets {
        match delete_expense_day(&selector) {
            Ok(true) => logger::success(&t("Expense day deleted")),
            Ok(false) => {
                logger::fail(&format!("{}: {}", selector, t("Expense day not found")));
                rc = 1;
            }
            Err(e) => {
                logger::fail(&format!("Failed to delete expense: {e}"));
                rc = 1;
            }
        }
    }
    rc
}

pub fn run_expense_item() -> i32 {
    fail("Specify `expense item delete` or `expense item edit`")
}

pub fn run_expense_item_delete(parsed: &ParsedArgs) -> i32 {
    let Some(selector) = parsed.str("day") else {
        return fail("Specify a day ID or date");
    };
    let Some(raw_item) = parsed.str("item") else {
        return fail("Specify an item ID");
    };
    let item_id = match parse_item_id(&raw_item) {
        Ok(id) => id,
        Err(msg) => return fail(&msg),
    };
    match delete_expense_item(&selector, item_id) {
        Ok(true) => {
            logger::success(&t("Expense item deleted"));
            0
        }
        Ok(false) => fail(&t("Expense item not found")),
        Err(e) => fail(&format!("Failed to delete expense item: {e}")),
    }
}

pub fn run_expense_item_edit(parsed: &ParsedArgs) -> i32 {
    let Some(selector) = parsed.str("day") else {
        return fail("Specify a day ID or date");
    };
    let Some(raw_item) = parsed.str("item") else {
        return fail("Specify an item ID");
    };
    let item_id = match parse_item_id(&raw_item) {
        Ok(id) => id,
        Err(msg) => return fail(&msg),
    };
    let (_, current) = match get_expense_item(&selector, item_id) {
        Ok(Some(found)) => found,
        Ok(None) => return fail(&t("Expense item not found")),
        Err(e) => return fail(&format!("Failed to read expense item: {e}")),
    };

    let mut updated = current.clone();
    if let Some(raw) = parsed.str("amount") {
        updated.amount = match parse_amount(&raw) {
            Ok(v) => v,
            Err(msg) => return fail(&msg),
        };
    }
    if let Some(raw) = parsed.str("currency") {
        updated.currency = match normalize_currency(&raw) {
            Ok(v) => v,
            Err(msg) => return fail(&msg),
        };
    }
    if let Some(raw) = parsed.str("category") {
        updated.category = match normalize_category(&raw) {
            Ok(v) => v,
            Err(msg) => return fail(&msg),
        };
    }
    if let Some(raw) = parsed.str("method") {
        updated.method = match normalize_method(&raw) {
            Ok(v) => v,
            Err(msg) => return fail(&msg),
        };
    }
    if parsed.flag("clear-note") {
        updated.note = None;
    } else if let Some(raw) = parsed.str("note") {
        updated.note = normalize_note(&raw);
    }
    match update_expense_item(&selector, item_id, &updated) {
        Ok(true) => {
            logger::success(&t("Expense item updated"));
            0
        }
        Ok(false) => fail(&t("Expense item not found")),
        Err(e) => fail(&format!("Failed to update expense item: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amount_and_currency_validation() {
        assert_eq!(parse_amount(" 14.99 ").unwrap(), 14.99);
        assert!(parse_amount("0").is_err());
        assert!(parse_amount("100000000").is_err());
        assert_eq!(normalize_currency(" jpy ").unwrap(), "JPY");
        assert!(normalize_currency("JP").is_err());
    }

    #[test]
    fn month_period_uses_calendar_month_length() {
        assert_eq!(
            parse_month("2026-02").unwrap(),
            ("2026-02-01".to_string(), "2026-02-28".to_string())
        );
        assert_eq!(
            parse_month("2024-02").unwrap(),
            ("2024-02-01".to_string(), "2024-02-29".to_string())
        );
    }
}
