use std::collections::BTreeSet;

// ── ANSI state tracking ───────────────────────────────────

fn is_fg(code: i64) -> bool {
    matches!(code, 30..=38 | 90..=98)
}
fn is_bg(code: i64) -> bool {
    matches!(code, 40..=48 | 100..=108)
}

fn attr_off(code: i64) -> Option<i64> {
    match code {
        1 | 2 => Some(22),
        3 => Some(23),
        4 => Some(24),
        7 => Some(27),
        9 => Some(29),
        _ => None,
    }
}

pub(super) fn apply_sgr(state: &mut BTreeSet<i64>, code: i64) {
    if code == 0 {
        state.clear();
        return;
    }
    if code == 39 {
        state.retain(|c| !is_fg(*c));
        return;
    }
    if code == 49 {
        state.retain(|c| !is_bg(*c));
        return;
    }
    if let Some(off) = attr_off(code) {
        state.remove(&off);
        return;
    }
    if is_fg(code) {
        state.retain(|c| !is_fg(*c));
        state.insert(code);
        return;
    }
    if is_bg(code) {
        state.retain(|c| !is_bg(*c));
        state.insert(code);
        return;
    }
    if code != 0 {
        state.insert(code);
    }
}

/// Extract SGR (CSI ... m) code sequences in order of appearance.
fn sgr_codes(str: &str) -> Vec<i64> {
    let mut out = Vec::new();
    let bytes = str.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            let mut j = i + 2;
            let mut body = String::new();
            while j < bytes.len() {
                if bytes[j] == b'm' {
                    break;
                }
                body.push(bytes[j] as char);
                j += 1
            }
            if j < bytes.len() && bytes[j] == b'm' {
                let code = if body.is_empty() {
                    0
                } else {
                    body.split(';')
                        .next()
                        .unwrap_or("0")
                        .parse::<i64>()
                        .unwrap_or(0)
                };
                out.push(code);
                i = j + 1;
                continue;
            }
        }
        i += 1
    }
    out
}

/// Read the SGR state at the end of a string.
fn read_sgr(str: &str) -> BTreeSet<i64> {
    let mut state = BTreeSet::new();
    for code in sgr_codes(str) {
        apply_sgr(&mut state, code)
    }
    state
}

/// Open codes for a state (attributes → bg → fg).
fn open_codes(state: &BTreeSet<i64>) -> String {
    let mut out = String::new();
    let mut fg: Vec<i64> = Vec::new();
    let mut bg: Vec<i64> = Vec::new();
    for c in state {
        if is_fg(*c) {
            fg.push(*c)
        } else if is_bg(*c) {
            bg.push(*c)
        } else {
            out.push_str(&format!("\u{1b}[{}m", c))
        }
    }
    for c in bg {
        out.push_str(&format!("\u{1b}[{}m", c))
    }
    for c in fg {
        out.push_str(&format!("\u{1b}[{}m", c))
    }
    out
}

/// Close codes for a state (attributes → bg → fg).
pub(super) fn close_codes(state: &BTreeSet<i64>) -> String {
    let mut out = String::new();
    for c in state {
        if let Some(off) = attr_off(*c) {
            out.push_str(&format!("\u{1b}[{}m", off))
        }
    }
    if state.iter().any(|c| is_bg(*c)) {
        out.push_str("\u{1b}[49m")
    }
    if state.iter().any(|c| is_fg(*c)) {
        out.push_str("\u{1b}[39m")
    }
    out
}

/// Keep ANSI color state consistent across wrapped lines.
pub(super) fn colorize_lines(lines: &[String]) -> Vec<String> {
    let mut state = BTreeSet::new();
    lines
        .iter()
        .map(|line| {
            let prefixed = format!("{}{}", open_codes(&state), line);
            state = read_sgr(&prefixed);
            format!("{}{}", prefixed, close_codes(&state))
        })
        .collect()
}
