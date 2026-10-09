//! Command router — port of `packages/subtrack/src/cli/router.ts`
//! scoped to the commands relevant to the list/reads golden set.

use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct CommandNotFoundError {
    pub message: String,
}

#[derive(Clone, Debug)]
pub enum Resolved {
    /// The `list` command was resolved; `skip` = number of leading
    /// argv tokens consumed by the command path (used by resolveArgs).
    List { skip: usize },
    NotFound(CommandNotFoundError),
}

/// First positional token (a token not starting with "-").
fn first_positional(args: &[String]) -> Option<&str> {
    args.iter().find(|tok| !tok.starts_with('-')).map(|s| s.as_str())
}

/// Port of resolveCommand for the entry `subtrack` + the list subcommand.
pub fn resolve_command(args: &[String]) -> Resolved {
    match first_positional(args) {
        Some("list") => Resolved::List { skip: 1 },
        Some(tok) => Resolved::NotFound(CommandNotFoundError {
            message: format!("Command not found: {}", tok),
        }),
        None => Resolved::List { skip: 0 },
    }
}

/// Debug helper for future command groups (unused in the list scope).
#[allow(dead_code)]
pub fn _command_map() -> HashMap<String, u8> {
    let mut m = HashMap::new();
    m.insert("list".to_string(), 1);
    m
}