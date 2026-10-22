//! `optimize` command — port of `commands/report.ts` (optimizeCommand)
//! + `optimize.ts` (handleOptimize and analyzers).

use crate::ansi;
use crate::date::{period_factor, today};
use crate::db::{
    PriceChange, QueryOptions, Subscription, get_all_price_changes, get_subscriptions,
};
use crate::fx::{fetch_rates, try_convert_subs};
use crate::json::{number, quote_string};
use crate::logger::{self, info, log, out_str, warn};
use crate::parser::ParsedArgs;
use crate::price::{format_price, round_currency};

#[derive(Clone)]
struct CycleSuggestion {
    name: String,
    currency: String,
    current_monthly: f64,
    suggested_monthly: f64,
    yearly_savings: f64,
}

struct DuplicateSuggestion {
    names: Vec<String>,
    category: String,
    by_currency: Vec<(String, f64)>,
}

struct InactiveSuggestion {
    name: String,
    id: i64,
    price: f64,
    currency: String,
    cycle: String,
    monthly: f64,
    last_changed: String,
}

struct CancelledSuggestion {
    name: String,
    price: f64,
    currency: String,
    cycle: String,
    monthly: f64,
}

/// Port of `tokenizeName` — lowercase, strip parens, split on whitespace/-,
/// drop tokens ≤2 chars and common plan words.
fn tokenize_name(name: &str) -> Vec<String> {
    const COMMON: [&str; 12] = [
        "plan",
        "pro",
        "basic",
        "premium",
        "standard",
        "plus",
        "family",
        "duo",
        "student",
        "enterprise",
        "business",
        "personal",
    ];
    let cleaned: String = name
        .to_lowercase()
        .chars()
        .filter(|c| *c != '(' && *c != ')')
        .collect();
    cleaned
        .split([' ', '-'])
        .filter(|t| t.chars().count() > 2 && !COMMON.contains(t))
        .map(|t| t.to_string())
        .collect()
}

fn jaccard_similarity(a: &[String], b: &[String]) -> f64 {
    let inter = a.iter().filter(|t| b.contains(t)).count();
    let mut union = a.to_vec();
    for t in b {
        if !union.contains(t) {
            union.push(t.clone())
        }
    }
    if union.is_empty() {
        return 0.0;
    }
    inter as f64 / union.len() as f64
}

fn analyze_cycle_optimization(subs: &[Subscription], discount_rate: f64) -> Vec<CycleSuggestion> {
    subs.iter()
        .filter(|s| crate::domain::not_cancelled_or_paused(&s.status) && s.cycle == "monthly")
        .filter_map(|s| {
            let monthly_cost = s.price;
            let yearly_cost = monthly_cost * 12.0;
            let estimated = yearly_cost * (1.0 - discount_rate / 100.0);
            let savings = yearly_cost - estimated;
            (savings > 0.0).then(|| CycleSuggestion {
                name: s.name.clone(),
                currency: s.currency.clone(),
                current_monthly: monthly_cost,
                suggested_monthly: round_currency(estimated / 12.0),
                yearly_savings: round_currency(savings),
            })
        })
        .collect()
}

