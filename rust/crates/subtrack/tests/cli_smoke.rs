//! End-to-end smoke tests: run the compiled binary against a disposable
//! database directory. Each case spawns the CLI (no shared process state),
//! pins `SUBTRACK_TODAY` for deterministic date output, and asserts exit
//! codes plus the load-bearing output fragments.

use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_subtrack");
const TODAY: &str = "2026-10-08";

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "subtrack-smoke-{}-{}-{}",
        name,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct Env {
    dir: PathBuf,
}

impl Env {
    fn new(name: &str) -> Self {
        Self { dir: scratch(name) }
    }

    fn envs(&self) -> Vec<(&str, String)> {
        vec![
            ("SUBTRACK_TODAY", TODAY.to_string()),
            ("SUBSC_CLI_DB_DIR", self.dir.display().to_string()),
            ("NO_COLOR", "1".to_string()),
            ("TERM", "dumb".to_string()),
        ]
    }

    /// Run the CLI with the per-test env (never mutating global state).
    fn run_custom(
        &self,
        args: &[&str],
        input: &str,
        extra: &[(&str, &str)],
        remove: &[&str],
    ) -> Output {
        let mut cmd = Command::new(BIN);
        cmd.args(args);
        for (k, v) in self.envs() {
            cmd.env(k, v);
        }
        for key in remove {
            cmd.env_remove(key);
        }
        for (k, v) in extra {
            cmd.env(k, v);
        }
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        let mut child = cmd.spawn().expect("spawn subtrack");
        child
            .stdin
            .as_mut()
            .expect("open subtrack stdin")
            .write_all(input.as_bytes())
            .ok();
        child.wait_with_output().expect("collect subtrack output")
    }

    fn run_with_stdin(&self, args: &[&str], input: &str) -> Output {
        self.run_custom(args, input, &[], &[])
    }

    fn run(&self, args: &[&str]) -> Output {
        self.run_with_stdin(args, "")
    }

    /// Run with prompts forced on (the child's stdin is still a pipe).
    fn run_interactive(&self, args: &[&str], input: &str) -> Output {
        let mut cmd = Command::new(BIN);
        cmd.args(args);
        for (k, v) in self.envs() {
            cmd.env(k, v);
        }
        cmd.env("SUBTRACK_ASSUME_TTY", "1");
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        let mut child = cmd.spawn().expect("spawn subtrack");
        child
            .stdin
            .as_mut()
            .expect("open subtrack stdin")
            .write_all(input.as_bytes())
            .ok();
        child.wait_with_output().expect("collect subtrack output")
    }

    fn text(&self, args: &[&str]) -> String {
        let out = self.run(args);
        let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
        s.push_str(&String::from_utf8_lossy(&out.stderr));
        s
    }

    /// Run with a deterministic passphrase so encryption is exercised in a
    /// separate child process (no mutable process-global env in the test).
    fn run_passphrase(&self, args: &[&str]) -> Output {
        self.run_custom(
            args,
            "",
            &[("SUBSC_CLI_DB_PASSPHRASE", "rekey-passphrase")],
            &[],
        )
    }
}

/// Create the on-disk shape of a passphrase install created before the KDF
/// marker existed: explicit Legacy marker and a persisted salt.
fn legacy_passphrase_dir(env: &Env) {
    std::fs::write(env.dir.join(".key.kdf"), b"1\n").unwrap();
    std::fs::write(env.dir.join(".key.salt"), vec![7u8; 32]).unwrap();
}

/// On-disk shape of an install created by the previous (Fast) KDF version.
fn fast_passphrase_dir(env: &Env) {
    std::fs::write(env.dir.join(".key.kdf"), b"2\n").unwrap();
    std::fs::write(env.dir.join(".key.salt"), vec![11u8; 32]).unwrap();
}

