//! JSON and terminal presentation of optimization results.

use super::analysis::Analysis;
use crate::ansi;
use crate::json::{number, quote_string};
use crate::logger::{log, out_str};
use crate::price::{format_price, round_currency};

/// Write the JSON report, preserving suggestion and currency ordering.
pub(super) fn render_json(report: &Analysis) {
    let mut items: Vec<String> = Vec::new();
    for s in &report.cycle {
        items.push(format!(
            "    {{\n      \"type\": \"cycle\",\n      \"name\": {},\n      \"currency\": {},\n      \"currentCycle\": \"monthly\",\n      \"suggestedCycle\": \"yearly\",\n      \"currentMonthly\": {},\n      \"suggestedMonthly\": {},\n      \"yearlySavings\": {}\n    }}",
            quote_string(&s.name),
            quote_string(&s.currency),
            number(s.current_monthly),
            number(s.suggested_monthly),
            number(s.yearly_savings)
        ));
    }
    for d in &report.duplicates {
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
    for s in &report.inactive {
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
    for c in &report.cancelled {
        items.push(format!(
            "    {{\n      \"type\": \"cancelled\",\n      \"name\": {},\n      \"price\": {},\n      \"currency\": {},\n      \"cycle\": {},\n      \"monthly\": {}\n    }}",
            quote_string(&c.name),
            number(c.price),
            quote_string(&c.currency),
            quote_string(&c.cycle),
            number(c.monthly)
        ));
    }
    let total_json = if report.total_yearly_savings.len() == 1 {
        number(report.total_yearly_savings[0].1)
    } else {
        "null".to_string()
    };
    let total_by_currency: Vec<String> = report
        .total_yearly_savings
        .iter()
        .map(|(ccy, total)| format!("    {}: {}", quote_string(ccy), number(*total)))
        .collect();
    out_str(&format!(
        "{{\n  \"suggestions\": [\n{}\n  ],\n  \"totalYearlySavings\": {},\n  \"totalYearlySavingsByCurrency\": {{\n{}\n  }}\n}}\n",
        items.join(",\n"),
        total_json,
        total_by_currency.join(",\n")
    ));
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

/// Write the terminal report, optionally highlighting a converted currency total.
pub(super) fn render_report(report: &Analysis, display_currency: Option<&str>) {
    let Analysis {
        cycle,
        duplicates,
        inactive,
        cancelled,
        total_yearly_savings,
        cancelled_monthly,
    } = report;
    let mut sections: Vec<String> = vec![ansi::bold("Cost Optimization Report")];

    // Cycle Optimization
    if !cycle.is_empty() {
        let mut lines: Vec<String> = vec![
            String::new(),
            ansi::bold("  Cycle Optimization"),
            String::new(),
        ];
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
        }
        if !total_yearly_savings.is_empty() {
            lines.push(format!("    {}", ansi::dim(&"─".repeat(40))));
            lines.push(format!(
                "    {}",
                ansi::green(&ansi::bold(&format!(
                    "Total savings: {}",
                    format_totals(total_yearly_savings, "/yr")
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
                format_totals(cancelled_monthly, "/mo")
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
