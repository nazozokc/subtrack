//! `payment` command — private orchestration, calculation, and rendering layers.

mod calculate;
mod orchestrate;
mod render;

use crate::parser::ParsedArgs;

struct ApiUsage {
    total: i64,
    by_provider: Vec<(String, i64)>,
}

struct Conversion {
    currency: String,
    amounts: Vec<Option<f64>>,
    total: f64,
    api_total: Option<f64>,
}

#[cfg(test)]
mod tests;

/// Validate payment options and report subscription and optional API totals.
pub fn run_payment(parsed: &ParsedArgs) -> i32 {
    orchestrate::run_payment(parsed)
}