fn analyze_duplicates(subs: &[Subscription]) -> Vec<DuplicateSuggestion> {
    let active: Vec<&Subscription> = subs
        .iter()
        .filter(|s| crate::domain::is_active(&s.status))
        .collect();
    let mut results = Vec::new();
    let mut checked: std::collections::HashSet<i64> = std::collections::HashSet::new();

    for (i, a) in active.iter().enumerate() {
        if checked.contains(&a.id) {
            continue;
        }
        let tokens_a = tokenize_name(&a.name);
        let mut similar: Vec<&Subscription> = Vec::new();
        for b in active.iter().skip(i + 1) {
            if checked.contains(&b.id) {
                continue;
            }
            let tokens_b = tokenize_name(&b.name);
            if jaccard_similarity(&tokens_a, &tokens_b) >= 0.5 {
                similar.push(b);
            }
        }
        if !similar.is_empty() {
            let mut all_similar = vec![*a];
            all_similar.extend(similar);
            for s in &all_similar {
                checked.insert(s.id);
            }
            let mut by_currency: Vec<(String, f64)> = Vec::new();
            for s in &all_similar {
                add_amount(
                    &mut by_currency,
                    &s.currency,
                    s.price * period_factor(&s.cycle, "monthly"),
                );
            }
            by_currency.sort_by(|a, b| a.0.cmp(&b.0));
            let mut first_tags: Vec<String> = all_similar
                .iter()
                .filter_map(|s| s.tags.first().cloned())
                .collect();
            first_tags.sort();
            let mut first_tokens: Vec<String> = all_similar
                .iter()
                .filter_map(|s| tokenize_name(&s.name).first().cloned())
                .collect();
            first_tokens.sort();
            let category = first_tags
                .first()
                .cloned()
                .or_else(|| first_tokens.first().cloned())
                .unwrap_or_else(|| "unknown".to_string());
            results.push(DuplicateSuggestion {
                names: all_similar.iter().map(|s| s.name.clone()).collect(),
                category,
                by_currency: by_currency
                    .into_iter()
                    .map(|(c, amount)| (c, round_currency(amount)))
                    .collect(),
            });
        }
    }
    results
}

/// Port of `analyzeInactive`: active subs with no price change for >= 18 months.
fn analyze_inactive(subs: &[Subscription], all_changes: &[PriceChange]) -> Vec<InactiveSuggestion> {
    let active: Vec<&Subscription> = subs
        .iter()
        .filter(|s| crate::domain::is_active(&s.status))
        .collect();
    let mut last_by_sub: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
    for ch in all_changes {
        let newer = match last_by_sub.get(&ch.subscription_id) {
            Some(existing) => ch.changed_at > *existing,
            None => true,
        };
        if newer {
            last_by_sub.insert(ch.subscription_id, ch.changed_at.clone());
        }
    }
    let mut results = Vec::new();
    for sub in active {
        if let Some(last) = last_by_sub.get(&sub.id) {
            let date_part = last.split(' ').next().unwrap_or("");
            if let Ok(d) = chrono::NaiveDate::parse_from_str(date_part, "%Y-%m-%d") {
                let days = (today() - d).num_days();
                let months_since = days as f64 / 30.0;
                if months_since >= 18.0 {
                    results.push(InactiveSuggestion {
                        name: sub.name.clone(),
                        id: sub.id,
                        price: sub.price,
                        currency: sub.currency.clone(),
                        cycle: sub.cycle.clone(),
                        monthly: round_currency(sub.price * period_factor(&sub.cycle, "monthly")),
                        last_changed: date_part.to_string(),
                    });
                }
            }
        }
    }
    results
}

fn analyze_cancelled(subs: &[Subscription]) -> Vec<CancelledSuggestion> {
    subs.iter()
        .filter(|s| {
            matches!(
                crate::domain::Status::parse(&s.status),
                Ok(crate::domain::Status::Cancelled) | Ok(crate::domain::Status::Paused)
            )
        })
        .map(|s| CancelledSuggestion {
            name: s.name.clone(),
            price: s.price,
            currency: s.currency.clone(),
            cycle: s.cycle.clone(),
            monthly: round_currency(s.price * period_factor(&s.cycle, "monthly")),
        })
        .collect()
}

