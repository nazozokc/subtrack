//! Logger — port of `@subtrack/lib/logger.ts`.
//! IMPORTANT: every level writes to STDOUT (console.log), never stderr.

use std::sync::Mutex;

/// Global output buffer; flushed to stdout once at the end of `run`.
pub static OUT: Mutex<String> = Mutex::new(String::new());

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

fn emit(color: &str, icon: &str, message: &str) {
    outln(&format!("\x1b[{}m{}\x1b[39m {}", color, icon, message))
}

/// consola.info
pub fn info(message: &str) {
    emit("36", "ℹ", message)
}

/// consola.success
pub fn success(message: &str) {
    emit("32", "✔", message)
}

/// consola.warn
pub fn warn(message: &str) {
    emit("33", "⚠", message)
}

/// consola.error
pub fn error(message: &str) {
    emit("31", "✖", message)
}

/// Report a fatal CLI error and mark the process to exit non-zero.
pub fn fail(message: &str) {
    error(message);
    crate::set_exit_code(1)
}