fn passphrase_add_args(name: &str) -> Vec<&str> {
    vec![
        "add",
        "--name",
        name,
        "--price",
        "12.34",
        "--currency",
        "USD",
        "--cycle",
        "monthly",
        "--tags",
        "test",
        "--billingDay",
        "8",
        "--status",
        "active",
    ]
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Seed 3 subscriptions via the scriptable add flags (no prompts: every
/// prompted field — name/price/currency/cycle/status — is provided).
fn seed(env: &Env) {
    let entries: &[&[&str]] = &[
        &[
            "--name",
            "Netflix",
            "--price",
            "15.99",
            "--currency",
            "USD",
            "--cycle",
            "monthly",
            "--tags",
            "streaming,video",
            "--billingDay",
            "8",
            "--status",
            "active",
        ],
        &[
            "--name",
            "Spotify",
            "--price",
            "9.99",
            "--currency",
            "USD",
            "--cycle",
            "monthly",
            "--tags",
            "music,audio",
            "--billingDay",
            "10",
            "--status",
            "active",
        ],
        &[
            "--name",
            "iCloud",
            "--price",
            "250",
            "--currency",
            "JPY",
            "--cycle",
            "yearly",
            "--tags",
            "cloud",
            "--billingDay",
            "12",
            "--status",
            "active",
        ],
    ];
    for entry in entries {
        let mut args: Vec<&str> = vec!["add"];
        args.extend_from_slice(entry);
        let out = env.run(&args);
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.status.code(), Some(0), "add failed: {text}");
        assert!(text.contains("Added subscription:"), "add failed: {text}");
    }
}

#[test]
fn add_then_list_shows_expectations() {
    let env = Env::new("add-list");
    seed(&env);

    let out = env.text(&["list", "-j"]);
    assert!(out.contains("\"name\": \"Netflix\""), "json list: {out}");
    assert!(out.contains("\"price\": 15.99"), "json list: {out}");
    assert_eq!(out.matches("\"id\":").count(), 3, "sub count: {out}");

    let table = env.text(&["list"]);
    assert!(table.contains("Netflix"), "table list: {table}");
    assert!(table.contains("$15.99"), "table list: {table}");
    // Interior rows of a 3-row table must render in compact mode too
    // (divider-skipping must not swallow data rows).
    assert!(table.contains("Spotify"), "table list: {table}");
    assert!(table.contains("iCloud"), "table list: {table}");

    // filtered by tag: only Spotify carries "music"
    let out = env.text(&["tags", "music"]);
    assert!(out.contains("Spotify"), "tag filter: {out}");
    assert!(!out.contains("Netflix"), "tag filter: {out}");
}

#[test]
fn summary_and_payment_report_totals() {
    let env = Env::new("summary-payment");
    seed(&env);

    let summary = env.text(&["summary"]);
    assert!(
        summary.contains("Total subscriptions:"),
        "summary: {summary}"
    );
    assert!(summary.contains("$25.98"), "summary: {summary}");

    // Netflix/Spotify in USD and ¥250/12 in JPY → both buckets line up
    let payment = env.text(&["payment", "monthly"]);
    assert!(payment.contains("25.98"), "payment: {payment}");
    assert!(payment.contains("20.83"), "payment: {payment}");

    let json_summary = env.text(&["summary", "--json"]);
    assert!(
        json_summary.contains("\"totalCount\""),
        "summary json: {json_summary}"
    );
    assert!(
        json_summary.contains("\"monthlyByCurrency\""),
        "summary json: {json_summary}"
    );
}

#[test]
fn payment_uses_natural_period_labels() {
    let env = Env::new("payment-periods");
    seed(&env);

    let yearly = env.text(&["payment", "yearly"]);
    assert!(yearly.contains("¥250/year"), "payment yearly: {yearly}");
    assert!(yearly.contains("$311.76/year"), "payment yearly: {yearly}");
    assert!(!yearly.contains("/yearly"), "payment yearly: {yearly}");

    let quarterly = env.text(&["payment", "quarterly"]);
    assert!(
        quarterly.contains("/quarter"),
        "payment quarterly: {quarterly}"
    );
    assert!(
        !quarterly.contains("/quarterly"),
        "payment quarterly: {quarterly}"
    );
}

#[test]
fn summary_and_analytics_compare_most_expensive_per_currency() {
    let env = Env::new("most-expensive-currency");
    seed(&env);

    let summary = env.text(&["summary"]);
    assert!(
        summary.contains("Most expensive by currency:"),
        "summary: {summary}"
    );
    assert!(summary.contains("JPY  iCloud"), "summary: {summary}");
    assert!(summary.contains("USD  Netflix"), "summary: {summary}");

    let json = env.text(&["summary", "--json"]);
    assert!(
        json.contains("\"mostExpensiveByCurrency\""),
        "summary json: {json}"
    );
    assert!(!json.contains("\"mostExpensive\":"), "summary json: {json}");

    let analytics = env.text(&["analytics", "--period", "yearly"]);
    assert!(
        analytics.contains("Most expensive by currency:"),
        "analytics: {analytics}"
    );
    assert!(
        analytics.contains("¥250/year (1 sub)"),
        "analytics: {analytics}"
    );
    assert!(
        !analytics.contains("¥250/yearly (1 sub)"),
        "analytics: {analytics}"
    );

    let converted: serde_json::Value = serde_json::from_slice(
        &env.run(&["analytics", "--currency", "JPY", "--json"])
            .stdout,
    )
    .expect("analytics json");
    assert_eq!(
        converted["monthlyByCurrency"]["JPY"].as_f64(),
        Some(3924.33)
    );
    assert!(converted["monthlyByCurrency"].get("USD").is_none());
    assert_eq!(converted["mostExpensive"]["currency"], "JPY");
    assert_eq!(
        converted["monthlyByTag"]["streaming"]["monthly"]["JPY"],
        2402.5
    );
}

#[test]
fn unknown_flags_exit_1_and_fail_loudly() {
    let env = Env::new("errors");
    let out = env.run(&["list", "--min-pricee", "5"]);
    assert_eq!(out.status.code(), Some(1));
    // Diagnostics belong on stderr so piped data stays clean.
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Unknown option '--min-pricee'"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("✖"), "error marker missing: {stderr}");

    // required positional missing → silent no-op, exit 0
    let out = env.run(&["delete"]);
    assert_eq!(out.status.code(), Some(0));

    // `--` positional still flows through command handling without a crash
    let out = env.run(&["delete", "--", "-x"]);
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn edit_updates_price() {
    let env = Env::new("edit");
    seed(&env);

    let out = env.text(&["edit", "1", "--price", "20.99"]);
    assert!(out.contains("Updated: Netflix"), "edit: {out}");

    let out = env.text(&["list", "-j"]);
    assert!(out.contains("\"price\": 20.99"), "after edit: {out}");
}

#[test]
fn delete_removes_entries() {
    let env = Env::new("delete");
    seed(&env);
    let out = env.text(&["delete", "1", "2"]);
    assert!(out.contains("Deleted: Netflix"), "delete: {out}");
    assert!(out.contains("Deleted: Spotify"), "delete: {out}");

    let out = env.text(&["list", "-j"]);
    assert!(out.contains("iCloud"), "after delete: {out}");
    assert!(!out.contains("Netflix"), "after delete: {out}");
    assert_eq!(out.matches("\"id\":").count(), 1, "sub count: {out}");
}

#[test]
fn export_prints_documents_to_stdout() {
    let env = Env::new("export");
    seed(&env);

    let json = env.text(&["export", "json"]);
    assert!(
        json.contains("\"name\": \"Netflix\""),
        "json export: {json}"
    );

    let md = env.text(&["export", "md"]);
    assert!(md.contains("| name | status |"), "md export: {md}");
    assert!(md.contains("| Netflix |"), "md export: {md}");
}

#[test]
fn version_and_help_output() {
    let env = Env::new("version");
    let out = env.run(&["--version"]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains(env!("CARGO_PKG_VERSION")),
        "version: {stdout}"
    );
    assert!(stdout.contains('.'), "version: {stdout}");

    let out = env.run(&["--help"]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("USAGE"), "help: {stdout}");
    assert!(stdout.contains("subtrack"), "help: {stdout}");
    assert!(stdout.contains("[OPTIONS]"), "help: {stdout}");
    assert!(!stdout.contains("<OPTIONS>"), "help: {stdout}");
}

#[test]
fn mcp_tools_list_probe() {
    let env = Env::new("mcp");
    let payload = concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":",
        "{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{},\"clientInfo\":",
        "{\"name\":\"smoke\",\"version\":\"0.0\"}}}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\",\"params\":{}}\n"
    );
    let out = env.run_with_stdin(&["mcp"], payload);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("\"tools\""), "mcp output: {stdout}");
    assert!(stdout.contains("list_subscriptions"), "mcp tool: {stdout}");
    assert!(stdout.contains("subtrack-mcp"), "mcp server info: {stdout}");
}

