//! Self-contained table renderer — port of `@subtrack/lib/table.ts`
//! plus the width/theme logic from `packages/subtrack/src/display-constants.ts`
//! (calcColumnWidths, TABLE_CHARS, themes).

use std::collections::BTreeSet;

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

fn repeat(str: &str, times: usize) -> String {
    str.repeat(times)
}

fn pad(str: &str, len: i64, pad_char: char, dir: &str) -> String {
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

fn apply_sgr(state: &mut BTreeSet<i64>, code: i64) {
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
                    body.split(';').next().unwrap_or("0").parse::<i64>().unwrap_or(0)
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
fn close_codes(state: &BTreeSet<i64>) -> String {
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
fn colorize_lines(lines: &[String]) -> Vec<String> {
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

// ── truncate ──────────────────────────────────────────────

fn truncate_width_ansi(str: &str, max_width: i64) -> (String, BTreeSet<i64>) {
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
                    body.split(';').next().unwrap_or("0").parse::<i64>().unwrap_or(0)
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
        return str.to_string()
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
    let word_count = (split.len() + 1) / 2;
    for i in 0..word_count {
        let word = split.get(i * 2).map(|s| s.as_str()).unwrap_or("");
        let mut new_length = line_length + strlen(word);
        if line_length > 0 && whitespace.is_some() {
            new_length += strlen(whitespace.unwrap())
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
        let ws = if !is_word && i >= 1 {
            Some(piece.as_str())
        } else if is_word && i + 1 < split.len() {
            // leading whitespace was consumed by splitter; JS uses split[i-1]
            None
        } else {
            None
        };
        if !line.is_empty() && !is_word {
            // whitespace piece: append directly
            line.push_str(piece);
            continue;
        }
        // word piece
        if !line.is_empty() {
            // carrying whitespace from previous piece in JS: split[i-1]
            if i >= 1 {
                if let Some(prev) = split.get(i - 1) {
                    line.push_str(prev)
                }
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
fn multi_line_wrap(max_length: i64, input: &str, on_word_boundary: bool) -> Vec<String> {
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

// ── themes / styles ───────────────────────────────────────

pub const TABLE_CHARS: &[(&str, char)] = &[
    ("top", '─'),
    ("top-mid", '┬'),
    ("top-left", '┌'),
    ("top-right", '┐'),
    ("bottom", '─'),
    ("bottom-mid", '┴'),
    ("bottom-left", '└'),
    ("bottom-right", '┘'),
    ("left", '│'),
    ("left-mid", '├'),
    ("mid", '─'),
    ("mid-mid", '┼'),
    ("right", '│'),
    ("right-mid", '┤'),
    ("middle", '│'),
];

#[derive(Clone, Default)]
pub struct Theme {
    pub border: Option<&'static str>,
    pub header: Option<&'static str>,
    pub zebra: Option<&'static str>,
    pub accent: Option<&'static str>,
    pub status_active: Option<&'static str>,
    pub status_paused: Option<&'static str>,
    pub status_cancelled: Option<&'static str>,
    pub status_archived: Option<&'static str>,
}

pub fn default_theme() -> Theme {
    Theme {
        border: Some("gray"),
        header: Some("brightBlue"),
        zebra: Some("gray"),
        accent: Some("cyan"),
        status_active: Some("green"),
        status_paused: Some("yellow"),
        status_cancelled: Some("red"),
        status_archived: Some("gray"),
    }
}

/// cli-table3 style object for the resolved theme.
pub struct TableStyle {
    pub border: [String; 2],
    pub head: [String; 2],
    pub padding_left: usize,
    pub padding_right: usize,
}

pub fn table_style(theme: &Theme) -> TableStyle {
    let border = match theme.border {
        Some(c) => [crate::ansi::fg_code(c).to_string(), "\x1b[0m".to_string()],
        None => [String::new(), String::new()],
    };
    let head = match theme.header {
        Some(c) => [
            format!("\x1b[1m{}", crate::ansi::fg_code(c)),
            "\x1b[0m".to_string(),
        ],
        None => [String::new(), String::new()],
    };
    TableStyle {
        border,
        head,
        padding_left: 1,
        padding_right: 1,
    }
}

/// Wrap table cells with the zebra background (even-indexed rows).
pub fn zebra_row(cells: &[String], zebra: Option<&'static str>) -> Vec<String> {
    match zebra {
        Some(name) => cells
            .iter()
            .map(|cell| format!("{}{}\x1b[0m", crate::ansi::bg_code(name), cell))
            .collect(),
        None => cells.to_vec(),
    }
}

/// Colorize a subscription status consistently across all views.
pub fn status_color(status: &str, theme: &Theme) -> String {
    let name = match status {
        "active" => theme.status_active,
        "paused" => theme.status_paused,
        "cancelled" => theme.status_cancelled,
        "archived" => theme.status_archived,
        _ => None,
    };
    match name {
        Some(c) => format!("{}{}\x1b[0m", crate::ansi::fg_code(c), status),
        None => status.to_string(),
    }
}

/// Section heading: `` `── ${text} ──` `` with accent color.
pub fn section_title(text: &str, theme: &Theme) -> String {
    match theme.accent {
        Some(c) => format!("\x1b[1m{}── {} ──\x1b[0m", crate::ansi::fg_code(c), text),
        None => format!("── {} ──", text),
    }
}

// ── table ─────────────────────────────────────────────────

pub struct TableBuilder {
    chars: Vec<(&'static str, char)>,
    pub style: TableStyle,
    pub col_widths: Vec<i64>,
    pub head: Vec<String>,
    pub col_aligns: Vec<String>,
    pub word_wrap: bool,
    pub wrap_on_word_boundary: bool,
    pub truncate_char: String,
    rows: Vec<Vec<String>>,
}

impl TableBuilder {
    pub fn new(style: TableStyle) -> Self {
        TableBuilder {
            chars: TABLE_CHARS.to_vec(),
            style,
            col_widths: Vec::new(),
            head: Vec::new(),
            col_aligns: Vec::new(),
            word_wrap: false,
            wrap_on_word_boundary: true,
            truncate_char: "…".to_string(),
            rows: Vec::new(),
        }
    }

    fn char(&self, key: &str) -> char {
        self.chars
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| *v)
            .unwrap_or(' ')
    }

    pub fn push(&mut self, row: Vec<String>) {
        self.rows.push(row)
    }

    pub fn to_string(&self) -> String {
        let mut all_rows: Vec<&Vec<String>> = Vec::new();
        if !self.head.is_empty() {
            all_rows.push(&self.head)
        }
        for r in &self.rows {
            all_rows.push(r)
        }
        if all_rows.is_empty() {
            return String::new()
        }
        let col_count = all_rows.iter().map(|r| r.len()).max().unwrap_or(0);

        let padding_left = self.style.padding_left as i64;
        let padding_right = self.style.padding_right as i64;

        let mut widths: Vec<i64> = Vec::new();
        for x in 0..col_count {
            if let Some(fixed) = self.col_widths.get(x) {
                widths.push(*fixed);
                continue;
            }
            let mut desired: i64 = 1;
            for row in &all_rows {
                if let Some(cell) = row.get(x) {
                    desired = desired.max(strlen(cell) + padding_left + padding_right)
                }
            }
            widths.push(desired)
        }

        // Pre-compute the wrapped, colorized lines of every cell.
        // cell_lines[row][col] = Vec of colorized lines.
        let mut cell_lines: Vec<Vec<Vec<String>>> = Vec::new();
        for row in &all_rows {
            let mut row_cells: Vec<Vec<String>> = Vec::new();
            for (x, cell) in row.iter().enumerate() {
                let content_width = widths[x] - padding_left - padding_right;
                let raw_lines = if self.word_wrap {
                    multi_line_wrap(1.max(content_width), cell, self.wrap_on_word_boundary)
                } else {
                    cell.split('\n').map(|s| s.to_string()).collect()
                };
                row_cells.push(colorize_lines(&raw_lines))
            }
            cell_lines.push(row_cells)
        }

        let heights: Vec<usize> = (0..all_rows.len())
            .map(|y| {
                (0..all_rows[y].len())
                    .map(|x| cell_lines[y][x].len())
                    .max()
                    .unwrap_or(1)
                    .max(1)
            })
            .collect();

        let mut out: Vec<String> = Vec::new();
        for y in 0..all_rows.len() {
            out.push(self.draw_border(&widths, y, false));
            for line_num in 0..heights[y] {
                out.push(self.draw_line(&cell_lines, &widths, y, line_num, col_count))
            }
        }
        out.push(self.draw_border(&widths, all_rows.len() - 1, true));
        out.join("\n")
    }

    fn draw_border(&self, widths: &[i64], row_index: usize, bottom: bool) -> String {
        let top_row = !bottom && row_index == 0;
        let line_char = if bottom { "bottom" } else if top_row { "top" } else { "mid" };
        let cols = widths.len();

        let mut out = String::new();
        for x in 0..cols {
            if bottom {
                out.push(if x == 0 {
                    self.char("bottom-left")
                } else {
                    self.char("bottom-mid")
                })
            } else if x == 0 {
                out.push(if top_row { self.char("top-left") } else { self.char("left-mid") })
            } else {
                out.push(if top_row { self.char("top-mid") } else { self.char("mid-mid") })
            }
            out.push_str(&repeat(&self.char(line_char).to_string(), widths[x].max(0) as usize))
        }
        out.push(if bottom {
            self.char("bottom-right")
        } else if top_row {
            self.char("top-right")
        } else {
            self.char("right-mid")
        });

        if !self.style.border[0].is_empty() {
            return format!("{}{}{}", self.style.border[0], out, self.style.border[1])
        }
        out
    }

    fn draw_line(
        &self,
        cell_lines: &[Vec<Vec<String>>],
        widths: &[i64],
        y: usize,
        line_num: usize,
        col_count: usize,
    ) -> String {
        let padding_left = self.style.padding_left as i64;
        let padding_right = self.style.padding_right as i64;
        let is_head = y == 0 && !self.head.is_empty();

        let mut out = String::new();
        for x in 0..col_count {
            let content_width = widths[x] - padding_left - padding_right;
            let lines = cell_lines[y].get(x).cloned().unwrap_or_default();
            let line = if line_num < lines.len() {
                lines[line_num].as_str()
            } else {
                ""
            };
            let content = pad(
                &truncate(line, content_width, &self.truncate_char),
                content_width,
                ' ',
                self.col_aligns.get(x).map(|s| s.as_str()).unwrap_or("left"),
            );

            let colorize = |s: String| -> String {
                if !self.style.border[0].is_empty() {
                    format!("{}{}{}", self.style.border[0], s, self.style.border[1])
                } else {
                    s
                }
            };
            let left = colorize(self.char(if x == 0 { "left" } else { "middle" }).to_string());
            let padding = " ".repeat(padding_left as usize);
            let right_padding = " ".repeat(padding_right as usize);
            let right = if x + 1 == col_count {
                colorize(self.char("right").to_string())
            } else {
                String::new()
            };

            let mut formatted = format!("{}{}{}", padding, content, right_padding);
            if is_head && !self.style.head[0].is_empty() {
                formatted = format!(
                    "{}{}{}",
                    self.style.head[0],
                    formatted,
                    self.style.head[1]
                )
            }
            out.push_str(&left);
            out.push_str(&formatted);
            out.push_str(&right)
        }
        out
    }
}

// ── column widths ─────────────────────────────────────────

pub struct ColumnConfig {
    pub headers: Vec<String>,
    pub min_widths: Vec<i64>,
    pub max_widths: Vec<i64>,
    pub min_avail: Option<i64>,
}

pub const BORDER_AND_PADDING: i64 = 16;

/// Calculate column widths that fit the terminal width (mirror of
/// display-constants.ts calcColumnWidths).
pub fn calc_column_widths(
    rows: &[Vec<String>],
    config: &ColumnConfig,
    term_width: i64,
    table_min_width: i64,
) -> Vec<i64> {
    let avail = (config.min_avail.unwrap_or(table_min_width.max(40)))
        .max(term_width - BORDER_AND_PADDING);

    let weights: Vec<i64> = config
        .headers
        .iter()
        .enumerate()
        .map(|(i, hdr)| {
            let mut max = strlen(hdr);
            for row in rows {
                if let Some(cell) = row.get(i) {
                    let len = strlen(cell);
                    if len > max {
                        max = len
                    }
                }
            }
            max.min(config.max_widths[i])
        })
        .collect();

    let total_weight: i64 = weights.iter().sum();
    let mut widths: Vec<i64> = weights
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let rough = if total_weight == 0 {
                0.0
            } else {
                (avail as f64 * *w as f64) / total_weight as f64
            };
            (rough.round() as i64)
                .max(config.min_widths[i])
                .min(config.max_widths[i])
        })
        .collect();

    // Adjust to exactly fit avail
    let mut sum: i64 = widths.iter().sum();
    let mut diff = sum - avail;
    let mut iterations = 0;

    while diff > 0 && iterations < 100 {
        let mut idx: Option<usize> = None;
        for i in 0..widths.len() {
            if widths[i] > config.min_widths[i] && (idx.is_none() || widths[i] > widths[idx.unwrap()])
            {
                idx = Some(i)
            }
        }
        let Some(i) = idx else { break };
        widths[i] -= 1;
        diff -= 1;
        iterations += 1
    }

    iterations = 0;
    while diff < 0 && iterations < 100 {
        let mut idx: Option<usize> = None;
        for i in 0..widths.len() {
            if widths[i] < config.max_widths[i] && (idx.is_none() || weights[i] > weights[idx.unwrap()])
            {
                idx = Some(i)
            }
        }
        let Some(i) = idx else { break };
        widths[i] += 1;
        diff += 1;
        iterations += 1
    }

    widths
}