//! Usage / help renderer — port of `packages/subtrack/src/cli/help.ts`
//! (self-contained replacement for gunshi's `@gunshi/plugin-renderer`).

use crate::commands::CommandDef;

const LEFT_MARGIN: usize = 2;
const MIDDLE_MARGIN: usize = 10;

/// Argument / option kinds appearing in subtrack commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgKind {
    Boolean,
    String,
    Positional,
}

/// One argument/option schema entry (help surface).
#[derive(Clone, Copy, Debug)]
pub struct ArgSpec<'a> {
    pub name: &'a str,
    pub kind: ArgKind,
    pub short: Option<char>,
    pub description: &'a str,
    pub array: bool,
    pub required: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallMode {
    /// Entry command (top-level `--help`) — no description line.
    Entry,
    /// Resolved subcommand — shows the command description first.
    SubCommand,
}

/// kebabize: `camelCase` → `camel-case` (uppercase → `-lower`), mirror.
fn kebabize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_uppercase() {
            out.push('-');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// padEnd(s, n): right-pad to at least n chars with spaces (never truncates).
fn pad_end(s: &str, n: usize) -> String {
    let mut out = String::with_capacity(n);
    out.push_str(s);
    while out.len() < n {
        out.push(' ');
    }
    out
}

fn has_optional_args(args: &[ArgSpec]) -> bool {
    args.iter().any(|a| a.kind != ArgKind::Positional)
}

fn has_positional_args(args: &[ArgSpec]) -> bool {
    args.iter().any(|a| a.kind == ArgKind::Positional)
}

/// `-s, --name` / `--name` (kebabized when to_kebab).
fn make_short_long_option_pair(spec: &ArgSpec, name: &str, to_kebab: bool) -> String {
    let key = if to_kebab {
        kebabize(name)
    } else {
        name.to_string()
    };
    match spec.short {
        Some(s) => format!("-{}, --{}", s, key),
        None => format!("--{}", key),
    }
}

/// Undecorated positionals are required (no `default` support).
fn is_required_single_positional(spec: &ArgSpec) -> bool {
    spec.required != Some(false)
}

fn generate_positional_symbols(args: &[ArgSpec]) -> String {
    args.iter()
        .filter(|a| a.kind == ArgKind::Positional)
        .map(|a| {
            if a.array {
                if a.required == Some(true) {
                    format!("<{}> [<{}> ...]", a.name, a.name)
                } else {
                    format!("[<{}> ...]", a.name)
                }
            } else if is_required_single_positional(a) {
                format!("<{}>", a.name)
            } else {
                format!("[<{}>]", a.name)
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn generate_options_symbols(args: &[ArgSpec]) -> &'static str {
    if has_optional_args(args) {
        "<OPTIONS>"
    } else {
        ""
    }
}

/// A command row for the COMMANDS section: entry copy first, then sorted.
fn load_commands(
    command_name: &str,
    command_description: &str,
    subcommands: &[&CommandDef],
) -> Vec<(bool, String, String)> {
    let mut loaded: Vec<(bool, String, String)> = vec![(
        true,
        command_name.to_string(),
        command_description.to_string(),
    )];
    loaded.extend(
        subcommands
            .iter()
            .map(|c| (false, c.name.to_string(), c.description.to_string())),
    );
    loaded.sort_by(|a, b| {
        // entry first, then by name (localeCompare — names are ASCII)
        match (a.0, b.0) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.1.cmp(&b.1),
        }
    });
    loaded
}

/// Port of renderUsage. Returns the full help text (joined with "\n"), which
/// mirrors the TS output; may be empty for degenerate cases.
#[allow(clippy::too_many_arguments)]
pub fn render_usage(
    name: &str,
    command_name: &str,
    command_description: &str,
    path: &[String],
    omitted: bool,
    call_mode: CallMode,
    args: &[ArgSpec],
    subcommands: &[&CommandDef],
    to_kebab: bool,
) -> String {
    let mut messages: Vec<String> = Vec::new();

    if call_mode == CallMode::SubCommand && !command_description.is_empty() {
        messages.push(command_description.to_string());
        messages.push(String::new());
    }

    // ── USAGE ──
    messages.push("USAGE:".to_string());
    let option_symbols = generate_options_symbols(args);
    let positional_symbols = generate_positional_symbols(args);
    let loaded = load_commands(command_name, command_description, subcommands);
    let has_commands = loaded.len() > 1;

    let mut usage = name.to_string();
    if omitted && has_commands {
        usage.push_str(" [COMMANDS]");
    } else if call_mode == CallMode::SubCommand {
        usage.push_str(&format!(" {}", path.join(" ")));
    }
    if !option_symbols.is_empty() {
        usage.push_str(&format!(" {}", option_symbols));
    }
    if !positional_symbols.is_empty() {
        usage.push_str(&format!(" {}", positional_symbols));
    }
    messages.push(format!("{}{}", " ".repeat(LEFT_MARGIN), usage));
    messages.push(String::new());

    // ── COMMANDS + "for more info" hints ──
    if omitted && has_commands {
        messages.push("COMMANDS:".to_string());
        let command_max_length = loaded.iter().map(|(_, n, _)| n.len()).max().unwrap_or(0);
        let symbol_length = command_max_length;
        for (entry, cmd_name, desc) in &loaded {
            let command_str = if *entry {
                format!("[{}]", cmd_name)
            } else {
                cmd_name.clone()
            };
            let row = if desc.is_empty() {
                command_str
            } else {
                format!(
                    "{}{}",
                    pad_end(&command_str, symbol_length + MIDDLE_MARGIN),
                    desc
                )
            };
            messages.push(format!("{}{}", " ".repeat(LEFT_MARGIN), row));
        }
        messages.push(String::new());
        messages.push("For more info, run any command with the `--help` flag:".to_string());
        let base_path = if path.is_empty() {
            name.to_string()
        } else {
            format!("{} {}", name, path.join(" "))
        };
        for (entry, cmd_name, _) in &loaded {
            let hint = if *entry {
                format!("{} --help", base_path)
            } else {
                format!("{} {} --help", base_path, cmd_name)
            };
            messages.push(format!("{}{}", " ".repeat(LEFT_MARGIN), hint));
        }
        messages.push(String::new());
    }

    // ── ARGUMENTS ──
    if has_positional_args(args) {
        messages.push("ARGUMENTS:".to_string());
        let positionals: Vec<&ArgSpec> = args
            .iter()
            .filter(|a| a.kind == ArgKind::Positional)
            .collect();
        let args_max_length = positionals.iter().map(|a| a.name.len()).max().unwrap_or(0);
        for spec in &positionals {
            let desc = spec.description;
            let arg = format!(
                "{} {}",
                pad_end(spec.name, args_max_length + MIDDLE_MARGIN),
                desc
            );
            messages.push(format!("{}{}", " ".repeat(LEFT_MARGIN), arg));
        }
        messages.push(String::new());
    }

    // ── OPTIONS ──
    if has_optional_args(args) {
        messages.push("OPTIONS:".to_string());
        let options: Vec<(String, String)> = args
            .iter()
            .filter(|a| a.kind != ArgKind::Positional)
            .map(|a| {
                let key = make_short_long_option_pair(a, a.name, to_kebab);
                let display = if to_kebab {
                    kebabize(a.name)
                } else {
                    a.name.to_string()
                };
                let key = if a.kind == ArgKind::Boolean {
                    key
                } else {
                    format!("{} <{}>", key, display)
                };
                (a.name.to_string(), key)
            })
            .collect();
        let options_max_length = options.iter().map(|(_, k)| k.len()).max().unwrap_or(0);
        let by_name: std::collections::HashMap<&str, &ArgSpec> =
            args.iter().map(|a| (a.name, a)).collect();
        for (raw_name, key) in &options {
            let desc = by_name
                .get(raw_name.as_str())
                .map(|a| a.description)
                .unwrap_or("");
            let option = format!(
                "{}{}",
                pad_end(key, options_max_length + MIDDLE_MARGIN),
                desc
            );
            messages.push(format!("{}{}", " ".repeat(LEFT_MARGIN), option));
        }
        messages.push(String::new());
    }

    messages.join("\n")
}
