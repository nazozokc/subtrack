//! Display layer — port of `packages/subtrack/src/display.ts` +
//! the showApiUsage summary table.

use crate::ansi;
use crate::config;
use crate::date::{calculate_next_billing, days_until, format_cycle, format_short_date, today};
use crate::db::Subscription;
use crate::logger;
use crate::price::{format_price, format_usd_cost, round_currency};
use crate::table::{
    ColumnConfig, TableBuilder, calc_column_widths, section_title, status_color, strlen,
    table_style, zebra_row,
};
use crate::ui;

fn next_billing_cell(sub: &Subscription) -> String {
    if !crate::domain::listed(&sub.status) {
        return "-".to_string();
    }
    let now = today();
    let next = calculate_next_billing(sub.billing_day, &sub.created_at, &sub.cycle, now);
    let days = days_until(next);
    if days <= 0 {
        return ansi::bold(&ansi::cyan("today"));
    }
    let label = format_short_date(next);
    if days <= 3 {
        return ansi::red(&label);
    }
    if days <= 7 {
        return ansi::yellow(&label);
    }
    ansi::dim(&label)
}

fn build_row(
    sub: &Subscription,
    price: &str,
    show_notes: bool,
    show_method: bool,
    show_contract: bool,
    show_vendor: bool,
) -> Vec<String> {
    let theme = config::display_theme();
    let tags = if sub.tags.is_empty() {
        "-".to_string()
    } else {
        sub.tags.join(", ")
    };
    let mut row = vec![
        sub.name.clone(),
        status_color(&sub.status, &theme),
        format_cycle(&sub.cycle),
        next_billing_cell(sub),
        tags,
        price.to_string(),
    ];

    let mut insert_idx = row.len() - 1;

    if show_method {
        let method = sub.payment_method.clone().unwrap_or_default();
        let cell = if method.chars().count() > 20 {
            let trimmed: String = method.chars().take(17).collect();
            format!("{}...", trimmed)
        } else {
            method
        };
        row.insert(insert_idx, cell);
        insert_idx += 1
    }

    if show_contract {
        let period = match (&sub.contract_start, &sub.contract_end) {
            (Some(start), Some(end)) => format!("{} ~ {}", start, end),
            (Some(start), None) => format!("{} ~ ongoing", start),
            (None, _) => "-".to_string(),
        };
        let period_cell = if period.chars().count() > 16 {
            let trimmed: String = period.chars().take(13).collect();
            format!("{}...", trimmed)
        } else {
            period
        };
        let renewal = if sub.auto_renewal { "auto" } else { "manual" }.to_string();
        row.insert(insert_idx, period_cell);
        insert_idx += 1;
        row.insert(insert_idx, renewal);
        insert_idx += 1
    }

    if show_vendor {
        let vendor = sub.vendor_name.as_deref().unwrap_or("-").to_string();
        let tier = sub.plan_tier.as_deref().unwrap_or("-").to_string();
        let vendor_cell = if vendor.chars().count() > 15 {
            let trimmed: String = vendor.chars().take(12).collect();
            format!("{}...", trimmed)
        } else {
            vendor
        };
        let tier_cell = if tier.chars().count() > 12 {
            let trimmed: String = tier.chars().take(9).collect();
            format!("{}...", trimmed)
        } else {
            tier
        };
        row.insert(insert_idx, vendor_cell);
        insert_idx += 1;
        row.insert(insert_idx, tier_cell)
    }

    if show_notes {
        let insert_at = row.len() - 1;
        let notes = sub.notes.clone().unwrap_or_default();
        let notes_cell = if notes.chars().count() > 40 {
            let trimmed: String = notes.chars().take(37).collect();
            format!("{}...", trimmed)
        } else {
            notes
        };
        row.insert(insert_at, notes_cell)
    }

    row
}

fn config_headers(headers: &[&str]) -> Vec<String> {
    headers.iter().map(|h| h.to_string()).collect()
}

