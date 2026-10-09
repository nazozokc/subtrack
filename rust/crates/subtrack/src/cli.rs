//! CLI bootstrap — port of `packages/subtrack/src/cli/index.ts` (the `cli()`
//! function) and the version fast path + lazy banner logic of `src/index.ts`.

use crate::commands::{CommandDef, COMMANDS, TOP_SUBCOMMANDS};
use crate::help::{render_usage, ArgKind, ArgSpec, CallMode};
use crate::logger::{self, OUT};
use crate::parser::{resolve_args, tokenize, ParsedArgs, Token};
use crate::router::{get_command_positionals, resolve_command};

pub const NAME: &str = "subtrack";
pub const VERSION: &str = "10.0.0";

/// Global help/version options merged into every command's args.
pub const COMMON_ARGS: [ArgSpec<'static>; 2] = [
    ArgSpec {
        name: "help",
        kind: ArgKind::Boolean,
        short: Some('h'),
        description: "Display this help message",
        array: false,
        required: None,
    },
    ArgSpec {
        name: "version",
        kind: ArgKind::Boolean,
        short: Some('v'),
        description: "Display this version",
        array: false,
        required: None,
    },
];

/// True when the only argument is --version/-v (fast path in src/index.ts).
fn is_standalone_version(args: &[String]) -> bool {
    args.len() == 1 && (args[0] == "--version" || args[0] == "-v")
}

/// Port of shouldSuppressUsage: argv[0] == "mcp", or any tokenized option
/// named "json"/"j" (short groups and `--flag=value` collapse via tokenize).
fn should_suppress_usage(args: &[String]) -> bool {
    if args.first().map(|s| s.as_str()) == Some("mcp") {
        return true;
    }
    tokenize(args).iter().any(|t| match t {
        Token::Option { name, .. } => name == "json" || name == "j",
        _ => false,
    })
}

fn entry_desc() -> &'static str {
    "Manage subscription services from your terminal"
}

/// Port of cli(): resolve, banner, help, validation errors, command run.
pub fn run(args: &[String]) -> i32 {
    if is_standalone_version(args) {
        logger::outln(VERSION);
        return 0;
    }

    let silent = should_suppress_usage(args);

    let positionals = get_command_positionals(args);
    let resolved = resolve_command(&positionals);

    // ── 4. priority errors (CommandNotFoundError) — no banner, no resolveArgs
    if let Some(unknown) = &resolved.unknown_name {
        let message = format!("Command not found: {}", unknown);
        if !silent {
            logger::log(&message);
        }
        logger::error(&format!("CommandNotFoundError: {}", message));
        return 1;
    }

    // ── 3. resolve args for the target command (COMMON + command args) ──
    let mut specs: Vec<ArgSpec> = Vec::with_capacity(2 + 16);
    specs.extend_from_slice(&COMMON_ARGS);
    let (cmd_name, cmd_desc, to_kebab) = match resolved.command {
        Some(c) => {
            specs.extend_from_slice(c.args);
            (c.name, c.description, c.to_kebab)
        }
        None => (NAME, entry_desc(), false),
    };
    let parsed: ParsedArgs =
        resolve_args(args, &specs, resolved.depth, to_kebab, resolved.unknown_name.is_none());

    // ── 5. version ──
    if parsed.bool("version") == Some(true) {
        logger::outln(VERSION);
        return 0;
    }

    // ── 6. header ──
    if !silent {
        logger::log(&format!("{} ({} v{})", NAME, NAME, VERSION));
        logger::log("");
    }

    // ── 7. help ──
    if parsed.bool("help") == Some(true) {
        // subcommands for the resolved level: its own, or the CLI's for the entry
        let children: Vec<&'static CommandDef> = match resolved.command {
            Some(c) if !c.subcommands.is_empty() => c
                .subcommands
                .iter()
                .map(|i| &COMMANDS[*i as usize])
                .collect(),
            _ if resolved.depth == 0 => TOP_SUBCOMMANDS
                .iter()
                .map(|(_, i)| &COMMANDS[*i as usize])
                .collect(),
            _ => vec![],
        };
        let call_mode = if resolved.depth > 0 {
            CallMode::SubCommand
        } else {
            CallMode::Entry
        };
        let usage = render_usage(
            NAME,
            cmd_name,
            cmd_desc,
            &resolved.path,
            resolved.omitted,
            call_mode,
            &specs,
            &children,
            to_kebab,
        );
        if !usage.is_empty() && !silent {
            logger::log(&usage);
        }
        return 0;
    }

    // ── 8. validation errors ──
    if !parsed.errors.is_empty() {
        let lines: Vec<String> = parsed.errors.iter().map(|e| e.message.clone()).collect();
        if !silent {
            logger::log(&lines.join("\n"));
        }
        let first = &parsed.errors[0];
        logger::error(&format!("{}: {}", first.name, first.message));
        return 1;
    }

    // ── 9. run ──
    let path = resolved.path.join(" ");
    let code = match resolved.command {
        Some(_) => match path.as_str() {
            "list" => crate::list::run_list(&parsed),
            "check" => crate::check::run_check(&parsed),
            "summary" => crate::summary::run_summary(&parsed),
            "stats" => crate::stats::run_stats(&parsed),
            "export" => crate::export::run_export(&parsed, resolved.depth),
            "upcoming" => crate::upcoming::run_upcoming(&parsed),
            "payment" => crate::payment::run_payment(&parsed),
            "config list" => crate::config::handle_config_list(),
            "config get" => crate::config::handle_config_get(
                &parsed.str("key").unwrap_or_default(),
            ),
            "usage list" => crate::usage::run_usage_list(&parsed),
            "usage total" => crate::usage::run_usage_total(&parsed),
            "usage refresh" => crate::usage::run_usage_refresh(),
            "search" => crate::search::run_search(&parsed),
            "trial list" => crate::trial::run_trial_list(),
            "trial expiring" => crate::trial::run_trial_expiring(&parsed),
            "trial" => crate::trial::run_trial_usage(),
            p if p == "tags" || p.starts_with("tags ") => crate::tags::run_tags(&parsed),
            "tag list" => crate::tags::run_tag_list(&parsed),
            "analytics" => crate::analytics::run_analytics(&parsed),
            // entry menu and unported commands — later phases
            _ => 0,
        },
        None => 0,
    };
    if code != 0 {
        crate::set_exit_code(1);
    }

    0
}

#[allow(dead_code)]
fn _unused(_p: &ParsedArgs) {}

#[allow(dead_code)]
fn _flush_handle() -> usize {
    OUT.lock().unwrap().len()
}