#[test]
fn mcp_help_prints_usage_without_starting_server() {
    let env = Env::new("mcp-help");
    let out = env.run(&["mcp", "--help"]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("USAGE"), "mcp help: {stdout}");
    assert!(stdout.contains("mcp"), "mcp help: {stdout}");
}

#[test]
fn add_without_status_scripts_the_active_default() {
    // The help advertises "default: active" — a scriptable non-TTY run
    // without `--status` must save the row instead of exiting silently.
    let env = Env::new("add-status-default");
    let out = env.run(&[
        "add",
        "--name",
        "Standalone",
        "--price",
        "4.99",
        "--currency",
        "USD",
        "--cycle",
        "monthly",
    ]);
    assert_eq!(out.status.code(), Some(0));
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(text.contains("Added subscription:"), "silent add: {text}");

    let json: serde_json::Value =
        serde_json::from_slice(&env.run(&["list", "-j"]).stdout).expect("list json");
    assert_eq!(
        json.as_array().map(|a| a.len()),
        Some(1),
        "row was saved: {json}"
    );
    assert_eq!(json[0]["status"], "active", "default status: {json}");
}

#[test]
fn edit_prompts_for_id_field_and_value() {
    let env = Env::new("edit-prompts");
    seed(&env);
    // 1) pick the first subscription, 2) pick the "name" field by value,
    // 3) enter the new name.
    let out = env.run_interactive(&["edit"], "1\nname\nNetflix Plus\n");
    assert!(out.status.success());
    let text = env.text(&["list"]);
    assert!(text.contains("Netflix Plus"), "output: {text}");
    assert!(!text.contains("  Netflix "), "output: {text}");
}

#[test]
fn delete_prompts_for_selection_and_confirmation() {
    let env = Env::new("delete-prompts");
    seed(&env);
    let out = env.run_interactive(&["delete"], "1\ny\n");
    assert!(out.status.success());
    let text = env.text(&["list"]);
    assert!(!text.contains("Netflix"), "output: {text}");
    assert!(text.contains("Spotify"), "output: {text}");
}

#[test]
fn interactive_input_echoes_the_value_it_saved() {
    let env = Env::new("input-echo");
    seed(&env);
    let out = env.run_interactive(&["edit"], "1\nname\nNetflix Plus\n");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("✔ New name Netflix Plus"),
        "entered value was not echoed: {text}"
    );
}

