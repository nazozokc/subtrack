//! CLI entry — glue between the workspace crates. Command modules live in
//! this crate (entry → commands → handlers → DB layering); infrastructure
//! (core, table engine, DB, config, command registry, MCP) is re-exported
//! below so the pre-split `crate::<module>` paths keep resolving.

pub use subtrack_commands::{commands, help, router};
pub use subtrack_config::config;
pub use subtrack_core::{
    ansi, crypto, date, domain, flush, formats, fx, json, logger, parser, price, prompt, safe_path,
    set_exit_code, ui, validation, xlsx,
};
pub use subtrack_db::db;
pub use subtrack_mcp::mcp;
pub use subtrack_ui::table;

mod analytics;
mod audit;
mod backup;
mod budget;
mod bulk;
mod calendar;
mod changes;
mod check;
mod cleanup;
mod cli;
mod compare;
mod currency;
mod dedupe;
mod display;
mod export;
mod forecast;
mod history;
mod io;
mod list;
mod maintenance;
mod menu;
mod notify;
mod optimize;
mod payment;
mod profile;
mod prompts;
mod receipt;
mod report;
mod review;
mod search;
mod stats;
mod subs;
mod suggest;
mod suggest_parser;
mod summary;
mod tags;
mod templates;
mod timeline;
mod trial;
mod upcoming;
mod usage;

/// Remove plaintext backing files left by this run (port of the TS
/// `process.on("exit")` cleanup). Unlinking an open SQLite file is safe on
/// Unix; on platforms where it is not, the error is ignored.
fn cleanup_temp_files() {
    let dir = crypto::config_dir();
    let _ = std::fs::remove_file(dir.join(".subtrack.open.db"));
    crypto::cleanup_temp_files();
}

pub fn run(args: Vec<std::ffi::OsString>) -> i32 {
    let args: Vec<String> = args
        .into_iter()
        .filter_map(|a| a.into_string().ok())
        .collect();
    let code = cli::run(&args);
    let exit = logger::exit_code();
    cleanup_temp_files();
    flush();
    if exit != 0 { exit } else { code }
}