pub fn run_optimize(parsed: &ParsedArgs) -> i32 {
    let min_savings = match parsed.str("min-savings") {
        Some(v) => match v.trim().parse::<f64>() {
            Ok(n) if n >= 0.0 => n,
            _ => {
                logger::fail("min-savings must be a non-negative number");
                return 1;
            }
        },
        None => 0.0,
    };
    let discount_rate = match parsed.str("discount-rate") {
        Some(v) => match v.trim().parse::<f64>() {
            Ok(n) if (0.0..=100.0).contains(&n) => n,
            _ => {
                logger::fail("discount-rate must be a number between 0 and 100");
                return 1;
            }
        },
        None => 15.0,
    };
    let exclude: Vec<String> = parsed
        .str("exclude")
        .map(|s| {
            s.split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let mut subs = match get_subscriptions(&QueryOptions::default()) {
        Ok(v) => v,
        Err(e) => {
            logger::fail(&format!("Failed to load subscriptions: {}", e));
            return 1;
        }
    };
    if subs.is_empty() {
        info("No subscriptions found — try `subtrack add`");
        return 0;
    }
    if !exclude.is_empty() {
        subs.retain(|s| !exclude.contains(&s.name));
    }

    let mut display_currency: Option<String> = None;
    if let Some(target) = parsed
        .str("currency")
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
    {
        let rates = fetch_rates();
        let converted = try_convert_subs(&subs, &target, &rates);
        if converted.has_missing {
            warn(&format!(
                "Some prices could not be converted to {}; they are excluded from the report",
                target
            ));
        }
        subs = converted
            .list
            .into_iter()
            .filter(|s| s.currency == target)
            .collect();
        display_currency = Some(target);
    }

    let all_changes = match get_all_price_changes() {
        Ok(v) => v,
        Err(e) => {
            logger::fail(&format!("Failed to read price history: {}", e));
            return 1;
        }
    };
    let cycle_suggestions = analyze_cycle_optimization(&subs, discount_rate);
    let duplicate_suggestions = analyze_duplicates(&subs);
    let inactive_suggestions = analyze_inactive(&subs, &all_changes);
    let cancelled_suggestions = analyze_cancelled(&subs);

    let mut total_yearly_savings: Vec<(String, f64)> = Vec::new();
    for s in &cycle_suggestions {
        add_amount(&mut total_yearly_savings, &s.currency, s.yearly_savings);
    }
    total_yearly_savings.sort_by(|a, b| a.0.cmp(&b.0));
    for (_, total) in &mut total_yearly_savings {
        *total = round_currency(*total);
    }

    if min_savings > 0.0 {
        let (currency, total) = match (&display_currency, total_yearly_savings.as_slice()) {
            (Some(c), _) => (
                c.clone(),
                total_yearly_savings
                    .iter()
                    .find(|(ccy, _)| ccy == c)
                    .map(|(_, total)| *total)
                    .unwrap_or(0.0),
            ),
            (None, [(ccy, total)]) => (ccy.clone(), *total),
            (None, _) => {
                logger::fail(
                    "min-savings requires --currency when subscriptions use multiple currencies",
                );
                return 1;
            }
        };
        if total < min_savings {
            info(&format!(
                "Potential savings ({}/yr) below minimum threshold ({}/yr)",
                format_price(total, &currency),
                format_price(min_savings, &currency)
            ));
            return 0;
        }
    }

    if parsed.flag("json") {
        let mut items: Vec<String> = Vec::new();
        for s in &cycle_suggestions {
            items.push(format!(
                "    {{\n      \"type\": \"cycle\",\n      \"name\": {},\n      \"currency\": {},\n      \"currentCycle\": \"monthly\",\n      \"suggestedCycle\": \"yearly\",\n      \"currentMonthly\": {},\n      \"suggestedMonthly\": {},\n      \"yearlySavings\": {}\n    }}",
                quote_string(&s.name),
                quote_string(&s.currency),
                number(s.current_monthly),
                number(s.suggested_monthly),
                number(s.yearly_savings)
            ));
        }
        for d in &duplicate_suggestions {
            let names: Vec<String> = d.names.iter().map(|n| quote_string(n)).collect();
            let total_monthly = if d.by_currency.len() == 1 {
                number(d.by_currency[0].1)
            } else {
                "null".to_string()
            };
            let by_currency: Vec<String> = d
                .by_currency
                .iter()
                .map(|(ccy, amount)| format!("        {}: {}", quote_string(ccy), number(*amount)))
                .collect();
            items.push(format!(
                "    {{\n      \"type\": \"duplicate\",\n      \"names\": [\n        {}\n      ],\n      \"category\": {},\n      \"totalMonthly\": {},\n      \"byCurrency\": {{\n{}\n      }}\n    }}",
                names.join(",\n        "),
                quote_string(&d.category),
                total_monthly,
                by_currency.join(",\n")
            ));
        }
        for s in &inactive_suggestions {
            items.push(format!(
                "    {{\n      \"type\": \"inactive\",\n      \"name\": {},\n      \"id\": {},\n      \"price\": {},\n      \"currency\": {},\n      \"cycle\": {},\n      \"monthly\": {},\n      \"lastChanged\": {}\n    }}",
                quote_string(&s.name),
                s.id,
                number(s.price),
                quote_string(&s.currency),
                quote_string(&s.cycle),
                number(s.monthly),
                quote_string(&s.last_changed)
            ));
        }
        for c in &cancelled_suggestions {
            items.push(format!(
                "    {{\n      \"type\": \"cancelled\",\n      \"name\": {},\n      \"price\": {},\n      \"currency\": {},\n      \"cycle\": {},\n      \"monthly\": {}\n    }}",
                quote_string(&c.name),
                number(c.price),
                quote_string(&c.currency),
                quote_string(&c.cycle),
                number(c.monthly)
            ));
        }
        let total_json = if total_yearly_savings.len() == 1 {
            number(total_yearly_savings[0].1)
        } else {
            "null".to_string()
        };
        let total_by_currency: Vec<String> = total_yearly_savings
            .iter()
            .map(|(ccy, total)| format!("    {}: {}", quote_string(ccy), number(*total)))
            .collect();
        out_str(&format!(
            "{{\n  \"suggestions\": [\n{}\n  ],\n  \"totalYearlySavings\": {},\n  \"totalYearlySavingsByCurrency\": {{\n{}\n  }}\n}}\n",
            items.join(",\n"),
            total_json,
            total_by_currency.join(",\n")
        ));
        return 0;
    }

    render_report(
        &cycle_suggestions,
        &duplicate_suggestions,
        &inactive_suggestions,
        &cancelled_suggestions,
        &total_yearly_savings,
        display_currency.as_deref(),
    );
    0
}

fn add_amount(totals: &mut Vec<(String, f64)>, currency: &str, amount: f64) {
    match totals.iter_mut().find(|(c, _)| c == currency) {
        Some((_, total)) => *total += amount,
        None => totals.push((currency.to_string(), amount)),
    }
}

fn format_totals(totals: &[(String, f64)], suffix: &str) -> String {
    let mut totals = totals.to_vec();
    totals.sort_by(|a, b| a.0.cmp(&b.0));
    totals
        .iter()
        .map(|(currency, amount)| {
            format!(
                "{}{}",
                format_price(round_currency(*amount), currency),
                suffix
            )
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

fn render_report(
    cycle: &[CycleSuggestion],
    duplicates: &[DuplicateSuggestion],
    inactive: &[InactiveSuggestion],
    cancelled: &[CancelledSuggestion],
    total_yearly_savings: &[(String, f64)],
    display_currency: Option<&str>,
) {
    let mut sections: Vec<String> = vec![ansi::bold("Cost Optimization Report")];

    // Cycle Optimization
    if !cycle.is_empty() {
        let mut lines: Vec<String> = vec![
            String::new(),
            ansi::bold("  Cycle Optimization"),
            String::new(),
        ];
        let mut total_savings: Vec<(String, f64)> = Vec::new();
        for s in cycle {
            lines.push(format!(
                "    {}  {}{}  {}",
                s.name,
                ansi::dim(&format!(
                    "{}/mo → ",
                    format_price(s.current_monthly, &s.currency)
                )),
                ansi::green(&format!(
                    "{}/mo",
                    format_price(s.suggested_monthly, &s.currency)
                )),
                ansi::green(&format!(
                    "(save {}/yr)",
                    format_price(s.yearly_savings, &s.currency)
                ))
            ));
            add_amount(&mut total_savings, &s.currency, s.yearly_savings);
        }
        if !total_savings.is_empty() {
            lines.push(format!("    {}", ansi::dim(&"─".repeat(40))));
            lines.push(format!(
                "    {}",
                ansi::green(&ansi::bold(&format!(
                    "Total savings: {}",
                    format_totals(&total_savings, "/yr")
                )))
            ));
        }
        sections.push(lines.join("\n"));
    }

    // Possible Duplicates
    if !duplicates.is_empty() {
        let mut lines: Vec<String> = vec![
            String::new(),
            ansi::bold("  Possible Duplicates"),
            String::new(),
        ];
        for d in duplicates {
            lines.push(format!("    {}:", ansi::yellow(&d.category)));
            for name in &d.names {
                lines.push(format!("      • {}", name));
            }
            lines.push(format!(
                "      {}",
                ansi::dim(&format!(
                    "Combined: {}",
                    format_totals(&d.by_currency, "/mo")
                ))
            ));
            lines.push(String::new());
        }
        sections.push(lines.join("\n"));
    }

    // Inactive (no price change >18 months)
    if !inactive.is_empty() {
        let mut lines: Vec<String> = vec![
            String::new(),
            ansi::bold("  Inactive (no price change >18 months)"),
            String::new(),
        ];
        for s in inactive {
            let id = ansi::dim(&format!("#{}", s.id));
            let monthly = format_price(s.monthly, &s.currency);
            let last = ansi::dim(&format!("last change: {}", s.last_changed));
            lines.push(format!("    {} {}  {}/mo  {}", id, s.name, monthly, last));
        }
        sections.push(lines.join("\n"));
    }

    // Cancelled / Paused
    if !cancelled.is_empty() {
        let mut total_monthly: Vec<(String, f64)> = Vec::new();
        for c in cancelled {
            add_amount(&mut total_monthly, &c.currency, c.monthly);
        }
        let mut lines: Vec<String> = vec![
            String::new(),
            ansi::bold("  Cancelled / Paused (previously paid)"),
            String::new(),
        ];
        for c in cancelled {
            lines.push(format!(
                "    • {}  {}",
                c.name,
                ansi::dim(&format!("{}/mo", format_price(c.monthly, &c.currency)))
            ));
        }
        lines.push(format!("    {}", ansi::dim(&"─".repeat(40))));
        lines.push(format!(
            "    {}",
            ansi::dim(&format!(
                "Previously: {}",
                format_totals(&total_monthly, "/mo")
            ))
        ));
        sections.push(lines.join("\n"));
    }

    if !total_yearly_savings.is_empty() {
        if let Some(currency) = display_currency {
            let total = total_yearly_savings
                .iter()
                .find(|(c, _)| c == currency)
                .map(|(_, total)| *total)
                .unwrap_or(0.0);
            sections.push(format!(
                "\n{}",
                ansi::bold(&format!(
                    "  Total potential savings: {}/yr",
                    ansi::green(&format_price(total, currency))
                ))
            ));
        } else if total_yearly_savings.len() == 1 {
            let (currency, total) = &total_yearly_savings[0];
            sections.push(format!(
                "\n{}",
                ansi::bold(&format!(
                    "  Total potential savings: {}/yr",
                    ansi::green(&format_price(*total, currency))
                ))
            ));
        } else {
            let rendered = total_yearly_savings
                .iter()
                .map(|(currency, total)| format_price(*total, currency))
                .collect::<Vec<_>>()
                .join(" + ");
            sections.push(format!(
                "\n{}",
                ansi::bold(&format!(
                    "  Total potential savings: {}/yr",
                    ansi::green(&rendered)
                ))
            ));
        }
    }

    log(&sections.join("\n"));
}