#[test]
fn interactive_select_filters_long_choice_lists() {
    let env = Env::new("select-filter");
    seed(&env);
    // "dollar" matches 7 currencies (incl. TWD); the first shown is AUD.
    let out = env.run_interactive(&["edit"], "1\ncurrency\ndollar\n1\n");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("7 matching \"dollar\""),
        "filter hint missing: {text}"
    );
    assert!(
        text.contains("Australian Dollar"),
        "filtered list missing: {text}"
    );
    assert!(
        !text.contains("37 matching"),
        "full list was not narrowed: {text}"
    );

    let json = env.text(&["list", "--json"]);
    assert!(json.contains("AUD"), "selection was not saved: {json}");
}

#[test]
fn menu_exposes_more_actions_and_root_help() {
    let env = Env::new("menu-help");
    let out = env.run_interactive(&[], "help\n");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("Help — all commands"),
        "menu item missing: {text}"
    );
    assert!(
        text.contains("USAGE:"),
        "help action did not render: {text}"
    );
    assert!(
        text.contains("COMMANDS:"),
        "help action did not render: {text}"
    );
}

#[test]
fn piped_output_has_no_ansi_or_banner() {
    let env = Env::new("piped-clean");
    seed(&env);

    // A pipe is not a terminal: do not emit colors even without NO_COLOR.
    let out = env.run_custom(&["list"], "", &[("TERM", "xterm-256color")], &["NO_COLOR"]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains('\u{1b}'),
        "ANSI leaked into a pipe: {stdout:?}"
    );
    assert!(
        !stdout.starts_with("subtrack v"),
        "banner leaked into a pipe: {stdout}"
    );
    assert!(stdout.contains("Netflix"), "list content missing: {stdout}");
}

#[test]
fn force_color_emits_ansi_but_no_color_wins() {
    let env = Env::new("color-policy");
    seed(&env);

    let out = env.run_custom(
        &["list"],
        "",
        &[("FORCE_COLOR", "1"), ("TERM", "xterm-256color")],
        &["NO_COLOR"],
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains('\u{1b}'), "FORCE_COLOR did not colorize");

    let out = env.run_custom(
        &["list", "--no-color"],
        "",
        &[("FORCE_COLOR", "1"), ("TERM", "xterm-256color")],
        &["NO_COLOR"],
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains('\u{1b}'),
        "--no-color should override FORCE_COLOR"
    );
}

#[test]
fn list_respects_columns_width() {
    let env = Env::new("columns");
    seed(&env);

    let out = env.run_custom(&["list"], "", &[("COLUMNS", "60")], &[]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let widest = stdout
        .lines()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);
    assert!(widest <= 60, "table is {widest} columns wide:\n{stdout}");
    // The cycle column must stay wide enough for the common "monthly" value.
    assert!(stdout.contains("monthly"), "cycle truncated:\n{stdout}");
    assert!(!stdout.contains("month…"), "cycle truncated:\n{stdout}");
}

#[test]
fn bare_non_tty_invocation_explains_next_step() {
    let env = Env::new("bare-hint");
    let out = env.run(&[]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("subtrack --help"), "output: {stdout}");
}

#[test]
fn bare_invocation_shows_menu() {
    let env = Env::new("menu");
    seed(&env);
    // Choose "List subscriptions", then EOF exits the menu loop.
    let out = env.run_interactive(&[], "1\n");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("What would you like to do?"),
        "output: {text}"
    );
    assert!(text.contains("Netflix"), "output: {text}");
    // Cursor control belongs to real terminals only; piped answers must stay
    // byte-stable even with SUBTRACK_ASSUME_TTY forcing the interactive path.
    assert!(!text.contains("\x1b[0J"), "output: {text}");
}

