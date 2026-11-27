//! Interactive line-based prompts.
//!
//! The native CLI used to skip every guided flow because the port only
//! implemented the flag paths. These helpers restore the prompts while
//! staying dependency-free and cross-platform: they read whole lines from
//! stdin instead of switching the terminal into raw mode.
//!
//! Every prompt flushes the buffered logger first (`crate::flush()`), so the
//! banner and any content printed before the question is visible before the
//! user is asked to type.

use std::io::{self, BufRead, IsTerminal, Read, Write};

use crate::ansi;
use crate::i18n::tr;

/// A selectable entry. `value` is what the caller receives; `label` is shown.
#[derive(Clone, Debug)]
pub struct Choice {
    pub label: String,
    pub value: String,
}

impl Choice {
    pub fn new(label: &str, value: &str) -> Self {
        Self {
            label: label.to_string(),
            value: value.to_string(),
        }
    }

    /// Choice whose label and value are the same string.
    pub fn plain(value: &str) -> Self {
        Self::new(value, value)
    }
}

/// True when prompts may read from the terminal.
///
/// `SUBTRACK_ASSUME_TTY=1` forces prompting for automated tests (a test can
/// pipe answers in); without it only a real TTY is considered interactive so
/// the scriptable paths keep their old non-blocking behavior.
pub fn interactive() -> bool {
    if std::env::var_os("SUBTRACK_ASSUME_TTY").is_some() {
        return true;
    }
    io::stdin().is_terminal()
}

fn out(text: &str) {
    // flush pending logger output before drawing the prompt
    crate::flush();
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let _ = handle.write_all(text.as_bytes());
    let _ = handle.flush();
}

/// Cursor control is for real terminals only. `SUBTRACK_ASSUME_TTY` makes
/// prompts run with piped answers in tests; those byte streams must stay
/// plain, or the smoke snapshots would fill up with escape codes.
fn cursor_control() -> bool {
    io::stdin().is_terminal() && crate::ui::is_stdout_tty()
}

fn arrow_mode() -> bool {
    interactive() && cursor_control()
}

/// Erase `lines` already-drawn prompt lines and leave the cursor where the
/// first line started. Called before redrawing or after a choice is made so
/// spent selection screens do not pile up in the console.
fn clear_lines(lines: usize) {
    if cursor_control() {
        crate::flush();
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        let _ = write_clear(&mut handle, lines);
    }
}

fn write_clear(writer: &mut impl Write, lines: usize) -> io::Result<()> {
    if lines == 0 {
        return Ok(());
    }
    writer.write_all(format!("\x1b[{lines}A\x1b[0J").as_bytes())?;
    writer.flush()
}

/// Approximate terminal columns (CJK/fullwidth chars take two) so a drawn
/// frame never wraps: a wrapped logical line would break the cursor-up count.
fn char_width(c: char) -> usize {
    let n = c as u32;
    if (0x300..=0x36f).contains(&n) || (0x7f..0xa0).contains(&n) {
        return 0;
    }
    if matches!(
        n,
        0x1100..=0x115f
            | 0x2e80..=0xa4cf
            | 0xac00..=0xd7a3
            | 0xf900..=0xfaff
            | 0xfe10..=0xfe19
            | 0xfe30..=0xfe6f
            | 0xff00..=0xff60
            | 0xffe0..=0xffe6
            | 0x1f300..=0x1faff
    ) {
        2
    } else {
        1
    }
}

fn truncate_to_width(text: &str, max: usize) -> String {
    let mut out = String::new();
    let mut width = 0;
    for c in text.chars() {
        let w = char_width(c);
        if width + w > max {
            break;
        }
        out.push(c);
        width += w;
    }
    out
}

fn ask(message: &str) -> Option<String> {
    out(&format!("{} {} \u{203a} ", ansi::cyan("?"), tr(message)));
    let mut line = String::new();
    match io::stdin().lock().read_line(&mut line) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(line.trim_end_matches(['\r', '\n']).to_string()),
    }
}

