//! Common infrastructure and domain layer — the self-contained
//! `@subtrack/lib` heritage port: terminal IO (`ansi`, `ui`, `logger`),
//! the argument parser (`parser`), domain data (`domain`, `models`,
//! `validation`), pure services (`date`, `price`, `fx`, `json`, `xlsx`,
//! `formats`), storage plumbing (`crypto`, `safe_path`) and the
//! interactive prompt core (`prompt`).
//!
//! Nothing in this crate talks to the database or renders command output;
//! upper layers (`subtrack-db`, `subtrack-ui`, the CLI and MCP crates)
//! depend on it, never the other way around.

pub mod ansi;
pub mod calendar;
pub mod crypto;
pub mod date;
pub mod domain;
pub mod formats;
pub mod fx;
pub mod json;
pub mod logger;
pub mod models;
pub mod parser;
pub mod price;
pub mod prompt;
pub mod safe_path;
pub mod ui;
pub mod upcoming;
pub mod validation;
pub mod xlsx;

pub use logger::{exit_code, flush, set_exit_code};
