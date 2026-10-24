use std::collections::BTreeSet;

use super::ansi::{apply_sgr, close_codes};

// ── string width ──────────────────────────────────────────

fn is_wide(ch: char) -> bool {
    matches!(ch as u32,
        0x1100..=0x115f | 0x2e80..=0x303e | 0x3041..=0x33ff | 0x3400..=0x4dbf
        | 0x4e00..=0x9fff | 0xac00..=0xd7a3 | 0xf900..=0xfaff | 0xfe30..=0xfe4f
        | 0xff00..=0xff60 | 0xffe0..=0xffe6)
}

/// Strip SGR (CSI ... m) sequences from a string.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            let mut j = i + 2;
            while j < bytes.len() {
                if bytes[j] == b'm' {
                    break;
                }
                j += 1
            }
            if j < bytes.len() && bytes[j] == b'm' {
                i = j + 1;
                continue;
            }
        }
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8()
    }
    out
}

/// Visible width of a string: ANSI codes stripped, wide (CJK) chars count 2.
pub fn strlen(str: &str) -> i64 {
    strip_ansi(str)
        .chars()
        .map(|ch| if is_wide(ch) { 2 } else { 1 })
        .sum()
}

pub(super) fn repeat(str: &str, times: usize) -> String {
    str.repeat(times)
}

pub(super) fn pad(str: &str, len: i64, pad_char: char, dir: &str) -> String {
    let length = strlen(str);
    if len + 1 >= length {
        let pad_len = (len - length).max(0) as usize;
        match dir {
            "right" => format!("{}{}", repeat(&pad_char.to_string(), pad_len), str),
            "center" => {
                let right = (pad_len as f64 / 2.0).ceil() as usize;
                let left = pad_len - right;
                format!(
                    "{}{}{}",
                    repeat(&pad_char.to_string(), left),
                    str,
                    repeat(&pad_char.to_string(), right)
                )
            }
            _ => format!("{}{}", str, repeat(&pad_char.to_string(), pad_len)),
        }
    } else {
        str.to_string()
    }
}
// ── truncate ──────────────────────────────────────────────

pub(super) fn truncate_width_ansi(str: &str, max_width: i64) -> (String, BTreeSet<i64>) {
    let mut out = String::new();
    let mut width = 0;
    let mut state = BTreeSet::new();
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
            if j < bytes.len()
                && bytes[j] == b'm'
                && body.chars().all(|c| c.is_ascii_digit() || c == ';')
            {
                out.push_str(&str[i..=j]);
                let code = if body.is_empty() {
                    0
                } else {
                    body.split(';')
                        .next()
                        .unwrap_or("0")
                        .parse::<i64>()
                        .unwrap_or(0)
                };
                apply_sgr(&mut state, code);
                i = j + 1;
                continue;
            }
        }
        let ch = str[i..].chars().next().unwrap();
        let w = if is_wide(ch) { 2 } else { 1 };
        if width + w > max_width {
            break;
        }
        out.push(ch);
        width += w;
        i += ch.len_utf8()
    }
    (out, state)
}

/// Truncate to a visible width, appending "…" when content is cut.
pub fn truncate(str: &str, desired_length: i64, truncate_char: &str) -> String {
    if strlen(str) <= desired_length {
        return str.to_string();
    }
    let (text, state) = truncate_width_ansi(str, desired_length - strlen(truncate_char));
    format!("{}{}{}", text, close_codes(&state), truncate_char)
}

// ── split / wrap ──────────────────────────────────────────

/// Like JS `input.split(/(\s+)/g)` — words interleaved with whitespace runs.
fn split_words(input: &str) -> Vec<String> {
    if input.is_empty() {
        return vec![String::new()];
    }
    let mut out = Vec::new();
    let mut rest = input;
    while !rest.is_empty() {
        let ws_len: usize = rest
            .chars()
            .take_while(|c| c.is_whitespace())
            .map(|c| c.len_utf8())
            .sum();
        if ws_len > 0 {
            out.push(rest[..ws_len].to_string());
            rest = &rest[ws_len..];
            continue;
        }
        let mut idx = rest.len();
        for (i, c) in rest.char_indices() {
            if c.is_whitespace() {
                idx = i;
                break;
            }
        }
        out.push(rest[..idx].to_string());
        rest = &rest[idx..]
    }
    out
}

fn wrap_on_words(max_length: i64, input: &str) -> Vec<String> {
    let split = split_words(input);
    let mut lines: Vec<String> = Vec::new();
    let mut line: Vec<String> = Vec::new();
    let mut line_length: i64 = 0;
    let mut whitespace: Option<&str> = None;
    let word_count = split.len().div_ceil(2);
    for i in 0..word_count {
        let word = split.get(i * 2).map(|s| s.as_str()).unwrap_or("");
        let mut new_length = line_length + strlen(word);
        if line_length > 0
            && let Some(ws) = whitespace
        {
            new_length += strlen(ws);
        }
        if new_length > max_length {
            if line_length != 0 {
                lines.push(line.join(""))
            }
            line = vec![word.to_string()];
            line_length = strlen(word)
        } else {
            line.push(format!("{}{}", whitespace.unwrap_or(""), word));
            line_length = new_length
        }
        whitespace = split.get(i * 2 + 1).map(|s| s.as_str())
    }
    if line_length != 0 {
        lines.push(line.join(""))
    }
    lines
}

/// Wrap on every character — used when wrapOnWordBoundary is false.
fn wrap_chars(max_length: i64, input: &str) -> Vec<String> {
    let split = split_words(input);
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for (i, piece) in split.iter().enumerate() {
        let is_word = i % 2 == 0;
        if !line.is_empty() && !is_word {
            // whitespace piece: append directly
            line.push_str(piece);
            continue;
        }
        // word piece
        if !line.is_empty() {
            // carrying whitespace from previous piece in JS: split[i-1]
            if i >= 1
                && let Some(prev) = split.get(i - 1)
            {
                line.push_str(prev)
            }
        }
        line.push_str(piece);
        while strlen(&line) > max_length {
            let over = (strlen(&line) - max_length) as usize;
            let cut_at = line.chars().count() - over;
            lines.push(line.chars().take(cut_at).collect());
            line = line.chars().skip(cut_at).collect()
        }
    }
    if !line.is_empty() {
        lines.push(line)
    }
    lines
}

/// Split on "\n", then wrap each resulting line. ANSI-aware.
pub(super) fn multi_line_wrap(max_length: i64, input: &str, on_word_boundary: bool) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for part in input.split('\n') {
        if on_word_boundary {
            out.extend(wrap_on_words(max_length, part))
        } else {
            out.extend(wrap_chars(max_length, part))
        }
    }
    out
}
