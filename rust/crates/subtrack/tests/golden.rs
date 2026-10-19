//! Golden-fixture parity harness.
//!
//! Replays every case in `rust/fixtures/cases.jsonl` against the built
//! `subtrack` binary and byte-compares stdout / stderr / exit codes (plus
//! produced files) with the goldens frozen from the TypeScript reference
//! implementation (tag `last-typescript`).
//!
//! Run: `cargo test` — the whole suite is a single test function so that
//! seed cases can share one database/working dir (the seed goldens are a
//! sequential chain built against accumulating state).

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use flate2::read::GzDecoder;
use regex::Regex;
use serde::Deserialize;

// ── Locations & injected environment ─────────────────────────────

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures");
const BIN: &str = env!("CARGO_BIN_EXE_subtrack");
const VERSION: &str = env!("CARGO_PKG_VERSION");
const CAPTURE_TODAY: &str = "2026-10-08";

/// NDJSON stream the MCP probe case feeds to the server on stdin
/// (port of MCP_STDIN from the capture driver).
const MCP_STDIN: &str = concat!(
    "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{},\"clientInfo\":{\"name\":\"capture\",\"version\":\"1\"}}}\n",
    "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
    "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n",
    "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{\"name\":\"list_subscriptions\",\"arguments\":{}}}\n",
    "{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"tools/call\",\"params\":{\"name\":\"get_subscription\",\"arguments\":{\"id\":1}}}\n",
    "{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"shutdown\"}\n",
);

// ── Case model (rust/fixtures/cases.jsonl) ───────────────────────

#[derive(Deserialize)]
struct Case {
    id: String,
    group: String,
    steps: Vec<Vec<String>>,
    db: String,
    #[serde(default)]
    stdin: bool,
    #[serde(default)]
    binary: bool,
    exits: Vec<i32>,
}

// ── Filesystem helpers ───────────────────────────────────────────

