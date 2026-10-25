#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Option {
        name: String,
        raw_name: String,
        value: Option<String>,
        inline_value: bool,
    },
    Positional {
        value: String,
    },
    Terminator,
}

pub(super) fn is_short_option(arg: &str) -> bool {
    let b = arg.as_bytes();
    b.len() == 2 && b[0] == b'-' && b[1] != b'-'
}

fn is_short_option_group(arg: &str) -> bool {
    let b = arg.as_bytes();
    b.len() > 2 && b[0] == b'-' && b[1] != b'-'
}

/// `--x` / `--x=v` long-option detection. The JS checks `arg.includes("=", 3)`
/// (a `=` at full-string index >= 3), i.e. a `=` at or after the 2nd name char.
fn is_long_option(arg: &str) -> bool {
    if !arg.starts_with("--") {
        return false;
    }
    let s = &arg[2..];
    !s.is_empty() && !s[1..].contains('=')
}

fn is_long_option_and_value(arg: &str) -> bool {
    if !arg.starts_with("--") {
        return false;
    }
    let s = &arg[2..];
    !s.is_empty() && s[1..].contains('=')
}

/// Split argv into tokens (port of tokenize).
pub fn tokenize(argv: &[String]) -> Vec<Token> {
    let mut tokens: Vec<Token> = Vec::new();
    let mut remainings: Vec<String> = argv.to_vec();
    let mut terminated = false;

    while let Some(arg) = remainings.first().cloned() {
        remainings.remove(0);

        if terminated {
            tokens.push(Token::Positional { value: arg });
            continue;
        }
        if arg == "--" {
            tokens.push(Token::Terminator);
            terminated = true;
            continue;
        }
        if is_short_option(&arg) {
            tokens.push(Token::Option {
                name: arg[1..2].to_string(),
                raw_name: arg.clone(),
                value: None,
                inline_value: false,
            });
            continue;
        }
        if is_short_option_group(&arg) {
            let eq_idx = arg.find('=');
            if eq_idx == Some(2) {
                // `-x=v` : single short option with an inline value
                tokens.push(Token::Option {
                    name: arg[1..2].to_string(),
                    raw_name: format!("-{}", &arg[1..2]),
                    value: Some(arg[3..].to_string()),
                    inline_value: true,
                });
                continue;
            }
            let mut expanded: Vec<String> = Vec::new();
            let mut has_separator = false;
            let mut short_value = String::new();
            for ch in arg[1..].chars() {
                if has_separator {
                    short_value.push(ch);
                } else if ch == '=' {
                    has_separator = true;
                } else {
                    expanded.push(format!("-{}", ch));
                }
            }
            if !short_value.is_empty() {
                expanded.push(short_value);
            }
            // unshift in reverse so the first element is processed first
            while let Some(e) = expanded.pop() {
                remainings.insert(0, e);
            }
            continue;
        }
        if is_long_option(&arg) {
            tokens.push(Token::Option {
                name: arg[2..].to_string(),
                raw_name: arg.clone(),
                value: None,
                inline_value: false,
            });
            continue;
        }
        if is_long_option_and_value(&arg) {
            let eq = arg.find('=').unwrap();
            let name = arg[2..eq].to_string();
            tokens.push(Token::Option {
                name: name.clone(),
                raw_name: format!("--{}", name),
                value: Some(arg[eq + 1..].to_string()),
                inline_value: true,
            });
            continue;
        }
        tokens.push(Token::Positional { value: arg });
    }
    tokens
}

// ── Resolver ─────────────────────────────────────────────────────────────