/// Confirm a field without echoing an empty (optional) value.
fn finish_only(message: &str) {
    out(&format!("{} {}\n", ansi::green("\u{2714}"), tr(message)));
}

/// Long choice lists are capped: a short list is scannable, a long one is
/// searched by typing part of a label (see `select`).
const MAX_VISIBLE_CHOICES: usize = 10;

fn choice_matches(choice: &Choice, needle: &str) -> bool {
    choice.label.to_lowercase().contains(needle) || choice.value.to_lowercase().contains(needle)
}

fn choice_equals(choice: &Choice, needle: &str) -> bool {
    choice.value.to_lowercase() == needle || choice.label.to_lowercase() == needle
}

fn finish(message: &str, value: &str) {
    out(&format!(
        "{} {} {}\n",
        ansi::green("\u{2714}"),
        ansi::dim(&tr(message)),
        ansi::green(value)
    ));
}

fn cancel(message: &str) {
    out(&format!("{} {}\n", ansi::red("\u{2716}"), tr(message)));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Key {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Esc,
    Space,
    Backspace,
    CtrlC,
    CtrlU,
    Char(char),
}

#[cfg(unix)]
struct RawModeGuard {
    fd: i32,
    original: libc::termios,
}

#[cfg(unix)]
impl RawModeGuard {
    fn enable() -> Option<Self> {
        unsafe {
            let fd = libc::STDIN_FILENO;
            let mut original: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(fd, &mut original) != 0 {
                return None;
            }
            let mut raw = original;
            raw.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ISIG);
            raw.c_iflag &= !(libc::IXON | libc::ICRNL);
            raw.c_cc[libc::VMIN] = 1;
            raw.c_cc[libc::VTIME] = 0;
            if libc::tcsetattr(fd, libc::TCSANOW, &raw) != 0 {
                return None;
            }
            Some(Self { fd, original })
        }
    }
}

#[cfg(unix)]
impl Drop for RawModeGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = libc::tcsetattr(self.fd, libc::TCSANOW, &self.original);
        }
    }
}

#[cfg(not(unix))]
struct RawModeGuard;

#[cfg(not(unix))]
impl RawModeGuard {
    fn enable() -> Option<Self> {
        None
    }
}

fn read_key() -> Option<Key> {
    let mut stdin = io::stdin().lock();
    let mut buf = [0u8; 1];
    if stdin.read_exact(&mut buf).is_err() {
        return None;
    }
    match buf[0] {
        b'\r' | b'\n' => Some(Key::Enter),
        3 => Some(Key::CtrlC),
        21 => Some(Key::CtrlU),
        127 | 8 => Some(Key::Backspace),
        27 => {
            let mut seq = [0u8; 2];
            if stdin.read_exact(&mut seq[0..1]).is_err() {
                return Some(Key::Esc);
            }
            if seq[0] != b'[' && seq[0] != b'O' {
                return Some(Key::Esc);
            }
            if stdin.read_exact(&mut seq[1..2]).is_err() {
                return Some(Key::Esc);
            }
            match seq[1] {
                b'A' => Some(Key::Up),
                b'B' => Some(Key::Down),
                b'C' => Some(Key::Right),
                b'D' => Some(Key::Left),
                _ => Some(Key::Esc),
            }
        }
        b' ' => Some(Key::Space),
        c if c.is_ascii_graphic() => Some(Key::Char(c as char)),
        _ => None,
    }
}

fn visible_indices(choices: &[Choice], filter: &Option<String>) -> Vec<usize> {
    match filter {
        Some(needle) => choices
            .iter()
            .enumerate()
            .filter(|(_, c)| choice_matches(c, needle))
            .map(|(i, _)| i)
            .collect(),
        None => (0..choices.len()).collect(),
    }
}