fn copy_dir_all(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let target = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir_all(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Recursive (rel-path -> bytes) map of a directory, deterministically sorted.
fn snapshot(dir: &Path) -> HashMap<String, Vec<u8>> {
    let mut out = HashMap::new();
    if !dir.exists() {
        return out;
    }
    fn walk(dir: &Path, base: &Path, out: &mut HashMap<String, Vec<u8>>) {
        let mut entries: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        entries.sort_by_key(|a| a.file_name());
        for e in entries {
            let rel = base.join(e.file_name()).display().to_string();
            if e.file_type().unwrap().is_dir() {
                walk(&e.path(), Path::new(&rel), out);
            } else {
                out.insert(rel, fs::read(e.path()).unwrap());
            }
        }
    }
    walk(dir, Path::new(""), &mut out);
    out
}

/// Deterministic, minimal env: nothing inherited but PATH (harness contract).
fn case_env(db_dir: &Path, home: &Path) -> HashMap<String, String> {
    [
        (
            "PATH",
            std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into()),
        ),
        ("HOME", home.display().to_string()),
        ("TZ", "UTC".into()),
        ("LC_ALL", "C.utf8".into()),
        ("LANG", "C.utf8".into()),
        ("TERM", "xterm-256color".into()),
        ("SUBSC_CLI_DB_DIR", db_dir.display().to_string()),
        ("SUBTRACK_TODAY", CAPTURE_TODAY.into()),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect()
}

/// Copy a fixture db dir (populated/legacy/…) into a case's db dir.
fn copy_db(db: &str, db_dir: &Path) {
    let src = Path::new(FIXTURES).join("dbs").join(db);
    if src.is_dir() {
        copy_dir_all(&src, db_dir);
    } else {
        // `dbs/empty` is only a marker: the case starts with a fresh DB.
        fs::create_dir_all(db_dir).unwrap();
    }
}

fn copy_inputs(cwd: &Path) {
    copy_dir_all(&Path::new(FIXTURES).join("inputs"), cwd);
}

// ── Normalization (mirror of the capture driver) ─────────────────

fn normalize(text: &[u8], case_root: &Path) -> Vec<u8> {
    let mut s = String::from_utf8_lossy(text).into_owned();
    for (from, to) in [
        (case_root.display().to_string(), "{WORK}"),
        (FIXTURES.to_string(), "{FIXTURES}"),
        (
            Path::new(FIXTURES).join("../..").display().to_string(),
            "{REPO}",
        ),
    ] {
        s = s.replace(&from, to);
    }
    // Version, boundary-safe: not adjacent to a digit or `.` (regex look-around
    // is unsupported, so check the boundaries by hand).
    let version_re = Regex::new(&regex::escape(VERSION)).unwrap();
    let boundary_ok = |b: Option<u8>| b.map(|c| !c.is_ascii_digit() && c != b'.').unwrap_or(true);
    let mut out = String::with_capacity(s.len());
    let mut last = 0;
    for m in version_re.find_iter(&s) {
        let before = m.start().checked_sub(1).map(|i| s.as_bytes()[i]);
        let after = s.as_bytes().get(m.end()).copied();
        if !boundary_ok(before) || !boundary_ok(after) {
            continue;
        }
        out.push_str(&s[last..m.start()]);
        out.push_str("{VERSION}");
        last = m.end();
    }
    out.push_str(&s[last..]);
    let mut s = out;
    let datetime_re =
        Regex::new(r"\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?")
            .unwrap();
    s = datetime_re.replace_all(&s, "{DATETIME}").into_owned();
    let date_re = Regex::new(r"\d{4}-\d{2}-\d{2}").unwrap();
    s = date_re.replace_all(&s, "{DATE}").into_owned();
    let stamp_re = Regex::new(r"\d{8}_\d{6}").unwrap();
    s = stamp_re.replace_all(&s, "{STAMP}").into_owned();
    s.into_bytes()
}

// ── Case execution ───────────────────────────────────────────────

fn run_case(
    case: &Case,
    step_base: usize,
    cwd: &Path,
    env: &HashMap<String, String>,
) -> (Vec<i32>, Vec<u8>, Vec<u8>) {
    let separator = case.group == "seed" || case.steps.len() > 1;
    let mut exits = Vec::new();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    for (i, argv) in case.steps.iter().enumerate() {
        let mut cmd = Command::new(BIN);
        cmd.args(argv)
            .current_dir(cwd)
            .envs(env)
            .stdin(if case.stdin {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn().expect("spawn subtrack binary");
        if case.stdin {
            use std::io::Write;
            child
                .stdin
                .take()
                .unwrap()
                .write_all(MCP_STDIN.as_bytes())
                .unwrap();
        }
        let out = child.wait_with_output().unwrap();
        if separator {
            let n = step_base + i;
            stdout.extend_from_slice(format!("<<<step {n}>>>\n").as_bytes());
            stderr.extend_from_slice(format!("<<<step {n}>>>\n").as_bytes());
        }
        stdout.extend_from_slice(&out.stdout);
        stderr.extend_from_slice(&out.stderr);
        exits.push(out.status.code().unwrap_or(-1));
    }
    (exits, stdout, stderr)
}

// ── File output comparison (.files/ goldens) ─────────────────────

fn file_mode(name: &str) -> &'static str {
    if name.ends_with(".gz") {
        "gzip"
    } else if name.ends_with(".xlsx") || name.ends_with(".sha256") || name.ends_with(".db") {
        "skip"
    } else {
        "bytes"
    }
}

fn gunzip(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    GzDecoder::new(data).read_to_end(&mut out).unwrap();
    out
}

/// Deterministic logical snapshot of a SQLite db (schema + every row).
///
/// Used for `.gz` backups: the file bytes of a node:sqlite vs rusqlite db
/// legitimately differ in header bookkeeping (schema cookie, version-valid-for)
/// even when the data is identical, so we compare content, not bytes.
fn db_snapshot(path: &Path) -> String {
    use rusqlite::types::Value;
    let conn = rusqlite::Connection::open(path).expect("open backup db");
    let mut out = String::new();
    for row in conn
        .prepare("SELECT type, name, sql FROM sqlite_master ORDER BY type, name")
        .unwrap()
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })
        .unwrap()
        .map(|r| r.unwrap())
    {
        out.push_str(&format!(
            "{}|{}|{}\n",
            row.0,
            row.1,
            row.2.unwrap_or_default()
        ));
    }
    let names: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    for name in names {
        let sql = format!("SELECT * FROM \"{name}\" ORDER BY rowid");
        let mut stmt = conn.prepare(&sql).unwrap();
        let cols = stmt.column_count();
        let mut rows = stmt.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            let vals: Vec<Value> = (0..cols).map(|i| row.get(i).unwrap()).collect();
            out.push_str(&format!("{name}|{vals:?}\n"));
        }
    }
    out
}

