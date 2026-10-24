//! `usage` commands — port of `usage.ts` (list), `usage-total.ts` (total)
//! and `usage-refresh.ts` (refresh; sources absent in the golden env).

mod add;
mod delete;
mod edit;
mod import;
mod list;
mod refresh;
mod total;

pub use add::run_usage_add;
pub use delete::run_usage_delete;
pub use edit::run_usage_edit;
pub use import::run_usage_import;
pub use list::run_usage_list;
pub use refresh::run_usage_refresh;
pub use total::run_usage_total;

pub(super) const LLM_PROVIDERS: [&str; 8] = [
    "openai",
    "anthropic",
    "google-ai",
    "mistral",
    "groq",
    "together",
    "deepseek",
    "cohere",
];

pub(super) fn validate_cost_input(value: &str) -> Result<(), String> {
    match crate::validation::js_number(value.trim()) {
        Some(n) if n.is_finite() && n >= 0.0 => Ok(()),
        _ => Err("Enter a non-negative number (e.g. 0.50 for 50 cents)".to_string()),
    }
}