/// Render a numbered choice list.
///
/// Numbers are dimmed and right-aligned so they line up on long lists, and
/// the pre-selected choice carries a `›` pointer — the "(default)" suffix in
/// the question leaves the default discoverable without scanning list text.
/// `visible` holds indices into `choices`; `highlight` is a position in
/// `visible`, so filtered lists can be renumbered from 1.
fn render_numbered_choices(
    choices: &[Choice],
    visible: &[usize],
    highlight: Option<usize>,
) -> usize {
    let number_width = visible.len().to_string().len();
    for (position, &index) in visible.iter().enumerate() {
        let choice = &choices[index];
        let is_pointer = Some(position) == highlight;
        let gutter = if is_pointer {
            format!("{} ", ansi::cyan("\u{203a}"))
        } else {
            "  ".to_string()
        };
        let plain_label = if cursor_control() {
            let available =
                (crate::ui::term_width().max(20) as usize).saturating_sub(number_width + 7);
            truncate_to_width(&choice.label, available)
        } else {
            choice.label.clone()
        };
        let label = if is_pointer {
            ansi::bold(&plain_label)
        } else {
            plain_label
        };
        let number = format!(
            "{number:>width$}",
            number = position + 1,
            width = number_width + 1
        );
        out(&format!("{gutter}{}) {}\n", ansi::dim(&number), label));
    }
    visible.len()
}

/// Full list used by `checkbox` (short lists: subscriptions, trials, fields).
fn render_choices(choices: &[Choice], highlight: Option<usize>) -> usize {
    let indices: Vec<usize> = (0..choices.len()).collect();
    render_numbered_choices(choices, &indices, highlight)
}

/// Render the (possibly filtered, capped) choices used by `select`.
fn render_select_choices(
    choices: &[Choice],
    visible: &[usize],
    highlight: Option<usize>,
    filtered: bool,
) -> usize {
    let shown = visible.len().min(MAX_VISIBLE_CHOICES);
    render_numbered_choices(choices, &visible[..shown], highlight);
    let mut lines = shown;
    if visible.len() > MAX_VISIBLE_CHOICES {
        out(&format!(
            "  … {} more — type to narrow down\n",
            visible.len() - MAX_VISIBLE_CHOICES
        ));
        lines += 1;
    }
    if filtered && visible.len() <= MAX_VISIBLE_CHOICES {
        out(&format!("  ({} matching)\n", visible.len()));
        lines += 1;
    }
    lines
}

/// Prompt for a line of text.
///
/// `default` is returned when the user submits an empty line. Validation
/// failures re-ask instead of aborting. `None` means EOF with no default, so
/// the caller can abort the workflow quietly (matching the piped-in behavior
/// of the TypeScript implementation).
pub type Validator = fn(&str) -> Result<(), String>;

pub fn input(message: &str, default: Option<&str>, validate: Option<Validator>) -> Option<String> {
    input_inner(message, default, validate, None, false)
}

/// Like [`input`], but a literal `-` returns `None` so the caller can go back
/// without saving anything. Used by the settings wizard's value prompts.
pub fn input_cancelable(
    message: &str,
    default: Option<&str>,
    validate: Option<Validator>,
) -> Option<String> {
    input_inner(message, default, validate, Some("-"), false)
}

/// Like [`input_cancelable`], but an empty answer is also a cancellation when
/// there is no default value to fall back to.
pub fn input_optional_cancelable(
    message: &str,
    default: Option<&str>,
    validate: Option<Validator>,
) -> Option<String> {
    input_inner(message, default, validate, Some("-"), true)
}

fn input_inner(
    message: &str,
    default: Option<&str>,
    validate: Option<Validator>,
    cancel_token: Option<&str>,
    empty_cancels: bool,
) -> Option<String> {
    if !interactive() {
        return default.map(|d| d.to_string());
    }

    let mut hint = match default {
        Some(d) if !d.is_empty() => format!("{} [{}]", message, d),
        _ => message.to_string(),
    };
    if cancel_token.is_some() {
        hint.push_str(" (- = back)");
    }

    loop {
        let answer = ask(&hint)?;
        if cancel_token.is_some_and(|token| answer.trim() == token) {
            cancel("Cancelled");
            return None;
        }
        if answer.is_empty() && empty_cancels && default.is_none() {
            cancel("Cancelled");
            return None;
        }
        let value = if answer.is_empty() {
            match default {
                Some(d) => d.to_string(),
                None => {
                    cancel("A value is required");
                    continue;
                }
            }
        } else {
            answer
        };

        if let Some(check) = validate
            && let Err(error) = check(&value)
        {
            cancel(&error);
            continue;
        }

        if value.is_empty() {
            finish_only(message);
        } else {
            finish(message, &value);
        }
        return Some(value);
    }
}

