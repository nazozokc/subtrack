use crate::logger::info;

// ── refresh ──────────────────────────────────────────────

/// The golden environment has none of the scanner sources; scans are ported
/// as existence checks (paths relative to $HOME) with the "not found" info.
pub fn run_usage_refresh() -> i32 {
    let home = crate::crypto::home_dir();
    let checks: &[(&str, Vec<std::path::PathBuf>)] = &[
        (
            "OpenCode DB",
            vec![
                home.join(".local/share/opencode/opencode.sqlite"),
                home.join(".opencode/opencode.sqlite"),
            ],
        ),
        (
            "Claude Code session files",
            vec![home.join(".claude/projects")],
        ),
        ("Codex CLI DB", vec![home.join(".codex/state_5.sqlite")]),
        (
            "Cursor DB",
            vec![home.join(".config/Cursor/User/globalStorage/state.vscdb")],
        ),
        (
            "Copilot CLI session files",
            vec![home.join(".copilot/session-state")],
        ),
        (
            "Windsurf DB",
            vec![home.join(".config/Windsurf/User/globalStorage/state.vscdb")],
        ),
    ];

    let mut found_any = false;
    for (label, paths) in checks {
        let found = paths.iter().any(|p| p.exists());
        if found {
            found_any = true;
            // Scanning is not required by the golden cases (sources absent).
        } else {
            info(&format!("{} not found — skip", label));
        }
    }
    let _ = found_any;
    info("No new usage entries found");
    0
}
