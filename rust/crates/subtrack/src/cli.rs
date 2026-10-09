//! CLI bootstrap — port of `packages/subtrack/src/cli/index.ts` (the `cli()`
//! function) and the version fast path of `src/index.ts`.

use crate::logger::{self, OUT};
use crate::parser::{resolve_args, ArgSpec, ParsedArgs, LIST_SPEC};
use crate::router::{resolve_command, Resolved};

pub const NAME: &str = "subtrack";
pub const VERSION: &str = "10.0.0";

/// True when the only argument is --version/-v (fast path in src/index.ts).
fn is_standalone_version(args: &[String]) -> bool {
    args.len() == 1 && (args[0] == "--version" || args[0] == "-v")
}

/// Port of cli(): resolve, banner, validation errors, command run.
pub fn run(args: &[String]) -> i32 {
    if is_standalone_version(args) {
        logger::outln(VERSION);
        return 0;
    }

    let resolved = resolve_command(args);
    if let Resolved::NotFound(err) = &resolved {
        // priority error — no banner, no resolveArgs
        logger::log(&err.message);
        logger::error(&format!("CommandNotFoundError: {}", err.message));
        return 1;
    }

    let parsed = resolve_args(args, &LIST_SPEC, 1);

    // options.version beats everything (no banner)
    if parsed.bool("version") == Some(true) {
        logger::outln(VERSION);
        return 0;
    }

    // banner — skipped for machine-readable output (`--json`)
    if parsed.bool("json") != Some(true) {
        logger::log(&format!("{} ({} v{})", NAME, NAME, VERSION));
        logger::log("");
    }

    if parsed.bool("help") == Some(true) {
        // `list --help` lives in the help golden group — out of Phase 1 scope
        return 0;
    }

    if !parsed.errors.is_empty() {
        let lines: Vec<String> = parsed.errors.iter().map(|e| e.message.clone()).collect();
        logger::log(&lines.join("\n"));
        let first = &parsed.errors[0];
        logger::error(&format!("{}: {}", first.name, first.message));
        return 1;
    }

    let code = crate::list::run_list(&parsed);
    // fail() already recorded exit 1 for command-level validation errors
    if code != 0 {
        crate::set_exit_code(1)
    }

    0
}

/// Used by tests / future commands to build specs. Kept here so LIST_SPEC
/// stays the single source for the list flag surface.
#[allow(dead_code)]
fn _spec() -> &'static ArgSpec {
    &LIST_SPEC
}

#[allow(dead_code)]
fn _flush_handle() -> usize {
    OUT.lock().unwrap().len()
}

#[allow(dead_code)]
fn _parsed(_p: &ParsedArgs) {}