fn select_filter_hint(
    message: &str,
    filter: &Option<String>,
    visible_len: usize,
    choices_len: usize,
    highlight: Option<usize>,
) -> String {
    match filter {
        Some(needle) => format!(
            "{} [{} matching \"{}\"] (↑↓ move, Enter select, type to filter, Esc cancel)",
            message, visible_len, needle
        ),
        None => match highlight {
            Some(i) => format!(
                "{} [1-{}] (default {}, ↑↓ move, Enter select, type to filter)",
                message,
                choices_len,
                i + 1
            ),
            None => format!(
                "{} [1-{}] (↑↓ move, Enter select, type to filter)",
                message, choices_len
            ),
        },
    }
}

fn select_arrow(message: &str, choices: &[Choice], default: Option<usize>) -> Option<String> {
    let _raw = RawModeGuard::enable()?;
    let mut filter: Option<String> = None;
    let mut cursor: usize = default.unwrap_or(0).min(choices.len().saturating_sub(1));
    let mut drawn: usize = 0;
    loop {
        clear_lines(drawn);
        let visible = visible_indices(choices, &filter);
        if visible.is_empty() {
            filter = None;
            cursor = 0;
            out(&format!(
                "  {}\n",
                ansi::dim(&tr("No matches — type to filter"))
            ));
            drawn = 1;
            continue;
        }
        if cursor >= visible.len() {
            cursor = visible.len() - 1;
        }
        let shown = visible.len().min(MAX_VISIBLE_CHOICES);
        let page_start = if cursor >= MAX_VISIBLE_CHOICES {
            cursor + 1 - MAX_VISIBLE_CHOICES
        } else {
            0
        };
        let page_end = (page_start + MAX_VISIBLE_CHOICES).min(visible.len());
        let page = &visible[page_start..page_end];
        drawn = render_numbered_choices(choices, page, Some(cursor - page_start));
        if visible.len() > MAX_VISIBLE_CHOICES {
            out(&format!(
                "  … {} more — type to narrow down\n",
                visible.len() - shown
            ));
            drawn += 1;
        }
        let filter_text = filter.as_deref().unwrap_or("");
        let hint = select_filter_hint(
            message,
            &filter,
            visible.len(),
            choices.len(),
            default.filter(|_| filter.is_none()),
        );
        let hint = truncate_to_width(
            &hint,
            (crate::ui::term_width().max(20) as usize).saturating_sub(12),
        );
        out(&format!(
            "{} {} \u{203a} {}\n",
            ansi::cyan("?"),
            tr(&hint),
            filter_text
        ));
        drawn += 1;

        match read_key()? {
            Key::Up => cursor = cursor.saturating_sub(1),
            Key::Down => {
                if cursor + 1 < visible.len() {
                    cursor += 1;
                }
            }
            Key::Enter => {
                let index = visible[cursor];
                let choice = &choices[index];
                clear_lines(drawn);
                finish(message, &choice.label);
                return Some(choice.value.clone());
            }
            Key::Esc | Key::CtrlC => {
                clear_lines(drawn);
                cancel("Cancelled");
                return None;
            }
            Key::Backspace => {
                if let Some(s) = filter.as_mut() {
                    s.pop();
                }
                if filter.as_ref().is_some_and(|s| s.is_empty()) {
                    filter = None;
                }
                cursor = 0;
            }
            Key::CtrlU => {
                filter = None;
                cursor = 0;
            }
            Key::Char(c) => {
                {
                    let f = filter.get_or_insert_with(String::new);
                    f.push(c);
                }
                cursor = 0;
                let f = filter.as_deref().unwrap_or("");
                let v = visible_indices(choices, &filter);
                if v.len() == 1 && choice_equals(&choices[v[0]], f) {
                    let choice = &choices[v[0]];
                    clear_lines(drawn);
                    finish(message, &choice.label);
                    return Some(choice.value.clone());
                }
            }
            _ => {}
        }
    }
}

