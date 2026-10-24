//! Interactive entry menu shown when `subtrack` runs with no arguments.

use crate::logger;
use crate::parser::ParsedArgs;
use crate::prompt::{self, Choice};

pub fn run_menu() -> i32 {
    loop {
        let args = ParsedArgs::default();
        let choices = vec![
            Choice::new("List subscriptions", "list"),
            Choice::new("Add subscription", "add"),
            Choice::new("Edit subscription", "edit"),
            Choice::new("Delete subscription", "delete"),
            Choice::new("Summary", "summary"),
            Choice::new("Payment totals", "payment"),
            Choice::new("Budget status", "budget"),
            Choice::new("Yearly report", "report"),
            Choice::new("LLM usage", "usage"),
            Choice::new("Trials", "trial"),
            Choice::new("Exit", "exit"),
        ];
        let Some(action) = prompt::select("What would you like to do?", &choices, Some(0)) else {
            return 0;
        };
        let code = match action.as_str() {
            "list" => crate::list::run_list(&args),
            "add" => crate::subs::run_add(&args),
            "edit" => crate::subs::run_edit(&args),
            "delete" => crate::subs::run_delete(&args),
            "summary" => crate::summary::run_summary(&args),
            "payment" => crate::payment::run_payment(&args),
            "budget" => crate::budget::run_budget(&args),
            "report" => crate::report::run_report(&args),
            "usage" => crate::usage::run_usage_list(&args),
            "trial" => crate::trial::run_trial_list(),
            _ => return 0,
        };
        if code != 0 {
            logger::warn("Command failed");
        }
        logger::log("");
    }
}
