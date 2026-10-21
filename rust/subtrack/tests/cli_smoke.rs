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