/// Pick one entry from a list.
///
/// On a real terminal this uses arrow-key navigation with optional type-to-filter.
/// In piped/test environments it falls back to the line-based numbered input so
/// existing scripts and smoke tests keep working.
pub fn select(message: &str, choices: &[Choice], default: Option<usize>) -> Option<String> {
    if choices.is_empty() {
        return None;
    }
    if !interactive() {
        return default
            .filter(|i| *i < choices.len())
            .map(|i| choices[i].value.clone());
    }
    if arrow_mode()
        && let Some(value) = select_arrow(message, choices, default)
    {
        return Some(value);
    }
    // If raw mode could not be enabled, fall through to line mode.

    let mut filter: Option<String> = None;
    let mut drawn: usize = 0;

    loop {
        clear_lines(drawn);
        let visible = visible_indices(choices, &filter);
        if visible.is_empty() {
            filter = None;
            cancel("No matches — showing all choices");
            drawn = 1;
            continue;
        }

        let highlight = if filter.is_none() {
            default.filter(|i| *i < choices.len())
        } else {
            None
        };
        drawn = render_select_choices(choices, &visible, highlight, filter.is_some()) + 1;

        let hint = match &filter {
            Some(needle) => format!(
                "{} [{} matching \"{}\"] (empty = show all)",
                message,
                visible.len(),
                needle
            ),
            None => match highlight {
                Some(i) => format!(
                    "{} [1-{}] (default {}, type to filter)",
                    message,
                    choices.len(),
                    i + 1
                ),
                None => format!("{} [1-{}] (type to filter)", message, choices.len()),
            },
        };
        let hint = if cursor_control() {
            let available = (crate::ui::term_width().max(20) as usize).saturating_sub(12);
            truncate_to_width(&hint, available)
        } else {
            hint
        };

        let answer = match ask(&hint) {
            Some(answer) => answer,
            None => {
                clear_lines(drawn);
                return None;
            }
        };
        let trimmed = answer.trim();

        if trimmed.is_empty() {
            if filter.is_some() {
                filter = None;
                continue;
            }
            if let Some(i) = highlight {
                clear_lines(drawn);
                finish(message, &choices[i].label);
                return Some(choices[i].value.clone());
            }
            clear_lines(drawn);
            cancel("Please choose a number");
            drawn = 1;
            continue;
        }

        if let Ok(number) = trimmed.parse::<usize>() {
            if let Some(&index) = visible.get(number.saturating_sub(1)) {
                let choice = &choices[index];
                clear_lines(drawn);
                finish(message, &choice.label);
                return Some(choice.value.clone());
            }
            clear_lines(drawn);
            cancel("Invalid choice");
            drawn = 1;
            continue;
        }

        let needle = trimmed.to_lowercase();
        if let Some(index) = choices.iter().position(|c| choice_equals(c, &needle)) {
            let choice = &choices[index];
            clear_lines(drawn);
            finish(message, &choice.label);
            return Some(choice.value.clone());
        }

        let matches: Vec<usize> = choices
            .iter()
            .enumerate()
            .filter(|(_, c)| choice_matches(c, &needle))
            .map(|(i, _)| i)
            .collect();
        match matches.len() {
            1 => {
                let choice = &choices[matches[0]];
                clear_lines(drawn);
                finish(message, &choice.label);
                return Some(choice.value.clone());
            }
            n if n > 1 => filter = Some(needle),
            _ => {
                clear_lines(drawn);
                cancel("Invalid choice");
                drawn = 1;
            }
        }
    }
}