#[test]
fn timeline_keeps_currencies_separate() {
    let env = Env::new("timeline-currencies");
    seed(&env);

    let out = env.run(&["timeline", "--months", "1", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let value: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("timeline json parses");
    assert!(value["currency"].is_null(), "mixed currency: {value}");
    let entries = value["currencies"].as_array().expect("currencies array");
    assert_eq!(entries.len(), 2, "expected USD and JPY: {value}");
    assert_eq!(entries[0]["currency"], "USD");
    assert_eq!(entries[0]["total"], 25.98);
    assert_eq!(entries[1]["currency"], "JPY");
    assert_eq!(entries[1]["total"], 20.83);
}

#[test]
fn stats_reports_price_ranges_per_currency() {
    let env = Env::new("stats-ranges");
    seed(&env);

    let text = env.text(&["stats"]);
    assert!(text.contains("USD: $9.99 – $15.99"), "stats: {text}");
    assert!(text.contains("JPY: ¥250"), "stats: {text}");

    let json: serde_json::Value =
        serde_json::from_slice(&env.run(&["stats", "--json"]).stdout).expect("stats json");
    assert_eq!(json["priceRanges"]["USD"]["min"], 9.99);
    assert_eq!(json["priceRanges"]["USD"]["max"], 15.99);
    assert_eq!(json["priceRanges"]["JPY"]["min"], 250);
}

#[test]
fn optimize_reports_savings_per_currency() {
    let env = Env::new("optimize-currencies");
    seed(&env);
    let out = env.run(&[
        "add",
        "--name",
        "Hulu",
        "--price",
        "1200",
        "--currency",
        "JPY",
        "--cycle",
        "monthly",
        "--billingDay",
        "12",
        "--status",
        "active",
    ]);
    assert_eq!(out.status.code(), Some(0));

    let text = env.text(&["optimize"]);
    assert!(text.contains("¥2,160/yr + $46.76/yr"), "optimize: {text}");

    // A currency-specific threshold cannot be compared across currencies.
    let out = env.run(&["optimize", "--min-savings", "10"]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("--currency"), "stderr: {stderr}");
}

#[test]
fn report_json_total_is_null_for_mixed_currencies() {
    let env = Env::new("report-currencies");
    seed(&env);

    let json: serde_json::Value =
        serde_json::from_slice(&env.run(&["report", "--json"]).stdout).expect("report json");
    assert!(json["total"].is_null(), "mixed total: {json}");
    assert!(json["byCurrency"]["USD"].as_f64().unwrap() > 0.0);
    assert!(json["byCurrency"]["JPY"].as_f64().unwrap() > 0.0);
}

#[test]
fn bare_parent_commands_default_to_list_and_forecast_runs() {
    let env = Env::new("bare-defaults");
    seed(&env);

    let config = env.text(&["config"]);
    assert!(config.contains("defaultCurrency: USD"), "config: {config}");

    let forecast = env.text(&["forecast"]);
    assert!(forecast.contains("Subscription"), "forecast: {forecast}");
    assert!(forecast.contains("Year"), "forecast: {forecast}");

    let search = env.text(&["search"]);
    assert!(
        search.contains("Provide a search query"),
        "search: {search}"
    );

    let profile = env.text(&["profile"]);
    assert!(profile.contains("No saved profiles"), "profile: {profile}");
}

#[test]
fn config_language_switches_common_ui_to_japanese() {
    let env = Env::new("language-ja");
    assert!(
        env.run(&["config", "set", "language", "ja"])
            .status
            .success()
    );

    let help = env.text(&["--help"]);
    assert!(help.contains("使い方:"), "help: {help}");
    assert!(help.contains("コマンド:"), "help: {help}");
    assert!(
        help.contains("サブスクリプションを追加"),
        "translated command description missing: {help}"
    );

    let menu = String::from_utf8_lossy(&env.run_interactive(&[], "1\n").stdout).into_owned();
    assert!(menu.contains("サブスクリプション一覧"), "menu: {menu}");
    assert!(
        menu.contains("「追加」で作成できます"),
        "menu header: {menu}"
    );

    let profile = env.text(&["profile"]);
    assert!(
        profile.contains("保存済みプロファイルがありません"),
        "status message: {profile}"
    );

    // Invalid values are rejected instead of silently changing the UI.
    let bad = env.run(&["config", "set", "language", "fr"]);
    assert_eq!(bad.status.code(), Some(1));
}

#[test]
fn config_log_level_controls_status_chatter() {
    let env = Env::new("log-level");
    let home = env.dir.join("empty-home");
    std::fs::create_dir_all(&home).unwrap();
    let extra = [("HOME", home.to_str().unwrap())];

    // Default (normal): status messages stay, progress/debug chatter is hidden.
    let normal = env.run_custom(&["usage", "refresh"], "", &extra, &[]);
    let stdout = String::from_utf8_lossy(&normal.stdout);
    assert!(
        stdout.contains("No new usage entries found"),
        "normal: {stdout}"
    );
    assert!(!stdout.contains("not found — skip"), "normal: {stdout}");

    assert!(
        env.run(&["config", "set", "logLevel", "quiet"])
            .status
            .success()
    );
    let quiet = env.text(&["profile"]);
    assert!(!quiet.contains("No saved profiles"), "quiet: {quiet}");

    assert!(
        env.run(&["config", "set", "logLevel", "verbose"])
            .status
            .success()
    );
    let verbose = env.run_custom(&["usage", "refresh"], "", &extra, &[]);
    let stdout = String::from_utf8_lossy(&verbose.stdout);
    assert!(stdout.contains("not found — skip"), "verbose: {stdout}");

    let bad = env.run(&["config", "set", "logLevel", "loud"]);
    assert_eq!(bad.status.code(), Some(1));
}

#[test]
fn config_wizard_edits_a_setting_and_persists_it() {
    let env = Env::new("config-wizard-set");
    let out = env.run_interactive(&["config"], "language\nja\nDone\n");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("Set language = ja"),
        "wizard did not save: {text}"
    );
    assert!(
        text.contains("1 setting(s) updated"),
        "wizard summary missing: {text}"
    );

    let value = env.text(&["config", "get", "language"]);
    assert!(value.contains("language: ja"), "not persisted: {value}");

    // Non-TTY stays scriptable: the bare command still prints the full list.
    let list = env.text(&["config"]);
    assert!(list.contains("defaultCurrency: USD"), "config list: {list}");
}

#[test]
fn config_wizard_keeps_current_value_on_empty_answer() {
    let env = Env::new("config-wizard-keep");
    let out = env.run_interactive(&["config"], "logLevel\n\nDone\n");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("No settings changed"),
        "empty answer changed a setting: {text}"
    );
    let value = env.text(&["config", "get", "logLevel"]);
    assert!(value.contains("logLevel: normal"), "value changed: {value}");
}