fn pick_config(
    show_notes: bool,
    show_method: bool,
    show_contract: bool,
    show_vendor: bool,
) -> (Vec<String>, Vec<i64>, Vec<i64>) {
    if show_notes && show_method && show_contract && show_vendor {
        (
            config_headers(&[
                "name", "status", "cycle", "next", "tags", "method", "contract", "renewal",
                "vendor", "plan", "notes", "price",
            ]),
            vec![8, 8, 9, 8, 8, 10, 18, 6, 10, 8, 15, 8],
            vec![30, 12, 20, 10, 50, 30, 30, 10, 30, 20, 50, 20],
        )
    } else if show_contract && show_vendor {
        (
            config_headers(&[
                "name", "status", "cycle", "next", "tags", "contract", "renewal", "vendor", "plan",
                "price",
            ]),
            vec![10, 8, 9, 8, 8, 18, 6, 10, 8, 8],
            vec![40, 12, 20, 10, 60, 30, 10, 30, 20, 20],
        )
    } else if show_contract {
        (
            config_headers(&[
                "name", "status", "cycle", "next", "tags", "contract", "renewal", "price",
            ]),
            vec![10, 8, 9, 8, 8, 18, 6, 8],
            vec![40, 12, 20, 10, 60, 30, 10, 20],
        )
    } else if show_vendor {
        (
            config_headers(&[
                "name", "status", "cycle", "next", "tags", "vendor", "plan", "price",
            ]),
            vec![10, 8, 9, 8, 8, 10, 8, 8],
            vec![40, 12, 20, 10, 60, 30, 20, 20],
        )
    } else if show_notes && show_method {
        (
            config_headers(&[
                "name", "status", "cycle", "next", "tags", "method", "notes", "price",
            ]),
            vec![10, 8, 9, 8, 8, 10, 15, 8],
            vec![40, 12, 20, 10, 60, 30, 50, 20],
        )
    } else if show_notes {
        (
            config_headers(&["name", "status", "cycle", "next", "tags", "notes", "price"]),
            vec![10, 8, 9, 8, 8, 15, 8],
            vec![40, 12, 20, 10, 60, 50, 20],
        )
    } else if show_method {
        (
            config_headers(&["name", "status", "cycle", "next", "tags", "method", "price"]),
            vec![10, 8, 9, 8, 8, 10, 8],
            vec![40, 12, 20, 10, 60, 30, 20],
        )
    } else {
        (
            config_headers(&["name", "status", "cycle", "next", "tags", "price"]),
            vec![10, 8, 9, 8, 10, 8],
            vec![40, 12, 20, 10, 60, 20],
        )
    }
}

pub fn render_table(
    rows: &[Vec<String>],
    headers: &[String],
    min_widths: &[i64],
    max_widths: &[i64],
    term_width: i64,
    table_min_width: i64,
) -> String {
    let config = ColumnConfig {
        headers: headers.to_vec(),
        min_widths: min_widths.to_vec(),
        max_widths: max_widths.to_vec(),
        min_avail: None,
    };
    let widths = calc_column_widths(rows, &config, term_width, table_min_width);
    let col_aligns: Vec<String> = headers
        .iter()
        .enumerate()
        .map(|(i, _)| {
            if i == headers.len() - 1 {
                "right".to_string()
            } else {
                "left".to_string()
            }
        })
        .collect();

    let theme = config::display_theme();
    let mut table = TableBuilder::new(table_style(&theme));
    table.col_widths = widths.clone();
    table.head = headers.to_vec();
    table.word_wrap = true;
    table.wrap_on_word_boundary = true;
    table.col_aligns = col_aligns.clone();

    for (i, row) in rows.iter().enumerate() {
        if i % 2 == 0 {
            table.push(zebra_row(row, theme.zebra))
        } else {
            table.push(row.clone())
        }
    }

    table.to_string()
}

