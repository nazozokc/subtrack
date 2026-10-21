//! Terminal capability detection and the process-wide display policy.
//!
//! Everything that depends on whether output is going to a human (colors,
//! banner, terminal width) is resolved here once at startup. Commands and
//! renderers ask this module instead of inspecting the environment on their
//! own.

use std::env;
use std::io::IsTerminal;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

const AUTO: u8 = 0;
const ALWAYS: u8 = 1;
const NEVER: u8 = 2;

static COLOR_MODE: AtomicU8 = AtomicU8::new(AUTO);
static WIDE: AtomicBool = AtomicBool::new(false);

/// True when stdout is attached to a terminal.
pub fn is_stdout_tty() -> bool {
    std::io::stdout().is_terminal()
}

/// Resolve the global display policy from the environment and CLI flags.
///
/// Precedence: explicit flags > environment > terminal detection. `--no-color`
/// always wins over `--color=...`, because the safer choice should be the one
/// the user cannot accidentally override.
pub fn init(args: &[String]) {
    let mut mode = env_color_mode();
    let mut wide = false;

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "--no-color" => mode = NEVER,
            "--wide" => wide = true,
            "--color" => {
                if let Some(value) = args.get(i + 1)
                    && let Some(parsed) = parse_color_mode(value)
                {
                    mode = parsed;
                    i += 1;
                }
            }
            _ => {
                if let Some(value) = arg.strip_prefix("--color=")
                    && let Some(parsed) = parse_color_mode(value)
                {
                    mode = parsed;
                }
            }
        }
        i += 1;
    }

    if args.iter().any(|arg| arg == "--no-color") {
        mode = NEVER;
    }

    COLOR_MODE.store(mode, Ordering::Relaxed);
    WIDE.store(wide, Ordering::Relaxed);
}

fn env_color_mode() -> u8 {
    if env::var_os("NO_COLOR")
        .map(|value| !value.is_empty())
        .unwrap_or(false)
    {
        return NEVER;
    }
    if let Some(value) = env::var_os("FORCE_COLOR")
        && !value.is_empty()
    {
        return if value == "0" { NEVER } else { ALWAYS };
    }
    if env::var("CLICOLOR_FORCE")
        .map(|value| !value.is_empty() && value != "0")
        .unwrap_or(false)
    {
        return ALWAYS;
    }
    if env::var("CLICOLOR")
        .map(|value| value == "0")
        .unwrap_or(false)
    {
        return NEVER;
    }
    if env::var("TERM")
        .map(|value| value == "dumb")
        .unwrap_or(false)
    {
        return NEVER;
    }
    AUTO
}

fn parse_color_mode(value: &str) -> Option<u8> {
    match value.trim().to_ascii_lowercase().as_str() {
        "always" | "force" | "true" | "yes" | "1" => Some(ALWAYS),
        "never" | "false" | "no" | "0" => Some(NEVER),
        "auto" => Some(AUTO),
        _ => None,
    }
}

/// Whether ANSI styling should be emitted.
pub fn color_enabled() -> bool {
    match COLOR_MODE.load(Ordering::Relaxed) {
        ALWAYS => true,
        NEVER => false,
        _ => is_stdout_tty(),
    }
}

/// `--wide` disables the narrow-terminal layout.
pub fn wide() -> bool {
    WIDE.load(Ordering::Relaxed)
}

/// Usable output width in columns.
///
/// `COLUMNS` is honored even when stdout is piped so snapshot tests and
/// scripts can pin the layout. Without a terminal the conventional 80-column
/// default keeps piped output stable.
pub fn term_width() -> i64 {
    if wide() {
        return 200;
    }
    if let Some(cols) = env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|cols| *cols >= 20)
    {
        return cols as i64;
    }
    if is_stdout_tty()
        && let Some((terminal_size::Width(width), _)) = terminal_size::terminal_size()
    {
        return (width as i64).max(20);
    }
    80
}

#[cfg(test)]
mod tests {
    use super::parse_color_mode;

    #[test]
    fn parses_color_modes() {
        assert_eq!(parse_color_mode("always"), Some(1));
        assert_eq!(parse_color_mode("never"), Some(2));
        assert_eq!(parse_color_mode("auto"), Some(0));
        assert_eq!(parse_color_mode(" TRUE "), Some(1));
        assert_eq!(parse_color_mode("sometimes"), None);
    }
}
