//! CLI entry — port of `src/index.ts` main + cli bootstrap.

mod analytics;
mod ansi;
mod check;
mod cli;
mod commands;
mod config;
mod crypto;
mod date;
mod db;
mod display;
mod export;
mod forecast;
mod fx;
mod help;
mod json;
mod list;
mod logger;
mod parser;
mod optimize;
mod payment;
mod price;
mod router;
mod search;
mod stats;
mod summary;
mod tags;
mod table;
mod trial;
mod upcoming;
mod usage;

use std::sync::atomic::{AtomicI32, Ordering};

static EXIT_CODE: AtomicI32 = AtomicI32::new(0);

/// Recorded by logger::fail() — the process exit code when the last
/// operation failed.
pub fn set_exit_code(code: i32) {
    EXIT_CODE.store(code, Ordering::SeqCst)
}

/// Flush the buffered stdout (the whole CLI output is buffered so that
/// ordering — banner before validation errors, etc. — is deterministic).
pub fn flush() {
    use std::io::Write;
    let out = logger::OUT.lock().unwrap().clone();
    logger::OUT.lock().unwrap().clear();
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    let _ = handle.write_all(out.as_bytes());
    let _ = handle.flush();
}

pub fn run(args: Vec<std::ffi::OsString>) -> i32 {
    let args: Vec<String> = args
        .into_iter()
        .filter_map(|a| a.into_string().ok())
        .collect();
    let code = cli::run(&args);
    let exit = EXIT_CODE.load(Ordering::SeqCst);
    flush();
    if exit != 0 {
        exit
    } else {
        code
    }
}