fn confirm_arrow(message: &str, default: bool) -> Option<bool> {
    let _raw = RawModeGuard::enable()?;
    let mut value = default;
    let mut drawn = 0usize;
    loop {
        clear_lines(drawn);
        let yes = if value {
            ansi::bold(&ansi::green("Yes"))
        } else {
            tr("Yes").to_string()
        };
        let no = if !value {
            ansi::bold(&ansi::green("No"))
        } else {
            tr("No").to_string()
        };
        out(&format!(
            "{} {}  {}/{}  {}\n",
            ansi::cyan("?"),
            tr(message),
            yes,
            no,
            ansi::dim("(←/→, Enter, y/n, Esc)")
        ));
        drawn = 1;
        match read_key()? {
            Key::Left | Key::Up | Key::Char('h') | Key::Char('H') => value = true,
            Key::Right | Key::Down | Key::Char('l') | Key::Char('L') => value = false,
            Key::Char('y') | Key::Char('Y') => {
                clear_lines(drawn);
                finish(message, &tr("yes"));
                return Some(true);
            }
            Key::Char('n') | Key::Char('N') => {
                clear_lines(drawn);
                finish(message, &tr("no"));
                return Some(false);
            }
            Key::Enter => {
                clear_lines(drawn);
                let label = if value { tr("yes") } else { tr("no") };
                finish(message, &label);
                return Some(value);
            }
            Key::Esc | Key::CtrlC => {
                clear_lines(drawn);
                cancel("Cancelled");
                return None;
            }
            _ => {}
        }
    }
}

/// Yes/no confirmation. `None` means EOF/cancel, never an implicit answer.
pub fn confirm(message: &str, default: bool) -> Option<bool> {
    if !interactive() {
        return None;
    }
    if arrow_mode()
        && let Some(v) = confirm_arrow(message, default)
    {
        return Some(v);
    }
    let hint = if default {
        format!("{} (Y/n)", message)
    } else {
        format!("{} (y/N)", message)
    };

    loop {
        let answer = ask(&hint)?;
        let value = match answer.trim().to_lowercase().as_str() {
            "" => default,
            "y" | "yes" => true,
            "n" | "no" => false,
            _ => {
                cancel("Please answer y or n");
                continue;
            }
        };
        let answer = if value { tr("yes") } else { tr("no") };
        finish(message, &answer);
        return Some(value);
    }
}

fn render_checkbox_choices(choices: &[Choice], selected: &[bool], cursor: usize) -> usize {
    for (i, choice) in choices.iter().enumerate() {
        let pointer = if i == cursor {
            ansi::cyan("\u{203a}")
        } else {
            " ".to_string()
        };
        let mark = if selected[i] {
            ansi::green("[x]")
        } else {
            ansi::dim("[ ]")
        };
        let label = if i == cursor {
            ansi::bold(&choice.label)
        } else {
            choice.label.clone()
        };
        out(&format!("{} {} {}\n", pointer, mark, label));
    }
    choices.len()
}

fn checkbox_arrow(message: &str, choices: &[Choice]) -> Option<Vec<String>> {
    let _raw = RawModeGuard::enable()?;
    let mut selected = vec![false; choices.len()];
    let mut cursor = 0usize;
    let mut drawn = 0usize;
    loop {
        clear_lines(drawn);
        drawn = render_checkbox_choices(choices, &selected, cursor);
        out(&format!(
            "{} {} {}\n",
            ansi::cyan("?"),
            tr(message),
            ansi::dim("(↑↓ move, Space toggle, a all, Enter confirm, Esc cancel)")
        ));
        drawn += 1;
        match read_key()? {
            Key::Up => cursor = cursor.saturating_sub(1),
            Key::Down => {
                if cursor + 1 < choices.len() {
                    cursor += 1;
                }
            }
            Key::Space => selected[cursor] = !selected[cursor],
            Key::Char('a') | Key::Char('A') => {
                let all = selected.iter().all(|v| *v);
                selected.fill(!all);
            }
            Key::Enter => {
                clear_lines(drawn);
                let values: Vec<String> = choices
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| selected[*i])
                    .map(|(_, c)| c.value.clone())
                    .collect();
                let labels: Vec<String> = choices
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| selected[*i])
                    .map(|(_, c)| c.label.clone())
                    .collect();
                if labels.is_empty() {
                    finish(message, &tr("none"));
                } else {
                    finish(message, &labels.join(", "));
                }
                return Some(values);
            }
            Key::Esc | Key::CtrlC => {
                clear_lines(drawn);
                cancel("Cancelled");
                return None;
            }
            _ => {}
        }
    }
}