#[test]
fn config_wizard_reprompts_on_invalid_value_and_can_cancel() {
    let env = Env::new("config-wizard-validate");
    let out = env.run_interactive(
        &["config"],
        "tableMinWidth\n999\nabc\n60\nmonthlyBudget\n-\nmonthlyBudget\n\nDone\n",
    );
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("tableMinWidth must be an integer between 20 and 200"),
        "validation error missing: {text}"
    );
    assert!(text.contains("- = back"), "cancel hint missing: {text}");
    assert!(
        text.contains("1 setting(s) updated"),
        "cancelled edit was saved: {text}"
    );
    let value = env.text(&["config", "get", "tableMinWidth"]);
    assert!(
        value.contains("tableMinWidth: 60"),
        "not persisted: {value}"
    );
    assert!(
        !text.contains("monthlyBudget must be a non-negative number"),
        "empty answer was validated: {text}"
    );
    let budget = env.text(&["config", "get", "monthlyBudget"]);
    assert!(
        budget.contains("monthlyBudget: (not set)"),
        "cancelled edit: {budget}"
    );
}

#[test]
fn config_wizard_is_reachable_from_menu_and_applies_language_immediately() {
    let env = Env::new("config-wizard-menu");
    let out = env.run_interactive(&[], "Configuration\nlanguage\nja\nDone\nexit\n");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("Select a setting to change"),
        "wizard did not open: {text}"
    );
    assert!(
        text.contains("変更する設定を選択"),
        "menu did not pick up the new language: {text}"
    );
    let value = env.text(&["config", "get", "language"]);
    assert!(value.contains("language: ja"), "not persisted: {value}");
}

#[test]
fn config_wizard_does_not_offer_secret_keys() {
    let env = Env::new("config-wizard-secrets");
    let out = env.run_interactive(&["config"], "Done\n");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(!text.contains("slackWebhook"), "secret key listed: {text}");
    assert!(!text.contains("webhookUrl"), "secret key listed: {text}");
}

fn expense_json(env: &Env, args: &[&str]) -> serde_json::Value {
    let out = env.run(args);
    assert!(out.status.success(), "command failed: {args:?}");
    let body = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(&body).unwrap_or_else(|e| panic!("invalid JSON ({e}): {body}"))
}