/// Like `db_snapshot`, but drops volatile timestamp columns
/// (`created_at` / `changed_at`) so two dbs can be compared by content.
///
/// Used to assert that the seed chain reconstructs the committed `populated`
/// fixture — a property the stdout/file goldens never exercise, because the
/// seed cases are write-only (they never read the rows back).
fn db_table_snapshot(path: &Path) -> String {
    use rusqlite::types::Value;
    let conn = rusqlite::Connection::open(path).expect("open db");
    let mut out = String::new();
    for (ty, name, sql) in conn
        .prepare("SELECT type, name, sql FROM sqlite_master ORDER BY type, name")
        .unwrap()
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })
        .unwrap()
        .map(|r| r.unwrap())
    {
        // node:sqlite (TS) and rusqlite emit identical schema semantics but
        // the stored CREATE-statement text differs in indentation, so compare
        // it whitespace-normalized.
        let sql = sql.unwrap_or_default();
        let sql_norm = sql.split_whitespace().collect::<Vec<_>>().join(" ");
        out.push_str(&format!("{ty}|{name}|{sql_norm}\n"));
    }
    const VOLATILE: &[&str] = &["created_at", "changed_at"];
    let names: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    for name in names {
        let cols: Vec<String> = conn
            .prepare(&format!("PRAGMA table_info(\"{name}\")"))
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        let keep: Vec<String> = cols
            .iter()
            .filter(|c| !VOLATILE.contains(&c.as_str()))
            .map(|c| format!("\"{c}\""))
            .collect();
        let sql = format!("SELECT {} FROM \"{name}\" ORDER BY rowid", keep.join(","));
        let mut stmt = conn.prepare(&sql).unwrap();
        let cols = stmt.column_count();
        let mut rows = stmt.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            let vals: Vec<Value> = (0..cols).map(|i| row.get(i).unwrap()).collect();
            out.push_str(&format!("{name}|{vals:?}\n"));
        }
    }
    out
}

/// Multiset difference of two line-oriented snapshots: reports every line
/// whose occurrence count differs (robust to row ordering).
fn snapshot_diff(seed: &str, populated: &str) -> Vec<String> {
    let mut ca: std::collections::BTreeMap<&str, i64> = std::collections::BTreeMap::new();
    for l in seed.lines() {
        *ca.entry(l).or_default() += 1;
    }
    let mut cb: std::collections::BTreeMap<&str, i64> = std::collections::BTreeMap::new();
    for l in populated.lines() {
        *cb.entry(l).or_default() += 1;
    }
    let mut keys: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    keys.extend(ca.keys());
    keys.extend(cb.keys());
    let mut out = Vec::new();
    for k in keys {
        let (x, y) = (
            ca.get(k).copied().unwrap_or(0),
            cb.get(k).copied().unwrap_or(0),
        );
        if x != y {
            out.push(format!("  [seed:{x} populated:{y}] {k}"));
        }
    }
    out
}

