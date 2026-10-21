//! Logger — port of `@subtrack/lib/logger.ts`.
//!
//! Informational output goes to stdout; warnings and errors go to stderr so a
//! piped `subtrack list` stays parseable even when something goes wrong.

use std::sync::Mutex;

/// Global output buffer; flushed to stdout once at the end of `run`.
pub static OUT: Mutex<String> = Mutex::new(String::new());
/// Diagnostics buffer; flushed to stderr once at the end of `run`.
pub static ERR: Mutex<String> = Mutex::new(String::new());

pub fn outln(s: &str) {
    let mut buf = OUT.lock().unwrap();
    buf.push_str(s);
    buf.push('\n')
}

pub fn out_str(s: &str) {
    OUT.lock().unwrap().push_str(s)
}

/// consola.log — plain line.
pub fn log(message: &str) {
    outln(message)
}

fn emit(buffer: &Mutex<String>, color: &str, icon: &str, message: &str) {
    let line = if crate::ansi::enabled() {
        format!("\x1b[{}m{}\x1b[39m {}", color, icon, message)
    } else {
        format!("{} {}", icon, message)
    };
    let mut buf = buffer.lock().unwrap();
    buf.push_str(&line);
    buf.push('\n')
}

/// consola.info
pub fn info(message: &str) {
    emit(&OUT, "36", "ℹ", message)
}

/// consola.success
pub fn success(message: &str) {
    emit(&OUT, "32", "✔", message)
}

/// consola.warn
pub fn warn(message: &str) {
    emit(&ERR, "33", "⚠", message)
}

/// consola.error
pub fn error(message: &str) {
    emit(&ERR, "31", "✖", message)
}

/// Report a fatal CLI error and mark the process to exit non-zero.
pub fn fail(message: &str) {
    error(message);
    crate::set_exit_code(1)
}