#[test]
fn expense_add_list_total_and_item_edits() {
    let env = Env::new("expense-crud");
    let add = |extra: &[&str]| {
        let mut args = vec![
            "expense",
            "add",
            "--date",
            TODAY,
            "--amount",
            "1200",
            "--currency",
            "jpy",
            "--category",
            "Food",
            "--method",
            "Cash",
            "--note",
            "ramen",
        ];
        args.extend_from_slice(extra);
        assert!(env.run(&args).status.success(), "add failed: {args:?}");
    };
    add(&[]);
    add(&[
        "--amount",
        "300",
        "--category",
        "transport",
        "--method",
        "card",
        "--note",
        "train",
    ]);

    let listed = expense_json(&env, &["expense", "list", "--json"]);
    let days = listed.as_array().unwrap();
    assert_eq!(days.len(), 1);
    assert_eq!(days[0]["date"], TODAY);
    let items = days[0]["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["category"], "food");
    assert_eq!(items[0]["currency"], "JPY");
    assert_eq!(days[0]["totals"][0]["amount"], 1500.0);

    let total = expense_json(&env, &["expense", "total", "--json", "--month", "2026-10"]);
    assert_eq!(total["period"]["from"], "2026-10-01");
    assert_eq!(total["period"]["to"], "2026-10-31");
    assert_eq!(total["totals"][0]["amount"], 1500.0);
    assert_eq!(total["groups"][0]["key"], "food");

    assert!(
        env.run(&[
            "expense",
            "item",
            "edit",
            TODAY,
            "1",
            "--amount",
            "1500",
            "--clear-note",
        ])
        .status
        .success()
    );
    let edited = expense_json(&env, &["expense", "list", "--json"]);
    assert_eq!(edited[0]["items"][0]["amount"], 1500.0);
    assert!(edited[0]["items"][0]["note"].is_null());

    assert!(
        env.run(&["expense", "item", "delete", TODAY, "2"])
            .status
            .success()
    );
    assert_eq!(
        expense_json(&env, &["expense", "list", "--json"])[0]["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    assert!(
        env.run(&["expense", "delete", TODAY, "--force"])
            .status
            .success()
    );
    assert_eq!(
        expense_json(&env, &["expense", "list", "--json"]),
        serde_json::json!([])
    );
}

#[test]
fn expense_interactive_add_collects_multiple_items() {
    let env = Env::new("expense-interactive");
    let out = env.run_interactive(
        &["expense", "add"],
        "1200\n\n\n\n\ny\n300\n\ntransport\n1\n\n\n",
    );
    assert!(out.status.success());
    let listed = expense_json(&env, &["expense", "list", "--json"]);
    let items = listed[0]["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["category"], "other");
    assert_eq!(items[1]["category"], "transport");
    assert_eq!(items[1]["method"], "cash");
}

#[test]
fn tag_merge_missing_source_is_rejected() {
    let env = Env::new("tag-merge-missing");
    let out = env.run(&["tag", "merge", "ghost", "ghost"]);
    assert_eq!(out.status.code(), Some(1));
    let message = String::from_utf8_lossy(&out.stderr);
    assert!(message.contains("not found"), "output: {message}");
}

#[test]
fn maintenance_rekey_kdf_migrates_legacy_data_and_keeps_backups_readable() {
    let env = Env::new("rekey-kdf");
    legacy_passphrase_dir(&env);

    let first = passphrase_add_args("Netflix");
    assert!(
        env.run_passphrase(&first).status.success(),
        "first add failed"
    );
    assert!(
        env.run_passphrase(&["config", "set", "language", "ja"])
            .status
            .success(),
        "config write failed"
    );
    assert_eq!(
        std::fs::read_to_string(env.dir.join(".key.kdf"))
            .unwrap()
            .trim(),
        "1"
    );

    let out = env.run_passphrase(&["maintenance", "rekey-kdf", "--json"]);
    assert!(
        out.status.success(),
        "rekey failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let body = String::from_utf8_lossy(&out.stdout);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap_or_else(|e| {
        panic!("invalid rekey JSON ({e}): {body}");
    });
    assert_eq!(json["from"], "legacy");
    assert_eq!(json["to"], "balanced");
    assert_eq!(json["databaseRekeyed"], true);
    assert_eq!(json["configRekeyed"], true);
    assert_eq!(
        std::fs::read_to_string(env.dir.join(".key.kdf"))
            .unwrap()
            .trim(),
        "3"
    );

    let listed = env.run_passphrase(&["list", "--json"]);
    assert!(listed.status.success());
    let listed = String::from_utf8_lossy(&listed.stdout);
    assert!(
        listed.contains("Netflix"),
        "database unreadable after rekey: {listed}"
    );
    let config = env.run_passphrase(&["config", "get", "language"]);
    let config = String::from_utf8_lossy(&config.stdout);
    assert!(
        config.contains("ja"),
        "config unreadable after rekey: {config}"
    );

    let backup = std::fs::read_dir(env.dir.join("backups"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .contains("before_rekey-kdf")
                && p.to_string_lossy().ends_with(".db.enc")
        })
        .expect("safety backup missing");
    let again = env.run_passphrase(&["maintenance", "rekey-kdf", "--json"]);
    let again = String::from_utf8_lossy(&again.stdout);
    let again: serde_json::Value = serde_json::from_str(&again).unwrap();
    assert_eq!(again["from"], "balanced", "rekey not idempotent: {again}");

    let second = passphrase_add_args("Spotify");
    assert!(env.run_passphrase(&second).status.success());
    let backup_arg = backup.to_string_lossy().into_owned();
    let restored = env.run_passphrase(&["restore", &backup_arg, "--force"]);
    let restore_output = format!(
        "{} {}",
        String::from_utf8_lossy(&restored.stdout),
        String::from_utf8_lossy(&restored.stderr)
    );
    assert!(
        restored.status.success(),
        "legacy-format backup not restorable: {restore_output}"
    );
    let listed = env.run_passphrase(&["list", "--json"]);
    let listed = String::from_utf8_lossy(&listed.stdout);
    assert!(
        listed.contains("Netflix"),
        "restored data missing: {listed}"
    );
    assert!(
        !listed.contains("Spotify"),
        "restore did not roll back: {listed}"
    );
}

#[test]
fn maintenance_rekey_kdf_migrates_fast_install_and_keeps_backups_readable() {
    let env = Env::new("rekey-kdf-fast");
    fast_passphrase_dir(&env);
    let first = passphrase_add_args("Netflix");
    assert!(
        env.run_passphrase(&first).status.success(),
        "first add failed"
    );

    let out = env.run_passphrase(&["maintenance", "rekey-kdf", "--json"]);
    assert!(
        out.status.success(),
        "rekey failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let body = String::from_utf8_lossy(&out.stdout);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap_or_else(|e| {
        panic!("invalid rekey JSON ({e}): {body}");
    });
    assert_eq!(json["from"], "fast");
    assert_eq!(json["to"], "balanced");
    assert_eq!(
        std::fs::read_to_string(env.dir.join(".key.kdf"))
            .unwrap()
            .trim(),
        "3"
    );
    let listed = env.run_passphrase(&["list", "--json"]);
    let listed = String::from_utf8_lossy(&listed.stdout);
    assert!(
        listed.contains("Netflix"),
        "database unreadable after rekey: {listed}"
    );

    let backup = std::fs::read_dir(env.dir.join("backups"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .contains("before_rekey-kdf")
                && p.to_string_lossy().ends_with(".db.enc")
        })
        .expect("safety backup missing");
    let second = passphrase_add_args("Spotify");
    assert!(env.run_passphrase(&second).status.success());
    let backup_arg = backup.to_string_lossy().into_owned();
    let restored = env.run_passphrase(&["restore", &backup_arg, "--force"]);
    assert!(
        restored.status.success(),
        "fast-format backup not restorable: {}",
        String::from_utf8_lossy(&restored.stderr)
    );
    let listed_out = env.run_passphrase(&["list", "--json"]);
    let listed = String::from_utf8_lossy(&listed_out.stdout);
    assert!(
        listed.contains("Netflix"),
        "restored data missing: {listed}"
    );
    assert!(
        !listed.contains("Spotify"),
        "restore did not roll back: {listed}"
    );
}

#[test]
fn first_interactive_add_offers_and_applies_rekey() {
    let env = Env::new("rekey-prompt");
    legacy_passphrase_dir(&env);

    let args = passphrase_add_args("Netflix");
    // One answer for the rekey prompt, then the optional add wizard fields.
    let input = "\n".repeat(12);
    let out = env.run_custom(
        &args,
        &input,
        &[
            ("SUBSC_CLI_DB_PASSPHRASE", "rekey-passphrase"),
            ("SUBTRACK_ASSUME_TTY", "1"),
        ],
        &[],
    );
    assert!(
        out.status.success(),
        "add failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        text.contains("older KDF work factor"),
        "no rekey warning: {text}"
    );
    assert!(
        text.contains("Switch to the faster KDF now"),
        "no rekey prompt: {text}"
    );
    assert!(
        text.contains("KDF rekey complete"),
        "rekey not applied: {text}"
    );
    assert_eq!(
        std::fs::read_to_string(env.dir.join(".key.kdf"))
            .unwrap()
            .trim(),
        "3"
    );
    let listed = env.run_passphrase(&["list", "--json"]);
    let listed = String::from_utf8_lossy(&listed.stdout);
    assert!(listed.contains("Netflix"), "add did not complete: {listed}");
}

#[test]
fn expense_add_rejects_unknown_currency() {
    let env = Env::new("expense-currency");
    let out = env.run(&[
        "expense",
        "add",
        "--date",
        TODAY,
        "--amount",
        "1",
        "--currency",
        "XXX",
        "--category",
        "food",
        "--method",
        "card",
    ]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        expense_json(&env, &["expense", "list", "--json"]),
        serde_json::json!([])
    );
}

#[test]
fn expense_rejects_negative_amount_as_value_error() {
    let env = Env::new("expense-negative-amount");
    let out = env.run(&[
        "expense",
        "add",
        "--date",
        TODAY,
        "--amount",
        "-5",
        "--currency",
        "USD",
        "--category",
        "food",
        "--method",
        "card",
    ]);
    assert_eq!(out.status.code(), Some(1));
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        text.contains("Amount must be greater than 0"),
        "negative amount: {text}"
    );
    assert!(!text.contains("Unknown option"), "negative amount: {text}");
}

