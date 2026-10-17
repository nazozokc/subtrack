//! `trial` commands — port of `commands/trial.ts` (list/expiring)
//! + the list rendering path of `trial.ts`.

use crate::ansi;
use crate::config::display_theme;
use crate::date::{days_until, today};
use crate::db::{get_trials, get_trials_expiring_soon, Trial};
use crate::logger::{self, info, log};
use crate::parser::ParsedArgs;
use crate::price::format_price;
use crate::table::{calc_column_widths, table_style, zebra_row, ColumnConfig};

/// `subtrack trial list` — all trials, any expiry.
pub fn run_trial_list() -> i32 {
    let trials = match get_trials() {
        Ok(v) => v,
        Err(e) => {
            logger::fail(&format!("Failed to read trials: {}", e));
            return 1;
        }
    };
    if trials.is_empty() {
        info("No trials found");
        return 0;
    }
    render_trial_table(&trials);
    0
}

/// `subtrack trial expiring [days]` — expiring within N days.
pub fn run_trial_expiring(parsed: &ParsedArgs) -> i32 {
    let days: i64 = match parsed.str("days") {
        Some(v) => match v.trim().parse::<i64>() {
            Ok(n) if n >= 0 => n,
            _ => {
                logger::fail(&format!("Invalid days: {}", v));
                return 1;
            }
        },
        None => 7,
    };
    let trials = match get_trials_expiring_soon(days) {
        Ok(v) => v,
        Err(e) => {
            logger::fail(&format!("Failed to read trials: {}", e));
            return 1;
        }
    };
    if trials.is_empty() {
        info(&format!(
            "No trials expiring within {} day{}",
            days,
            if days > 1 { "s" } else { "" }
        ));
        return 0;
    }
    info(&format!(
        "Trials expiring within {} day{}:",
        days,
        if days > 1 { "s" } else { "" }
    ));
    render_trial_table(&trials);
    0
}

/// `subtrack trial` — usage hint (port of `trialCommand.run`).
pub fn run_trial_usage() -> i32 {
    info("Usage: subtrack trial add|list|expiring|delete");
    0
}

/// Port of `handleTrialAdd` (validation + persistence).
pub fn run_trial_add(parsed: &ParsedArgs) -> i32 {
    use crate::prompts::{prompt_cycle, prompt_select, prompt_string};
    use crate::validation::{
        is_valid_currency, js_number, validate_expires_at, validate_notes, validate_trial_name,
        CURRENCY_VALUES,
    };

    let Some(name) = prompt_string(parsed.str("name").as_deref(), validate_trial_name) else {
        return 0;
    };
    let Some(expires_at) =
        prompt_string(parsed.str("expiresAt").as_deref(), validate_expires_at)
    else {
        return 0;
    };

    let price = match parsed.str("price") {
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                None
            } else {
                match js_number(trimmed) {
                    Some(n) if n.is_finite() && n >= 0.0 => Some(n),
                    _ => {
                        logger::fail("Price must be a non-negative number");
                        return 1;
                    }
                }
            }
        }
        None => None,
    };

    let Some(currency) =
        prompt_select(parsed.str("currency").as_deref(), is_valid_currency, &CURRENCY_VALUES)
    else {
        return 0;
    };
    let Some(cycle) = prompt_cycle(parsed.str("cycle").as_deref()) else {
        return 0;
    };

    let notes = match parsed.str("notes") {
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                None
            } else {
                if let Err(e) = validate_notes(trimmed) {
                    logger::fail(&e);
                    return 1;
                }
                Some(trimmed.to_string())
            }
        }
        None => None,
    };

    let conn = match crate::db::open_db() {
        Ok(c) => c,
        Err(e) => {
            logger::fail(&format!("Failed to add trial: {}", e));
            return 1;
        }
    };
    if let Err(e) = crate::db::insert_trial(
        &conn,
        &name,
        &expires_at,
        price,
        Some(&currency),
        Some(&cycle),
        notes.as_deref(),
        &crate::date::today_string(),
        true,
    ) {
        logger::fail(&format!("Failed to add trial: {}", e));
        return 1;
    }

    logger::success(&format!("Added trial: {}", name));
    0
}

/// Port of `handleTrialDelete`.
pub fn run_trial_delete(parsed: &ParsedArgs) -> i32 {
    let ids: Vec<i64> = parsed
        .str_array("id")
        .unwrap_or_default()
        .iter()
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .collect();
    if ids.is_empty() {
        return 0;
    }
    let mut rc = 0;
    for id in ids {
        match crate::db::get_trial(id) {
            Ok(Some(trial)) => {
                let _ = crate::db::remove_trial(id);
                logger::success(&format!("Deleted trial: {}", trial.name));
            }
            Ok(None) => {
                logger::fail(&format!("Trial with id {} not found", id));
                rc = 1;
            }
            Err(e) => {
                logger::fail(&format!("Failed to read trial: {}", e));
                rc = 1;
            }
        }
    }
    rc
}


fn format_price_or_dash(t: &Trial) -> String {
    match t.price {
        Some(p) => {
            let ccy = t.currency.as_deref().unwrap_or("USD");
            let cyc = t.cycle.as_deref().unwrap_or("monthly");
            format!("{}/{}", format_price(p, ccy), cyc)
        }
        None => "—".to_string(),
    }
}

/// Port of `trialStatusLabel`: red(expired/today/≤3d), yellow(≤7d), green(rest).
fn trial_status_label(days: i64) -> String {
    let body = if days < 0 {
        "expired".to_string()
    } else if days == 0 {
        "today".to_string()
    } else if days <= 3 {
        format!("{}d", days)
    } else if days <= 7 {
        format!("{}d", days)
    } else {
        format!("{}d", days)
    };
    if days < 0 || days == 0 || days <= 3 {
        ansi::red(&body)
    } else if days <= 7 {
        ansi::yellow(&body)
    } else {
        ansi::colored("green", &body)
    }
}

/// Port of `renderTrialTable`.
pub fn render_trial_table(trials: &[Trial]) {
    let headers: Vec<String> = ["ID", "Name", "Expires", "Days", "Price", "Notes"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    let rows: Vec<Vec<String>> = trials
        .iter()
        .map(|t| {
            let days = days_until(parse_date(&t.expires_at));
            vec![
                t.id.to_string(),
                t.name.clone(),
                t.expires_at.clone(),
                trial_status_label(days),
                format_price_or_dash(t),
                t.notes.clone().unwrap_or_default(),
            ]
        })
        .collect();

    let config = ColumnConfig {
        headers: headers.clone(),
        min_widths: vec![4, 16, 10, 6, 10, 8],
        max_widths: vec![10, 40, 14, 10, 20, 40],
        min_avail: Some(50),
    };
    let widths = calc_column_widths(&rows, &config, 80, 40);

    let theme = display_theme();
    let col_aligns: Vec<String> = ["right", "left", "left", "left", "right", "left"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    let mut table = crate::table::TableBuilder::new(table_style(&theme));
    table.col_widths = widths;
    table.head = headers;
    table.col_aligns = col_aligns;
    for (i, row) in rows.iter().enumerate() {
        if i % 2 == 0 {
            table.push(zebra_row(row, theme.zebra))
        } else {
            table.push(row.clone())
        }
    }
    log(&table.to_string());
}

fn parse_date(s: &str) -> chrono::NaiveDate {
    s.parse::<chrono::NaiveDate>().unwrap_or_else(|_| today())
}