//! Command router — port of `packages/subtrack/src/cli/router.ts`.
//! Walks positional tokens through the subcommand registry until a leaf
//! command is reached; remaining positionals are command args.

use std::collections::{HashMap, HashSet};

use crate::commands::{COMMANDS, CommandDef, TOP_SUBCOMMANDS};
use crate::help::ArgKind;
use crate::parser::{Token, tokenize};

/// Error thrown for an unknown subcommand. `message` mirrors gunshi's
/// `CommandNotFoundError: Command not found: <name>` rendering.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct CommandNotFoundError {
    pub message: String,
}

/// The resolved command tree position. `command: None` = the entry command
/// ("subtrack" root — not part of the registry).
#[derive(Clone, Debug)]
pub struct Resolved {
    pub command: Option<&'static CommandDef>,
    pub path: Vec<String>,
    pub depth: usize,
    /// True when the resolved command itself has subcommands (drives usage).
    pub omitted: bool,
    /// The unresolved subcommand name when resolution hit an unknown token.
    pub unknown_name: Option<String>,
}

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

fn command_ref(idx: usize) -> &'static CommandDef {
    &COMMANDS[idx]
}

/// Entry subcommand map: name → COMMANDS index.
fn entry_map() -> HashMap<&'static str, usize> {
    TOP_SUBCOMMANDS
        .iter()
        .map(|(n, i)| (*n, *i as usize))
        .collect()
}

/// Extract only command-path positionals from raw argv. Option values are
/// skipped using the registry's schemas, so `--threshold 0.5` cannot be
/// mistaken for an unknown subcommand.
pub fn get_command_positionals(argv: &[String]) -> Vec<String> {
    let mut value_long_names: HashSet<String> = HashSet::new();
    let mut value_short_names: HashSet<char> = HashSet::new();

    // visit(command): collect value-taking option names from the command's
    // args and every nested subcommand (entry itself has no args).
    fn visit(c: &CommandDef, value_long: &mut HashSet<String>, value_short: &mut HashSet<char>) {
        for a in c.args {
            if a.kind != ArgKind::Boolean {
                let long_names = [a.name.to_string(), kebabize(a.name)];
                for n in &long_names {
                    value_long.insert(n.clone());
                }
                if let Some(s) = a.short {
                    value_short.insert(s);
                }
            }
        }
        for idx in c.subcommands {
            visit(command_ref(*idx as usize), value_long, value_short);
        }
    }

    for (_, idx) in TOP_SUBCOMMANDS {
        visit(
            command_ref(*idx as usize),
            &mut value_long_names,
            &mut value_short_names,
        );
    }

    let mut positionals: Vec<String> = Vec::new();
    let mut skip_next_positional = false;
    for token in tokenize(argv) {
        match token {
            Token::Terminator => break,
            Token::Option {
                name,
                raw_name,
                inline_value,
                ..
            } => {
                let takes_value = if raw_name.starts_with("--") {
                    value_long_names.contains(&name)
                } else {
                    value_short_names.contains(&name.chars().next().unwrap())
                };
                skip_next_positional = takes_value && !inline_value;
            }
            Token::Positional { value } => {
                if skip_next_positional {
                    skip_next_positional = false;
                    continue;
                }
                positionals.push(value);
            }
        }
    }
    positionals
}

/// Port of resolveCommand for the entry `subtrack` + TOP_SUBCOMMANDS.
pub fn resolve_command(positionals: &[String]) -> Resolved {
    let entry_has_subs = !TOP_SUBCOMMANDS.is_empty();

    if positionals.is_empty() {
        return Resolved {
            command: None,
            path: vec![],
            depth: 0,
            omitted: entry_has_subs,
            unknown_name: None,
        };
    }
    if !entry_has_subs {
        return Resolved {
            command: None,
            path: vec![],
            depth: 0,
            omitted: false,
            unknown_name: positionals.first().cloned(),
        };
    }

    let mut current_map: HashMap<&str, usize> = entry_map();
    let mut resolved_command: Option<&'static CommandDef> = None;
    let mut path: Vec<String> = Vec::new();

    for token in positionals {
        let idx = current_map.get(token.as_str()).copied();
        let Some(idx) = idx else {
            // unknown subcommand — parent (or entry) is the target
            let depth = path.len();
            return Resolved {
                command: resolved_command,
                path,
                depth,
                omitted: false,
                unknown_name: Some(token.clone()),
            };
        };
        let cmd = command_ref(idx);
        resolved_command = Some(cmd);
        path.push(token.clone());
        if cmd.subcommands.is_empty() {
            break;
        }
        current_map = cmd
            .subcommands
            .iter()
            .map(|i| (command_ref(*i as usize).name, *i as usize))
            .collect();
    }

    match resolved_command {
        Some(cmd) => {
            let depth = path.len();
            Resolved {
                command: Some(cmd),
                path,
                depth,
                omitted: !cmd.subcommands.is_empty(),
                unknown_name: None,
            }
        }
        None => {
            // unreachable when entry_has_subs, but mirror the TS fallback
            Resolved {
                command: None,
                path,
                depth: 0,
                omitted: entry_has_subs,
                unknown_name: None,
            }
        }
    }
}