#[test]
fn expense_item_edit_without_changes_fails() {
    let env = Env::new("expense-noop-edit");
    let add = env.run(&[
        "expense",
        "add",
        "--date",
        TODAY,
        "--amount",
        "1200",
        "--currency",
        "JPY",
        "--category",
        "food",
        "--method",
        "cash",
    ]);
    assert!(add.status.success(), "add failed");

    let no_flags = env.run(&["expense", "item", "edit", TODAY, "1"]);
    assert_eq!(no_flags.status.code(), Some(1));
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&no_flags.stdout),
        String::from_utf8_lossy(&no_flags.stderr)
    );
    assert!(text.contains("No changes found"), "no-op edit: {text}");

    let same_value = env.run(&["expense", "item", "edit", TODAY, "1", "--amount", "1200"]);
    assert_eq!(same_value.status.code(), Some(1));
}

#[test]
fn profile_and_template_delete_accept_force() {
    let env = Env::new("force-delete");

    assert!(env.run(&["profile", "save", "demo"]).status.success());
    let profile = env.run(&["profile", "delete", "demo", "--force"]);
    assert_eq!(profile.status.code(), Some(0));
    assert!(!env.text(&["profile", "list"]).contains("demo"));

    let template = env.run(&[
        "template",
        "add",
        "tmp",
        "--price",
        "10",
        "--currency",
        "USD",
        "--cycle",
        "monthly",
    ]);
    assert!(template.status.success(), "template add failed");
    let deleted = env.run(&["template", "delete", "tmp", "--force"]);
    assert_eq!(deleted.status.code(), Some(0));
    assert!(!env.text(&["template", "list"]).contains("tmp"));
}

#[test]
fn audit_prune_abort_exits_nonzero() {
    let env = Env::new("audit-prune-abort");
    seed(&env);
    // The audit log uses wall-clock timestamps; give `days=0` a strictly
    // older entry without touching production time.
    std::thread::sleep(std::time::Duration::from_secs(1));

    let out = env.run(&["audit", "prune", "--days", "0"]);
    assert_eq!(out.status.code(), Some(1));
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        text.contains("Use --force to proceed"),
        "audit prune: {text}"
    );
}

#[test]
fn backup_twice_in_same_second_uses_unique_names() {
    let env = Env::new("backup-twice");
    let dir = env.dir.join("backups");
    let dir_s = dir.to_string_lossy();
    let args = vec!["backup", dir_s.as_ref()];

    assert!(env.run(&args).status.success());
    assert!(env.run(&args).status.success());

    let names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".db.gz"))
        .collect();
    assert_eq!(names.len(), 2, "backups: {names:?}");
    let unique: std::collections::BTreeSet<&String> = names.iter().collect();
    assert_eq!(
        unique.len(),
        2,
        "backups must have distinct names: {names:?}"
    );
}