/// Run `subtrack backup <dest>` against a db dir and return the content
/// snapshot of the resulting plaintext db (the `.db.gz` is gunzipped first).
fn backup_snapshot(env: &HashMap<String, String>, cwd: &Path, dest: &Path) -> String {
    fs::create_dir_all(dest).unwrap();
    let out = Command::new(BIN)
        .args(["backup", &dest.display().to_string()])
        .current_dir(cwd)
        .envs(env)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .expect("run backup");
    assert!(
        out.status.success(),
        "backup failed ({}): {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let gz = fs::read_dir(dest)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.to_string_lossy().ends_with(".db.gz"))
        .expect("backup produced a .db.gz");
    let plain = dest.join("__parity.db");
    fs::write(&plain, gunzip(&fs::read(&gz).unwrap())).unwrap();
    let snap = db_table_snapshot(&plain);
    let _ = fs::remove_file(&plain);
    snap
}

/// Compare files produced in the case cwd against the committed `<id>.files/`
/// goldens (gzip = compare after gunzip; sha256/xlsx/db = skip).
fn check_files(case: &Case, cwd: &Path, case_root: &Path) -> Vec<String> {
    let baseline = snapshot(&Path::new(FIXTURES).join("inputs"));
    let produced = snapshot(cwd);
    let golden_dir = Path::new(FIXTURES)
        .join("goldens")
        .join(&case.group)
        .join(format!("{}.files", case.id));

    let stamp_re = Regex::new(r"\d{8}_\d{6}").unwrap();
    let mut problems = Vec::new();
    for (rel, bytes) in &produced {
        // Files copied from inputs/ are the baseline, not output.
        if baseline.get(rel) == Some(bytes) {
            continue;
        }
        let rel_norm = stamp_re.replace_all(rel, "{STAMP}").into_owned();
        let mode = file_mode(rel);
        if mode == "skip" {
            continue;
        }
        let golden_path = golden_dir.join(&rel_norm);
        let golden = match fs::read(&golden_path) {
            Ok(g) => g,
            Err(_) => {
                problems.push(format!(
                    "unexpected output file: {rel} (no golden {golden_path:?})"
                ));
                continue;
            }
        };
        let equal = if mode == "gzip" {
            let golden_db = case_root.join("__compare_golden.db");
            let produced_db = case_root.join("__compare_produced.db");
            fs::write(&golden_db, gunzip(&golden)).unwrap();
            fs::write(&produced_db, gunzip(bytes)).unwrap();
            let eq = db_snapshot(&golden_db) == db_snapshot(&produced_db);
            let _ = fs::remove_file(&golden_db);
            let _ = fs::remove_file(&produced_db);
            eq
        } else if String::from_utf8(bytes.clone()).is_ok() {
            normalize(&golden, case_root) == normalize(bytes, case_root)
        } else {
            golden == *bytes
        };
        if !equal {
            problems.push(format!("file {rel}: content differs"));
        }
    }
    problems
}

// ── Test ─────────────────────────────────────────────────────────

#[test]
fn goldens_match_ts_reference() {
    // One temp root for the whole run: seed cases share db/home/cwd so the
    // chain state accumulates exactly like the reference capture did.
    let root = std::env::temp_dir().join(format!("subtrack-golden-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());

    let cases: Vec<Case> = fs::read_to_string(Path::new(FIXTURES).join("cases.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();

    // Shared root for the seed chain (accumulates one db across all 30 cases).
    let seed_root = root.join("work").join("seed-shared");
    for d in ["db", "home", "cwd"] {
        fs::create_dir_all(seed_root.join(d)).unwrap();
    }
    copy_inputs(&seed_root.join("cwd"));
    let seed_env = case_env(&seed_root.join("db"), &seed_root.join("home"));

    let goldens_dir = Path::new(FIXTURES).join("goldens");
    let mut failures = 0usize;
    let mut seed_seq = 0usize;
    for case in &cases {
        let (case_root, env, step_base) = if case.group == "seed" {
            seed_seq += 1;
            (seed_root.clone(), seed_env.clone(), seed_seq)
        } else {
            let case_root = root.join("work").join(&case.group).join(&case.id);
            fs::create_dir_all(case_root.join("cwd")).unwrap();
            fs::create_dir_all(case_root.join("home")).unwrap();
            copy_db(&case.db, &case_root.join("db"));
            copy_inputs(&case_root.join("cwd"));
            let env = case_env(&case_root.join("db"), &case_root.join("home"));
            (case_root, env, 1)
        };

        let (exits, stdout, stderr) = run_case(case, step_base, &case_root.join("cwd"), &env);
        let want_exits = &case.exits;
        let (want_out, want_err) = (
            fs::read(
                goldens_dir
                    .join(&case.group)
                    .join(format!("{}.stdout", case.id)),
            )
            .unwrap(),
            fs::read(
                goldens_dir
                    .join(&case.group)
                    .join(format!("{}.stderr", case.id)),
            )
            .unwrap(),
        );
        let got_out = if case.binary {
            stdout
        } else {
            normalize(&stdout, &case_root)
        };
        let got_err = normalize(&stderr, &case_root);

        let mut problems = Vec::new();
        if exits != *want_exits {
            problems.push(format!("exit {exits:?} != want {want_exits:?}"));
        }
        if got_out != want_out {
            problems.push(format!(
                "stdout differs ({}B golden vs {}B got)",
                want_out.len(),
                got_out.len()
            ));
        }
        if got_err != want_err {
            problems.push(format!(
                "stderr differs ({}B golden vs {}B got)",
                want_err.len(),
                got_err.len()
            ));
        }
        problems.extend(check_files(case, &case_root.join("cwd"), &case_root));

        if problems.is_empty() {
            println!("OK   {}/{}", case.group, case.id);
        } else {
            failures += problems.len();
            for p in problems {
                println!("FAIL {}/{}: {p}", case.group, case.id);
            }
        }
    }

    // ── Cross-check: the seed chain must rebuild the committed `populated` db ──
    // The goldens above are write-only for the seed group, so a broken
    // persistence path (missing save_db / dropped column) would still pass.
    // Back up both dbs to plaintext and compare every table's rows (timestamps
    // excluded).
    let seed_snap = backup_snapshot(&seed_env, &seed_root.join("cwd"), &root.join("parity/seed"));
    let pop_root = root.join("parity/populated");
    fs::create_dir_all(pop_root.join("cwd")).unwrap();
    fs::create_dir_all(pop_root.join("home")).unwrap();
    copy_db("populated", &pop_root.join("db"));
    let pop_env = case_env(&pop_root.join("db"), &pop_root.join("home"));
    let pop_snap = backup_snapshot(
        &pop_env,
        &pop_root.join("cwd"),
        &root.join("parity/pop-backup"),
    );
    if seed_snap == pop_snap {
        println!("OK   parity: seed chain == fixtures/dbs/populated");
    } else {
        failures += 1;
        let diff = snapshot_diff(&seed_snap, &pop_snap);
        println!(
            "FAIL parity: seed chain != fixtures/dbs/populated ({} differing line(s))",
            diff.len()
        );
        for line in diff.iter().take(40) {
            println!("{line}");
        }
    }

    assert_eq!(failures, 0, "{failures} golden comparison(s) failed");
}