/// Print a right-aligned total line directly below a rendered table.
fn log_table_total(table: &str, label: &str, value: &str) {
    let table_width = table.lines().next().map(strlen).unwrap_or(0);
    let plain = format!("{}  {}", label, value);
    let pad = (table_width - strlen(&plain)).max(0) as usize;
    logger::log(&format!(
        "{}{}  {}",
        " ".repeat(pad),
        ansi::bold(&ansi::yellow(label)),
        ansi::bold(&ansi::yellow(value))
    ));
}

/// Port of spreadSubscription — renders per-currency tables.
pub fn spread_subscription(
    list: &[Subscription],
    currency: Option<&str>,
    show_notes: bool,
    show_method: bool,
    show_contract: bool,
    show_vendor: bool,
) {
    if list.is_empty() {
        logger::info("No subscriptions found — try `subtrack add`");
        return;
    }

    let (headers, min_widths, max_widths) =
        pick_config(show_notes, show_method, show_contract, show_vendor);
    let table_min_width = config::load_config().table_min_width;

    if let Some(cu) = currency {
        let mut total = 0.0_f64;
        let mut rows: Vec<Vec<String>> = Vec::new();
        for sub in list {
            if sub.currency == cu {
                total += sub.price;
                rows.push(build_row(
                    sub,
                    &format_price(sub.price, cu),
                    show_notes,
                    show_method,
                    show_contract,
                    show_vendor,
                ))
            } else {
                rows.push(build_row(
                    sub,
                    &format!("? ({})", format_price(sub.price, &sub.currency)),
                    show_notes,
                    show_method,
                    show_contract,
                    show_vendor,
                ))
            }
        }
        let table = render_table(
            &rows,
            &headers,
            &min_widths,
            &max_widths,
            ui::term_width(),
            table_min_width,
        );
        logger::log(&table);
        log_table_total(
            &table,
            &format!("{} TOTAL", cu),
            &format_price(round_currency(total), cu),
        );
        return;
    }

    // Display subscriptions grouped by currency (insertion order).
    let mut groups: Vec<(String, Vec<Subscription>)> = Vec::new();
    for sub in list {
        if let Some(g) = groups.iter_mut().find(|(c, _)| *c == sub.currency) {
            g.1.push(sub.clone())
        } else {
            groups.push((sub.currency.clone(), vec![sub.clone()]))
        }
    }

    for (gi, (currency_code, currency_subs)) in groups.iter().enumerate() {
        let mut group_rows: Vec<Vec<String>> = Vec::new();
        let mut total = 0.0_f64;
        for sub in currency_subs {
            group_rows.push(build_row(
                sub,
                &format_price(sub.price, &sub.currency),
                show_notes,
                show_method,
                show_contract,
                show_vendor,
            ));
            total += sub.price
        }
        let table = render_table(
            &group_rows,
            &headers,
            &min_widths,
            &max_widths,
            ui::term_width(),
            table_min_width,
        );
        logger::log(&table);
        log_table_total(
            &table,
            &format!("{} TOTAL", currency_code),
            &format_price(total, currency_code),
        );
        if gi < groups.len() - 1 {
            logger::log("")
        }
    }
}

/// Port of showApiUsage — the `── API Usage (…) ──` summary table.
pub fn show_api_usage(total: i64, by_provider: &[(String, i64)], period_label: &str) {
    logger::log("");
    logger::log(&section_title(
        &format!("API Usage ({})", period_label),
        &config::display_theme(),
    ));

    if total <= 0 {
        logger::info("No API usage found for this month");
        return;
    }

    let theme = config::display_theme();
    let mut table = TableBuilder::new(table_style(&theme));
    table.head = config_headers(&["Provider", "Cost"]);
    table.col_aligns = vec!["left".to_string(), "right".to_string()];

    for (provider, cost) in by_provider {
        table.push(vec![provider.clone(), format_usd_cost(*cost, 2)])
    }
    table.push(vec![
        ansi::bold(&ansi::yellow("Total")),
        ansi::bold(&ansi::yellow(&format_usd_cost(total, 2))),
    ]);

    logger::log(&table.to_string())
}
