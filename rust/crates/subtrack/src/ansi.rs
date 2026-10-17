//! ANSI escape helpers mirroring @subtrack/lib/ansi.ts.

/// Named ANSI colors (foreground). Mirror of lib/src/ansi.ts FG_CODES.
pub fn fg_code(name: &str) -> &'static str {
    match name {
        "black" => "\x1b[30m",
        "red" => "\x1b[31m",
        "green" => "\x1b[32m",
        "yellow" => "\x1b[33m",
        "blue" => "\x1b[34m",
        "magenta" => "\x1b[35m",
        "cyan" => "\x1b[36m",
        "white" => "\x1b[37m",
        "gray" => "\x1b[90m",
        "brightBlack" => "\x1b[90m",
        "brightRed" => "\x1b[91m",
        "brightGreen" => "\x1b[92m",
        "brightYellow" => "\x1b[93m",
        "brightBlue" => "\x1b[94m",
        "brightMagenta" => "\x1b[95m",
        "brightCyan" => "\x1b[96m",
        "brightWhite" => "\x1b[97m",
        _ => "",
    }
}

/// Background code = foreground code + 10 (mirror of BG_CODES).
pub fn bg_code(name: &str) -> &'static str {
    match name {
        "black" => "\x1b[40m",
        "red" => "\x1b[41m",
        "green" => "\x1b[42m",
        "yellow" => "\x1b[43m",
        "blue" => "\x1b[44m",
        "magenta" => "\x1b[45m",
        "cyan" => "\x1b[46m",
        "white" => "\x1b[47m",
        "gray" => "\x1b[100m",
        "brightBlack" => "\x1b[100m",
        "brightRed" => "\x1b[101m",
        "brightGreen" => "\x1b[102m",
        "brightYellow" => "\x1b[103m",
        "brightBlue" => "\x1b[104m",
        "brightMagenta" => "\x1b[105m",
        "brightCyan" => "\x1b[106m",
        "brightWhite" => "\x1b[107m",
        _ => "",
    }
}

pub fn is_color_name(name: &str) -> bool {
    matches!(
        name,
        "black"

            | "red"
            | "green"
            | "yellow"
            | "blue"
            | "magenta"
            | "cyan"
            | "white"
            | "gray"
            | "brightBlack"
            | "brightRed"
            | "brightGreen"
            | "brightYellow"
            | "brightBlue"
            | "brightMagenta"
            | "brightCyan"
            | "brightWhite"
    )
}

/// bold = `\x1b[1m{s}\x1b[22m` (mirror of lib/src/ansi.ts).
pub fn bold(s: &str) -> String {
    format!("\x1b[1m{s}\x1b[22m")
}

/// dim = `\x1b[2m{s}\x1b[22m` (mirror of lib/src/ansi.ts).
pub fn dim(s: &str) -> String {
    format!("\x1b[2m{s}\x1b[22m")
}

/// Colored text closes with `\x1b[39m` (mirror of lib/src/ansi.ts).
pub fn colored(name: &str, s: &str) -> String {
    format!("{}{}\x1b[39m", fg_code(name), s)
}

pub fn red(s: &str) -> String {
    colored("red", s)
}

pub fn green(s: &str) -> String {
    colored("green", s)
}

pub fn yellow(s: &str) -> String {
    colored("yellow", s)
}

pub fn cyan(s: &str) -> String {
    colored("cyan", s)
}