//! Pure optimization analysis. Dates and database rows are supplied by the caller.

use chrono::NaiveDate;
use subtrack_core::models::{PriceChange, Subscription};

use crate::date::period_factor;
use crate::price::round_currency;

pub(super) struct CycleSuggestion {
    pub(super) name: String,
    pub(super) currency: String,
    pub(super) current_monthly: f64,
    pub(super) suggested_monthly: f64,
    pub(super) yearly_savings: f64,
}

pub(super) struct DuplicateSuggestion {
    pub(super) names: Vec<String>,
    pub(super) category: String,
    pub(super) by_currency: Vec<(String, f64)>,
}

pub(super) struct InactiveSuggestion {
    pub(super) name: String,
    pub(super) id: i64,
    pub(super) price: f64,
    pub(super) currency: String,
    pub(super) cycle: String,
    pub(super) monthly: f64,
    pub(super) last_changed: String,
}

pub(super) struct CancelledSuggestion {
    pub(super) name: String,
    pub(super) price: f64,
    pub(super) currency: String,
    pub(super) cycle: String,
    pub(super) monthly: f64,
}

pub(super) struct Analysis {
    pub(super) cycle: Vec<CycleSuggestion>,
    pub(super) duplicates: Vec<DuplicateSuggestion>,
    pub(super) inactive: Vec<InactiveSuggestion>,
    pub(super) cancelled: Vec<CancelledSuggestion>,
    pub(super) total_yearly_savings: Vec<(String, f64)>,
    pub(super) cancelled_monthly: Vec<(String, f64)>,
}

/// Analyze the supplied subscriptions and price history as of `today`.
pub(super) fn analyze(
    subs: &[Subscription],
    all_changes: &[PriceChange],
    discount_rate: f64,
    today: NaiveDate,
) -> Analysis {
    let cycle = analyze_cycle_optimization(subs, discount_rate);
    let duplicates = analyze_duplicates(subs);
    let inactive = analyze_inactive(subs, all_changes, today);
    let cancelled = analyze_cancelled(subs);

    let mut total_yearly_savings = Vec::new();
    for s in &cycle {
        add_amount(&mut total_yearly_savings, &s.currency, s.yearly_savings);
    }
    total_yearly_savings.sort_by(|a, b| a.0.cmp(&b.0));
    for (_, total) in &mut total_yearly_savings {
        *total = round_currency(*total);
    }
    let mut cancelled_monthly = Vec::new();
    for s in &cancelled {
        add_amount(&mut cancelled_monthly, &s.currency, s.monthly);
    }

    Analysis {
        cycle,
        duplicates,
        inactive,
        cancelled,
        total_yearly_savings,
        cancelled_monthly,
    }
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
fn analyze_inactive(
    subs: &[Subscription],
    all_changes: &[PriceChange],
    today: NaiveDate,
) -> Vec<InactiveSuggestion> {
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
            if let Ok(d) = NaiveDate::parse_from_str(date_part, "%Y-%m-%d") {
                let days = (today - d).num_days();
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

fn add_amount(totals: &mut Vec<(String, f64)>, currency: &str, amount: f64) {
    match totals.iter_mut().find(|(c, _)| c == currency) {
        Some((_, total)) => *total += amount,
        None => totals.push((currency.to_string(), amount)),
    }
}

#[cfg(test)]
#[path = "analysis_tests.rs"]
mod tests;