/// Pick zero or more entries.
///
/// On a real terminal this uses arrow-key navigation + Space toggle.
/// In piped/test environments it falls back to line-based numbered input.
pub fn checkbox(message: &str, choices: &[Choice]) -> Option<Vec<String>> {
    if choices.is_empty() {
        return Some(Vec::new());
    }
    if !interactive() {
        return None;
    }
    if arrow_mode()
        && let Some(v) = checkbox_arrow(message, choices)
    {
        return Some(v);
    }

    render_choices(choices, None);

    loop {
        let answer = ask(&format!("{} (numbers, 'all', empty = cancel)", message))?;
        let trimmed = answer.trim();
        if trimmed.is_empty() {
            finish(message, &tr("none"));
            return Some(Vec::new());
        }
        if trimmed.eq_ignore_ascii_case("all") {
            let values = choices.iter().map(|c| c.value.clone()).collect();
            finish(message, &tr("all"));
            return Some(values);
        }

        let mut selected: Vec<String> = Vec::new();
        let mut ok = true;
        for part in trimmed.split([',', ' ']).filter(|p| !p.trim().is_empty()) {
            let part = part.trim();
            let found = if let Ok(number) = part.parse::<usize>() {
                if number >= 1 && number <= choices.len() {
                    Some(&choices[number - 1])
                } else {
                    None
                }
            } else {
                let needle = part.to_lowercase();
                choices
                    .iter()
                    .find(|c| c.value.to_lowercase() == needle || c.label.to_lowercase() == needle)
            };
            match found {
                Some(choice) => {
                    if !selected.contains(&choice.value) {
                        selected.push(choice.value.clone());
                    }
                }
                None => {
                    cancel(&format!("Invalid choice: {}", part));
                    ok = false;
                    break;
                }
            }
        }

        if !ok {
            continue;
        }
        let labels: Vec<String> = choices
            .iter()
            .filter(|c| selected.contains(&c.value))
            .map(|c| c.label.clone())
            .collect();
        finish(message, &labels.join(", "));
        return Some(selected);
    }
}

/// Convenience: a labelled subscription picker used by several commands.
pub fn pick_subscription(
    message: &str,
    subs: &[crate::models::Subscription],
    include_archived: bool,
) -> Option<i64> {
    let choices: Vec<Choice> = subs
        .iter()
        .filter(|s| include_archived || !crate::domain::is_archived(&s.status))
        .map(|s| {
            let tags = if s.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", s.tags.join(", "))
            };
            let status = if crate::domain::is_active(&s.status) {
                String::new()
            } else {
                format!(" ({})", s.status)
            };
            Choice::new(
                &format!(
                    "#{} {} \u{2014} {}/{}{}{}",
                    s.id,
                    s.name,
                    crate::price::format_price(s.price, &s.currency),
                    s.cycle,
                    tags,
                    status
                ),
                &s.id.to_string(),
            )
        })
        .collect();
    if choices.is_empty() {
        crate::logger::info("No subscriptions found \u{2014} try `subtrack add`");
        return None;
    }
    let value = select(message, &choices, None)?;
    value.parse::<i64>().ok()
}

#[cfg(test)]
mod tests {
    use super::{truncate_to_width, write_clear};

    #[test]
    fn clear_sequence_erases_the_drawn_frame() {
        let mut buffer = Vec::new();
        write_clear(&mut buffer, 12).unwrap();
        assert_eq!(buffer, b"\x1b[12A\x1b[0J");
    }

    #[test]
    fn clear_sequence_is_empty_without_a_frame() {
        let mut buffer = Vec::new();
        write_clear(&mut buffer, 0).unwrap();
        assert!(buffer.is_empty());
    }

    #[test]
    fn truncation_counts_fullwidth_characters_as_two_columns() {
        assert_eq!(truncate_to_width("abcde", 3), "abc");
        assert_eq!(truncate_to_width("日ab", 3), "日a");
    }
}
