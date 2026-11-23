//! Interactive entry menu shown when `subtrack` runs with no arguments.

use crate::i18n::tr;
use crate::logger;
use crate::parser::ParsedArgs;
use crate::prompt::{self, Choice};

pub fn run_menu() -> i32 {
    loop {
        let args = ParsedArgs::default();
        let choices = vec![
            Choice::new(&tr("List subscriptions"), "list"),
            Choice::new(&tr("Add subscription"), "add"),
            Choice::new(&tr("Edit subscription"), "edit"),
            Choice::new(&tr("Delete subscription"), "delete"),
            Choice::new(&tr("Summary"), "summary"),
            Choice::new(&tr("Payment totals"), "payment"),
            Choice::new(&tr("Budget status"), "budget"),
            Choice::new(&tr("Yearly report"), "report"),
            Choice::new(&tr("Upcoming bills"), "upcoming"),
            Choice::new(&tr("Statistics"), "stats"),
            Choice::new(&tr("Calendar"), "calendar"),
            Choice::new(&tr("Tags"), "tags"),
            Choice::new(&tr("LLM usage"), "usage"),
            Choice::new(&tr("Trials"), "trial"),
            Choice::new(&tr("Configuration"), "config"),
            Choice::new(&tr("Help — all commands"), "help"),
            Choice::new(&tr("Exit"), "exit"),
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
            "upcoming" => crate::upcoming::run_upcoming(&args),
            "stats" => crate::stats::run_stats(&args),
            "calendar" => crate::calendar::run_calendar(&args),
            "tags" => crate::tags::run_tag_list(&args),
            "usage" => crate::usage::run_usage_list(&args),
            "trial" => crate::trial::run_trial_list(),
            "config" => crate::config::handle_config_list(),
            "help" => {
                crate::cli::print_root_help();
                0
            }
            _ => return 0,
        };
        if code != 0 {
            logger::warn("Command failed");
        }
        logger::log("");
    }
}
