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
            "add" => crate::subs::run_add(&parsed),
            "edit" => crate::subs::run_edit(&parsed),
            "clone" => crate::subs::run_clone(&parsed),
            "delete" => crate::subs::run_delete(&parsed),
            "unarchive" => crate::subs::run_unarchive(&parsed),
            "archive" => crate::subs::run_archive(&parsed),
            "pause" => crate::subs::run_pause(&parsed),
            "resume" => crate::subs::run_resume(&parsed),
            "renew" => crate::subs::run_renew(&parsed),
            "cancel" => crate::subs::run_cancel(&parsed),
            "import" => crate::io::run_import(&parsed),
            "restore" => crate::backup::run_restore(&parsed),
            "backup" => crate::backup::run_backup(&parsed),
            "bulk status" => crate::bulk::run_bulk_status(&parsed),
            "bulk delete" => crate::bulk::run_bulk_delete(&parsed),
            "bulk tag add" => crate::bulk::run_bulk_tag_add(&parsed),
            "bulk tag remove" => crate::bulk::run_bulk_tag_remove(&parsed),
            "config reset" => crate::config::handle_config_reset(),
            "config set" => crate::config::handle_config_set(
                &parsed.str("key").unwrap_or_default(),
                &parsed.str("value").unwrap_or_default(),
            ),
            "tag rename" => crate::tags::run_tag_rename(&parsed),
            "tag delete" => crate::tags::run_tag_delete(&parsed),
            "tag merge" => crate::tags::run_tag_merge(&parsed),
            "tag prune" => crate::tags::run_tag_prune(&parsed),
            "template add" => crate::templates::run_template_add(&parsed),
            "template delete" => crate::templates::run_template_delete(&parsed),
            "template use" => crate::templates::run_template_use(&parsed),
            "template edit" => crate::templates::run_template_edit(&parsed),
            "profile save" => crate::profile::run_profile_save(&parsed),
            "profile switch" => crate::profile::run_profile_switch(&parsed),
            "profile delete" => crate::profile::run_profile_delete(&parsed),
            "suggest add" => crate::suggest::run_suggest_add(&parsed),
            "suggest dismiss" => crate::suggest::run_suggest_dismiss(&parsed),
            "trial add" => crate::trial::run_trial_add(&parsed),
            "trial delete" => crate::trial::run_trial_delete(&parsed),
            "usage add" => crate::usage::run_usage_add(&parsed),
            "usage edit" => crate::usage::run_usage_edit(&parsed),
            "usage delete" => crate::usage::run_usage_delete(&parsed),
            "usage import" => crate::usage::run_usage_import(&parsed),
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
            "forecast" => crate::forecast::run_forecast(&parsed),
            "optimize" => crate::optimize::run_optimize(&parsed),
            "audit list" => crate::audit::run_audit(&parsed, resolved.depth, &resolved.path),
            "audit prune" => crate::audit::run_audit(&parsed, resolved.depth, &resolved.path),
            "audit" => crate::audit::run_audit(&parsed, resolved.depth, &resolved.path),
            "maintenance" => crate::maintenance::run_maintenance(&parsed),
            "cleanup" => crate::cleanup::run_cleanup(&parsed),
            "review" => crate::review::run_review(&parsed),
            "budget" => crate::budget::run_budget(&parsed),
            "currency" => crate::currency::run_currency(&parsed),
            "profile list" => crate::profile::run_profiles(&parsed),
            "profile show" => crate::profile::run_profiles(&parsed),
            "compare" => crate::compare::run_compare(&parsed),
            "calendar" => crate::calendar::run_calendar(&parsed),
            "notify" => crate::notify::run_notify(&parsed),
            "timeline" => crate::timeline::run_timeline(&parsed),
            "report" => crate::report::run_report(&parsed),
            "yearly" => crate::report::run_yearly(&parsed),
            "history" => crate::history::run_history(&parsed),
            "changes" => crate::changes::run_changes(&parsed),
            "dedupe" => crate::dedupe::run_dedupe(&parsed),
            "template list" => crate::templates::run_template_list(&parsed),
            "suggest list" => crate::suggest::run_suggest_list(&parsed),
            "suggest view" => crate::suggest::run_suggest_view(&parsed),
            "receipt" => crate::receipt::run_receipt(&parsed),
            "mcp" => crate::mcp::run_mcp(&parsed),
            // entry menu and unported commands — later phases
            _ => 0,
        },
        None => {
            if !silent {
                // Print menu header (port of showMenuHeader)
                print_root_header();
            }
            0
        },
    };
    if code != 0 {
        crate::set_exit_code(1);
    }

    0
}

fn print_root_header() {
    let subs = match crate::db::get_subscriptions(&crate::db::QueryOptions::default()) {
        Ok(s) => s,
        Err(_) => return,
    };
    let count = subs.iter().filter(|s| s.status != "cancelled").count();
    let total_all = subs.len();
    let active: Vec<_> = subs.iter().filter(|s| s.status != "cancelled").cloned().collect();
    logger::log(&format!("\x1b[1m\x1b[36msubtrack v{}\x1b[39m\x1b[22m", VERSION));
    logger::log(&format!(
        "\x1b[2m  {} subscriptions · {}\x1b[22m",
        total_all,
        crate::db::db_path().display()
    ));
    if !active.is_empty() {
        let data = crate::summary::calc_summary_internal(&active);
        let mut parts: Vec<String> = Vec::new();
        let mut sorted_currencies: Vec<_> = data.monthly_by_currency.iter().collect();
        sorted_currencies.sort_by(|a, b| a.0.cmp(&b.0));
        for (ccy, total) in &sorted_currencies {
            parts.push(crate::price::format_price(*total, ccy));
        }
        logger::log(&format!("  Monthly: {}", parts.join(" + ")));
        let days = 7i64;
        let (entries, _) = crate::upcoming::upcoming_with_currency_internal(&active, days, None);
        if !entries.is_empty() {
            let mut totals: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
            for entry in &entries {
                let sub = &active[entry.idx];
                *totals.entry(sub.currency.clone()).or_insert(0.0) += entry.amount;
            }
            let mut parts2: Vec<String> = Vec::new();
            let mut sorted2: Vec<_> = totals.iter().collect();
            sorted2.sort_by(|a,b| a.0.cmp(b.0));
            for (ccy, total) in &sorted2 {
                parts2.push(crate::price::format_price(**total, ccy));
            }
            let bill_label = if entries.len() == 1 { "bill" } else { "bills" };
            logger::log(&format!(
                "  Next 7 days: {} {} ({})",
                entries.len(),
                bill_label,
                parts2.join(" + ")
            ));
        }
    } else {
        logger::log("\x1b[2m  No subscriptions yet — choose Add to create one\x1b[22m");
    }
    logger::log("\x1b[2m────────────────────────────────────────────────────\x1b[0m");
    logger::log("");
}

#[allow(dead_code)]
fn _unused(_p: &ParsedArgs) {}

#[allow(dead_code)]
fn _flush_handle() -> usize {
    OUT.lock().unwrap().len()
}