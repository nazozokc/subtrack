use crate::date::{occurrences_per_year, period_factor};
use crate::db::{Subscription, get_audit_logs_filtered};
use crate::timeline::MonthTotal;

pub(super) fn yearly_cost(sub: &Subscription) -> f64 {
    sub.price * occurrences_per_year(&sub.cycle)
}

fn month_end_string(year: i64, month0: i64) -> String {
    crate::date::last_day_of_month_string(year, month0)
}

/// Port of `calcYearlyTotals` (per calendar year, with contract-end logic).
pub(super) fn calc_yearly_totals(subs: &[Subscription], year: i64) -> Vec<MonthTotal> {
    let mut results = Vec::new();
    for m in 0..12 {
        let month_start = format!("{:04}-{:02}-01", year, m + 1);
        let month_end = month_end_string(year, m);
        let mut total = 0.0;
        for sub in subs {
            if sub.status == "archived" {
                continue;
            }
            let created = sub.created_at.as_str();
            if created
                .as_bytes()
                .get(..10)
                .map(|b| std::str::from_utf8(b).unwrap_or_default())
                .unwrap_or_default()
                > month_end.as_str()
            {
                continue;
            }
            // Subscription must already exist by the end of the month
            if created > month_end.as_str() {
                continue;
            }
            if sub.status == "cancelled" {
                let Some(end) = sub.contract_end.as_deref() else {
                    continue;
                };
                if end < month_start.as_str() {
                    continue;
                }
            }
            total += sub.price * period_factor(&sub.cycle, "monthly");
        }
        results.push(MonthTotal {
            label: format!("{:04}-{:02}", year, m + 1),
            year,
            month: m as usize,
            total,
        });
    }
    results
}

pub(super) fn calc_top(subs: &[Subscription], n: usize) -> Vec<Subscription> {
    let mut list: Vec<Subscription> = subs
        .iter()
        .filter(|s| s.status != "archived")
        .cloned()
        .collect();
    list.sort_by(|a, b| {
        yearly_cost(b)
            .partial_cmp(&yearly_cost(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    list.truncate(n);
    list
}

pub(super) fn calc_added(subs: &[Subscription], year: i64) -> Vec<Subscription> {
    let prefix = format!("{}-", year);
    let mut list: Vec<Subscription> = subs
        .iter()
        .filter(|s| s.created_at.starts_with(&prefix) && s.status != "archived")
        .cloned()
        .collect();
    list.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    list
}

pub(super) fn calc_cancelled(subs: &[Subscription], year: i64) -> Vec<(String, String)> {
    let prefix = format!("{}-", year);
    let mut results: Vec<(String, String)> = Vec::new();
    for sub in subs {
        if sub.status == "cancelled"
            && let Some(end) = sub.contract_end.as_deref()
            && end.starts_with(&prefix)
            && !results.iter().any(|(n, _)| n == &sub.name)
        {
            results.push((sub.name.clone(), end.to_string()));
        }
    }
    let entries = get_audit_logs_filtered(
        Some("subscription.cancel"),
        None,
        1000,
        0,
        Some(&format!("{}-01-01", year)),
        None,
    );
    for entry in entries {
        if entry.created_at.starts_with(&prefix) {
            let name = entry
                .details
                .clone()
                .unwrap_or_else(|| format!("#{}", entry.target_id.unwrap_or_default()));
            let date = entry.created_at[..10.min(entry.created_at.len())].to_string();
            if !results.iter().any(|(n, _)| n == &name) {
                results.push((name, date));
            }
        }
    }
    results.sort_by(|a, b| a.1.cmp(&b.1));
    results
}
