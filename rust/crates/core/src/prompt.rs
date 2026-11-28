//! Interactive prompts: menus for choices, line input for free-form values.
//!
//! Choice prompts (select/confirm/checkbox) draw a proper selection menu on a
//! terminal — a heading, highlighted rows and a key-hint footer, navigated with
//! arrow keys. No numbered question is printed and no answer has to be typed.
//! Piped/test flows keep the line-based numbered input so scripts still work.
//!
//! Every prompt flushes the buffered logger first (`crate::flush()`), so the
//! banner and any content printed before the question is visible before the
//! user is asked to type.

use std::cell::RefCell;
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
    let _ = out_checked(text);
}

/// Like [`out`], but lets raw-mode callers abort when the terminal cannot be
/// written (a closed stdout would otherwise look like a hung prompt).
fn out_checked(text: &str) -> io::Result<()> {
    // flush pending logger output before drawing the prompt
    crate::flush();
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    handle.write_all(text.as_bytes())?;
    handle.flush()
}

/// Cursor control is for real terminals only. `SUBTRACK_ASSUME_TTY` makes
/// prompts run with piped answers in tests; those byte streams must stay
/// plain, or the smoke snapshots would fill up with escape codes.
fn cursor_control() -> bool {
    io::stdin().is_terminal() && crate::ui::is_stdout_tty()
}

/// Choice prompts need only stdin in raw mode to become a real selection UI.
/// Requiring a TTY on stdout as well pushed redirected terminals back into the
/// numbered question flow, which looked like a menu but still asked for input.
fn arrow_mode() -> bool {
    io::stdin().is_terminal()
}

/// Erase `lines` already-drawn prompt lines and leave the cursor where the
/// first line started. Called before redrawing or after a choice is made so
/// spent selection screens do not pile up in the console.
fn clear_lines(lines: usize) {
    let _ = clear_lines_checked(lines);
}

fn clear_lines_checked(lines: usize) -> io::Result<()> {
    if cursor_control() {
        crate::flush();
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        write_clear(&mut handle, lines)
    } else {
        Ok(())
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
fn is_emoji_modifier(c: char) -> bool {
    (0x1f3fb..=0x1f3ff).contains(&(c as u32))
}

fn is_emoji_base(c: char) -> bool {
    (0x1f000..=0x1faff).contains(&(c as u32))
}

fn char_width(c: char) -> usize {
    let n = c as u32;
    if (0x300..=0x36f).contains(&n)
        || (0x7f..0xa0).contains(&n)
        || (0x1ab0..=0x1aff).contains(&n)
        || (0x1dc0..=0x1dff).contains(&n)
        || (0x20d0..=0x20ff).contains(&n)
        || (0xfe00..=0xfe0f).contains(&n)
        || n == 0x200d
    {
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
            | 0x20000..=0x3fffd
    ) {
        2
    } else {
        1
    }
}

/// Skin-tone modifiers are double-width on their own but add no column after
/// an emoji base (the terminal renders one combined glyph).
fn width_after(previous: Option<char>, c: char) -> usize {
    if is_emoji_modifier(c) && previous.is_some_and(is_emoji_base) {
        0
    } else {
        char_width(c)
    }
}

fn display_width(text: &str) -> usize {
    let mut previous = None;
    let mut width = 0;
    for c in text.chars() {
        width += width_after(previous, c);
        previous = Some(c);
    }
    width
}

/// Number of physical terminal rows a plain (ANSI-free) line occupies.
fn wrapped_lines(plain: &str, width: usize) -> usize {
    let width = width.max(1);
    let columns = display_width(plain).max(1);
    columns.div_ceil(width)
}

fn truncate_to_width(text: &str, max: usize) -> String {
    let mut out = String::new();
    let mut width = 0;
    let mut previous = None;
    for c in text.chars() {
        let w = width_after(previous, c);
        if width + w > max {
            break;
        }
        out.push(c);
        width += w;
        previous = Some(c);
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
    let needle = needle.to_lowercase();
    choice.label.to_lowercase().contains(&needle) || choice.value.to_lowercase().contains(&needle)
}

fn choice_equals(choice: &Choice, needle: &str) -> bool {
    let needle = needle.to_lowercase();
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
    CtrlD,
    CtrlU,
    Char(char),
    Ignored,
}

/// Why an arrow-key prompt stopped.
///
/// `Unavailable` must stay distinct from `Cancelled`: the line-mode fallback
/// is only appropriate when raw mode could not be enabled, never when the
/// user explicitly cancelled or stdin reached EOF.
#[derive(Debug, PartialEq, Eq)]
enum ArrowOutcome<T> {
    Value(T),
    Cancelled,
    Unavailable,
}

enum ArrowKey {
    Key(Key),
    Abort,
}

fn read_arrow_key(drawn: usize) -> ArrowKey {
    match read_key() {
        Some(Key::Esc | Key::CtrlC | Key::CtrlD) | None => {
            let _ = clear_lines_checked(drawn);
            cancel("Cancelled");
            ArrowKey::Abort
        }
        Some(key) => ArrowKey::Key(key),
    }
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

#[cfg(unix)]
fn stdin_has_byte(timeout_ms: i32) -> bool {
    let mut descriptor = libc::pollfd {
        fd: libc::STDIN_FILENO,
        events: libc::POLLIN,
        revents: 0,
    };
    let ready = unsafe { libc::poll(&mut descriptor, 1, timeout_ms) };
    ready > 0 && descriptor.revents & libc::POLLIN != 0
}

#[cfg(not(unix))]
fn stdin_has_byte(_timeout_ms: i32) -> bool {
    true
}

/// Decode one key from `reader`. `more` reports whether another byte is
/// available before an escape sequence continues; standalone `Esc` therefore
/// never blocks waiting for a second byte.
/// A byte source that can put back a byte when `Esc` turns out to be
/// standalone instead of an escape-sequence prefix.
trait KeyInput: Read {
    fn unread(&mut self, byte: u8);
}

impl KeyInput for &[u8] {
    fn unread(&mut self, _byte: u8) {}
}

fn read_key_from<R: KeyInput>(reader: &mut R, more: impl Fn() -> bool) -> Option<Key> {
    let mut buf = [0u8; 1];
    if reader.read_exact(&mut buf).is_err() {
        return None;
    }
    match buf[0] {
        b'\r' | b'\n' => Some(Key::Enter),
        3 => Some(Key::CtrlC),
        4 => Some(Key::CtrlD),
        21 => Some(Key::CtrlU),
        127 | 8 => Some(Key::Backspace),
        27 => {
            if !more() {
                return Some(Key::Esc);
            }
            let mut next = [0u8; 1];
            if reader.read_exact(&mut next).is_err() {
                return Some(Key::Esc);
            }
            if next[0] != b'[' && next[0] != b'O' {
                reader.unread(next[0]);
                return Some(Key::Esc);
            }
            let mut terminator = None;
            for _ in 0..8 {
                if !more() {
                    break;
                }
                let mut byte = [0u8; 1];
                if reader.read_exact(&mut byte).is_err() {
                    break;
                }
                if (0x40..=0x7e).contains(&byte[0]) {
                    terminator = Some(byte[0]);
                    break;
                }
            }
            Some(match terminator {
                Some(b'A') => Key::Up,
                Some(b'B') => Key::Down,
                Some(b'C') => Key::Right,
                Some(b'D') => Key::Left,
                _ => Key::Ignored,
            })
        }
        b' ' => Some(Key::Space),
        c if c.is_ascii_graphic() => Some(Key::Char(c as char)),
        c if c >= 0x80 => {
            let length = if c & 0xe0 == 0xc0 {
                2
            } else if c & 0xf0 == 0xe0 {
                3
            } else if c & 0xf8 == 0xf0 {
                4
            } else {
                1
            };
            let mut bytes = [0u8; 4];
            bytes[0] = c;
            for index in 1..length {
                if !more() {
                    return Some(Key::Ignored);
                }
                if reader.read_exact(&mut bytes[index..=index]).is_err() {
                    return Some(Key::Ignored);
                }
            }
            match std::str::from_utf8(&bytes[..length]) {
                Ok(text) => {
                    let mut chars = text.chars();
                    match (chars.next(), chars.next()) {
                        (Some(ch), None) => Some(Key::Char(ch)),
                        _ => Some(Key::Ignored),
                    }
                }
                Err(_) => Some(Key::Ignored),
            }
        }
        _ => Some(Key::Ignored),
    }
}

/// Byte-at-a-time stdin for raw mode. `StdinLock` buffers ahead, which would
/// hide the rest of an escape sequence from `poll` and make arrows look like a
/// standalone `Esc`.
struct RawStdin;

thread_local! {
    static PENDING_KEY_BYTE: RefCell<Option<u8>> = const { RefCell::new(None) };
}

impl KeyInput for RawStdin {
    fn unread(&mut self, byte: u8) {
        PENDING_KEY_BYTE.with(|pending| *pending.borrow_mut() = Some(byte));
    }
}

impl Read for RawStdin {
    #[cfg(unix)]
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if let Some(byte) = PENDING_KEY_BYTE.with(|pending| pending.borrow_mut().take()) {
            if buffer.is_empty() {
                return Ok(0);
            }
            buffer[0] = byte;
            return Ok(1);
        }
        let count = unsafe {
            libc::read(
                libc::STDIN_FILENO,
                buffer.as_mut_ptr().cast::<libc::c_void>(),
                buffer.len(),
            )
        };
        if count < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(count as usize)
        }
    }

    #[cfg(not(unix))]
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if let Some(byte) = PENDING_KEY_BYTE.with(|pending| pending.borrow_mut().take()) {
            if buffer.is_empty() {
                return Ok(0);
            }
            buffer[0] = byte;
            return Ok(1);
        }
        io::stdin().read(buffer)
    }
}

fn read_key() -> Option<Key> {
    read_key_from(&mut RawStdin, || stdin_has_byte(80))
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

fn push_filter(filter: &mut Option<String>, c: char) {
    filter.get_or_insert_with(String::new).push(c);
}

fn pop_filter(filter: &mut Option<String>) {
    if let Some(value) = filter.as_mut() {
        value.pop();
        if value.is_empty() {
            *filter = None;
        }
    }
}

/// Keep the default selected while it remains visible after a filter edit.
fn cursor_for_filter(visible: &[usize], default: Option<usize>) -> usize {
    default
        .and_then(|choice| visible.iter().position(|&index| index == choice))
        .unwrap_or(0)
}

/// Append a typed character to the typeahead filter. Returns the exact match
/// when the new filter names one choice unambiguously.
fn push_filter_char<'a>(
    choices: &'a [Choice],
    filter: &mut Option<String>,
    cursor: &mut usize,
    default: Option<usize>,
    c: char,
) -> Option<&'a Choice> {
    push_filter(filter, c);
    let next = visible_indices(choices, filter);
    *cursor = cursor_for_filter(&next, default);
    if next.len() == 1 && choice_equals(&choices[next[0]], filter.as_deref().unwrap_or("")) {
        Some(&choices[next[0]])
    } else {
        None
    }
}

/// Line-mode numbers refer to the displayed (filtered) positions.
fn number_to_index(number: usize, visible: &[usize]) -> Option<usize> {
    number
        .checked_sub(1)
        .and_then(|position| visible.get(position).copied())
}

/// Render a numbered choice list.
///
/// Numbers are dimmed and right-aligned so they line up on long lists, and
/// the pre-selected choice carries a `›` pointer — the "(default)" suffix in
/// the question leaves the default discoverable without scanning list text.
/// `visible` holds indices into `choices`; `highlight` is an absolute choice
/// index. Paged arrow lists display absolute numbers, while line-mode lists
/// number the currently visible entries so typed positions stay predictable.
fn render_numbered_choices(
    choices: &[Choice],
    visible: &[usize],
    highlight: Option<usize>,
    absolute_numbers: bool,
) -> io::Result<usize> {
    let number_width = if absolute_numbers {
        choices.len().to_string().len()
    } else {
        visible.len().to_string().len()
    };
    let width = crate::ui::term_width().max(20) as usize;
    let mut lines = 0;
    for (position, &index) in visible.iter().enumerate() {
        let choice = &choices[index];
        let is_pointer = Some(index) == highlight;
        let gutter = if is_pointer {
            format!("{} ", ansi::cyan("\u{203a}"))
        } else {
            "  ".to_string()
        };
        let gutter_plain = if is_pointer { "\u{203a} " } else { "  " };
        let plain_label = if cursor_control() {
            let available = width.saturating_sub(number_width + 7);
            truncate_to_width(&choice.label, available)
        } else {
            choice.label.clone()
        };
        let label = if is_pointer {
            ansi::bold(&plain_label)
        } else {
            plain_label.clone()
        };
        let number = format!(
            "{number:>width$}",
            number = if absolute_numbers {
                index + 1
            } else {
                position + 1
            },
            width = number_width + 1
        );
        lines += wrapped_lines(&format!("{gutter_plain}{number}) {plain_label}"), width);
        out_checked(&format!("{gutter}{number}) {label}\n"))?;
    }
    Ok(lines)
}

/// Full list used by `checkbox` (short lists: subscriptions, trials, fields).
fn render_choices(choices: &[Choice], highlight: Option<usize>) -> io::Result<usize> {
    let indices: Vec<usize> = (0..choices.len()).collect();
    render_numbered_choices(choices, &indices, highlight, false)
}

/// Render the (possibly filtered, capped) choices used by `select`.
fn render_select_choices(
    choices: &[Choice],
    visible: &[usize],
    highlight: Option<usize>,
    filtered: bool,
) -> io::Result<usize> {
    let width = crate::ui::term_width().max(20) as usize;
    let shown = visible.len().min(MAX_VISIBLE_CHOICES);
    let mut lines = render_numbered_choices(choices, &visible[..shown], highlight, false)?;
    if visible.len() > MAX_VISIBLE_CHOICES {
        let text = format!(
            "  … {} more — type to narrow down",
            visible.len() - MAX_VISIBLE_CHOICES
        );
        lines += wrapped_lines(&text, width);
        out_checked(&format!("{text}\n"))?;
    }
    if filtered && visible.len() <= MAX_VISIBLE_CHOICES {
        let text = format!("  ({} matching)", visible.len());
        lines += wrapped_lines(&text, width);
        out_checked(&format!("{text}\n"))?;
    }
    Ok(lines)
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

/// One rendered line of a selection menu. `styled` is written to the terminal;
/// `plain` is used only to count the physical rows a line occupies so redraws
/// clear the whole frame.
struct MenuLine {
    styled: String,
    plain: String,
}

fn draw_menu(lines: &[MenuLine]) -> io::Result<usize> {
    let width = crate::ui::term_width().max(20) as usize;
    for line in lines {
        out_checked(&format!("{}\n", line.styled))?;
    }
    Ok(lines
        .iter()
        .map(|line| wrapped_lines(&line.plain, width))
        .sum())
}

/// A menu heading, never a `?` question with an input cursor.
fn title_line(message: &str, suffix: Option<&str>) -> MenuLine {
    let text = tr(message);
    let plain = match suffix {
        Some(value) => format!("{text} {value}"),
        None => text.to_string(),
    };
    let styled = match suffix {
        Some(value) => format!("{} {}", ansi::bold(&text), ansi::dim(value)),
        None => ansi::bold(&text),
    };
    MenuLine { styled, plain }
}

fn choice_line(label: &str, highlighted: bool) -> MenuLine {
    let width = crate::ui::term_width().max(20) as usize;
    let label = truncate_to_width(label, width.saturating_sub(4));
    let (pointer, styled) = if highlighted {
        (
            "\u{203a}",
            format!("{} {}", ansi::cyan("\u{203a}"), ansi::bold(&label)),
        )
    } else {
        (" ", format!("  {}", ansi::dim(&label)))
    };
    MenuLine {
        styled,
        plain: format!("{pointer} {label}"),
    }
}

fn checkbox_line(label: &str, selected: bool, highlighted: bool) -> MenuLine {
    let width = crate::ui::term_width().max(20) as usize;
    let label = truncate_to_width(label, width.saturating_sub(8));
    let mark = if selected { "[x]" } else { "[ ]" };
    let (pointer, mark_style, label_style) = if highlighted {
        (
            "\u{203a}",
            if selected {
                ansi::green(mark)
            } else {
                ansi::dim(mark)
            },
            ansi::bold(&label),
        )
    } else {
        (
            " ",
            if selected {
                ansi::green(mark)
            } else {
                ansi::dim(mark)
            },
            ansi::dim(&label),
        )
    };
    MenuLine {
        styled: format!("{} {} {}", ansi::cyan(pointer), mark_style, label_style),
        plain: format!("{pointer} {mark} {label}"),
    }
}

/// Secondary information (overflow counts, active filter) and key hints.
fn note_line(text: &str) -> MenuLine {
    MenuLine {
        styled: format!("  {}", ansi::dim(text)),
        plain: format!("  {text}"),
    }
}

fn select_arrow(message: &str, choices: &[Choice], default: Option<usize>) -> ArrowOutcome<String> {
    let _raw = match RawModeGuard::enable() {
        Some(raw) => raw,
        None => return ArrowOutcome::Unavailable,
    };
    let mut filter: Option<String> = None;
    let mut cursor = cursor_for_filter(&visible_indices(choices, &None), default);
    let mut drawn: usize = 0;

    loop {
        if clear_lines_checked(drawn).is_err() {
            return ArrowOutcome::Unavailable;
        }
        let visible = visible_indices(choices, &filter);
        if visible.is_empty() {
            let lines = vec![
                title_line(message, None),
                note_line(&tr("No matches — type to filter")),
                note_line("↑↓ move · type to filter · Esc cancel"),
            ];
            drawn = match draw_menu(&lines) {
                Ok(lines) => lines,
                Err(_) => return ArrowOutcome::Unavailable,
            };
            let key = match read_arrow_key(drawn) {
                ArrowKey::Key(key) => key,
                ArrowKey::Abort => return ArrowOutcome::Cancelled,
            };
            let exact = match key {
                Key::Backspace => {
                    pop_filter(&mut filter);
                    None
                }
                Key::CtrlU => {
                    filter = None;
                    None
                }
                Key::Char(c) => push_filter_char(choices, &mut filter, &mut cursor, default, c),
                Key::Space => push_filter_char(choices, &mut filter, &mut cursor, default, ' '),
                _ => None,
            };
            if let Some(choice) = exact {
                let _ = clear_lines_checked(drawn);
                finish(message, &choice.label);
                return ArrowOutcome::Value(choice.value.clone());
            }
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

        let mut lines = vec![title_line(message, None)];
        lines.extend(
            page.iter()
                .map(|&index| choice_line(&choices[index].label, index == visible[cursor])),
        );
        if visible.len() > shown {
            lines.push(note_line(&format!(
                "… {} more — type to narrow down",
                visible.len() - shown
            )));
        }
        if let Some(needle) = &filter {
            lines.push(note_line(&format!("Filter: {needle}")));
        }
        let hint = if filter.is_some() || visible.len() > shown {
            "↑↓ move · Enter select · type to filter · Esc cancel"
        } else {
            "↑↓ move · Enter select · Esc cancel"
        };
        lines.push(note_line(hint));
        drawn = match draw_menu(&lines) {
            Ok(lines) => lines,
            Err(_) => return ArrowOutcome::Unavailable,
        };

        let key = match read_arrow_key(drawn) {
            ArrowKey::Key(key) => key,
            ArrowKey::Abort => return ArrowOutcome::Cancelled,
        };
        match key {
            Key::Up => cursor = cursor.saturating_sub(1),
            Key::Down => {
                if cursor + 1 < visible.len() {
                    cursor += 1;
                }
            }
            Key::Enter => {
                let index = visible[cursor];
                let choice = &choices[index];
                let _ = clear_lines_checked(drawn);
                finish(message, &choice.label);
                return ArrowOutcome::Value(choice.value.clone());
            }
            Key::Backspace => {
                pop_filter(&mut filter);
                cursor = cursor_for_filter(&visible_indices(choices, &filter), default);
            }
            Key::CtrlU => {
                filter = None;
                cursor = cursor_for_filter(&visible_indices(choices, &None), default);
            }
            Key::Char(c) => {
                if let Some(choice) =
                    push_filter_char(choices, &mut filter, &mut cursor, default, c)
                {
                    let _ = clear_lines_checked(drawn);
                    finish(message, &choice.label);
                    return ArrowOutcome::Value(choice.value.clone());
                }
            }
            Key::Space => {
                if let Some(choice) =
                    push_filter_char(choices, &mut filter, &mut cursor, default, ' ')
                {
                    let _ = clear_lines_checked(drawn);
                    finish(message, &choice.label);
                    return ArrowOutcome::Value(choice.value.clone());
                }
            }
            Key::Ignored => {}
            _ => {}
        }
    }
}

/// Pick one entry from a selection menu.
///
/// On a terminal the choices are highlighted rows driven by ↑↓/Enter; typing
/// narrows long lists. Piped/test environments keep the numbered line input so
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
    if arrow_mode() {
        match select_arrow(message, choices, default) {
            ArrowOutcome::Value(value) => return Some(value),
            ArrowOutcome::Cancelled => return None,
            ArrowOutcome::Unavailable => {}
        }
    }
    // Only raw-mode setup failures fall through to line mode.

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
        drawn = match render_select_choices(choices, &visible, highlight, filter.is_some()) {
            Ok(lines) => lines + 1,
            Err(_) => return None,
        };

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
            if let Some(index) = number_to_index(number, &visible) {
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

fn confirm_arrow(message: &str, default: bool) -> ArrowOutcome<bool> {
    let _raw = match RawModeGuard::enable() {
        Some(raw) => raw,
        None => return ArrowOutcome::Unavailable,
    };
    let mut value = default;
    let mut drawn = 0usize;
    loop {
        if clear_lines_checked(drawn).is_err() {
            return ArrowOutcome::Unavailable;
        }
        let lines = vec![
            title_line(message, None),
            choice_line(&tr("Yes"), value),
            choice_line(&tr("No"), !value),
            note_line("←/→ move · Enter select · Esc cancel"),
        ];
        drawn = match draw_menu(&lines) {
            Ok(lines) => lines,
            Err(_) => return ArrowOutcome::Unavailable,
        };

        let key = match read_arrow_key(drawn) {
            ArrowKey::Key(key) => key,
            ArrowKey::Abort => return ArrowOutcome::Cancelled,
        };
        match key {
            Key::Left | Key::Up => value = true,
            Key::Right | Key::Down => value = false,
            Key::Enter => {
                let _ = clear_lines_checked(drawn);
                let label = if value { tr("yes") } else { tr("no") };
                finish(message, &label);
                return ArrowOutcome::Value(value);
            }
            _ => {}
        }
    }
}

/// Yes/no confirmation shown as a two-choice menu on a terminal.
/// `None` means EOF/cancel, never an implicit answer.
pub fn confirm(message: &str, default: bool) -> Option<bool> {
    if !interactive() {
        return None;
    }
    if arrow_mode() {
        match confirm_arrow(message, default) {
            ArrowOutcome::Value(value) => return Some(value),
            ArrowOutcome::Cancelled => return None,
            ArrowOutcome::Unavailable => {}
        }
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

fn checkbox_arrow(message: &str, choices: &[Choice]) -> ArrowOutcome<Vec<String>> {
    let _raw = match RawModeGuard::enable() {
        Some(raw) => raw,
        None => return ArrowOutcome::Unavailable,
    };
    let mut selected = vec![false; choices.len()];
    let mut cursor = 0usize;
    let mut drawn = 0usize;
    loop {
        if clear_lines_checked(drawn).is_err() {
            return ArrowOutcome::Unavailable;
        }
        let page_start = if cursor >= MAX_VISIBLE_CHOICES {
            cursor + 1 - MAX_VISIBLE_CHOICES
        } else {
            0
        };
        let page_end = (page_start + MAX_VISIBLE_CHOICES).min(choices.len());
        let page: Vec<usize> = (page_start..page_end).collect();

        let mut lines = vec![title_line(
            message,
            Some(&format!("[{}/{}]", cursor + 1, choices.len())),
        )];
        lines.extend(
            page.iter().map(|&index| {
                checkbox_line(&choices[index].label, selected[index], index == cursor)
            }),
        );
        if choices.len() > page.len() {
            lines.push(note_line(&format!(
                "… {} more — ↑↓ scroll",
                choices.len() - page.len()
            )));
        }
        lines.push(note_line(
            "↑↓ move · Space toggle · a all · Enter confirm · Esc cancel",
        ));
        drawn = match draw_menu(&lines) {
            Ok(lines) => lines,
            Err(_) => return ArrowOutcome::Unavailable,
        };

        let key = match read_arrow_key(drawn) {
            ArrowKey::Key(key) => key,
            ArrowKey::Abort => return ArrowOutcome::Cancelled,
        };
        match key {
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
                let _ = clear_lines_checked(drawn);
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
                return ArrowOutcome::Value(values);
            }
            _ => {}
        }
    }
}

/// Pick zero or more entries from a checkbox menu.
///
/// On a terminal rows are toggled with Space and confirmed with Enter.
/// In piped/test environments it falls back to line-based numbered input.
pub fn checkbox(message: &str, choices: &[Choice]) -> Option<Vec<String>> {
    if choices.is_empty() {
        return Some(Vec::new());
    }
    if !interactive() {
        return None;
    }
    if arrow_mode() {
        match checkbox_arrow(message, choices) {
            ArrowOutcome::Value(values) => return Some(values),
            ArrowOutcome::Cancelled => return None,
            ArrowOutcome::Unavailable => {}
        }
    }

    if render_choices(choices, None).is_err() {
        return None;
    }

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
    use super::{
        Choice, Key, checkbox_line, choice_line, choice_matches, cursor_for_filter, display_width,
        number_to_index, read_key_from, title_line, truncate_to_width, wrapped_lines, write_clear,
    };

    fn decode(bytes: &[u8]) -> Option<Key> {
        read_key_from(&mut &bytes[..], || true)
    }

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

    #[test]
    fn menu_choice_is_a_highlighted_row_not_a_numbered_question() {
        let row = choice_line("Add subscription", true);
        assert_eq!(row.plain, "\u{203a} Add subscription");
        assert!(!row.plain.contains(')'));

        let other = choice_line("Delete subscription", false);
        assert_eq!(other.plain, "  Delete subscription");
    }

    #[test]
    fn menu_title_is_a_heading_not_an_input_line() {
        let title = title_line("What would you like to do?", None);
        assert_eq!(title.plain, "What would you like to do?");
        assert!(!title.styled.starts_with("? "));
    }

    #[test]
    fn checkbox_rows_show_selection_state() {
        assert_eq!(checkbox_line("Name", true, true).plain, "\u{203a} [x] Name");
        assert_eq!(checkbox_line("Price", false, false).plain, "  [ ] Price");
    }

    #[test]
    fn typeahead_filter_ignores_input_case() {
        let choice = Choice::plain("Add subscription");
        assert!(choice_matches(&choice, "a"));
        assert!(choice_matches(&choice, "A"));
        assert!(!choice_matches(&choice, "z"));
    }

    #[test]
    fn standalone_esc_returns_without_waiting_for_a_second_byte() {
        assert_eq!(read_key_from(&mut &[27][..], || false), Some(Key::Esc));
    }

    #[test]
    fn standard_and_extended_escape_sequences_are_consumed() {
        assert_eq!(decode(b"\x1b[B"), Some(Key::Down));
        assert_eq!(decode(b"\x1bOA"), Some(Key::Up));
        assert_eq!(decode(b"\x1b[1;5A"), Some(Key::Up));
        assert_eq!(decode(b"\x1b[3~"), Some(Key::Ignored));
        assert_eq!(decode(b" "), Some(Key::Space));
    }

    #[test]
    fn utf8_typed_char_is_kept_in_arrow_mode() {
        assert_eq!(decode("情".as_bytes()), Some(Key::Char('情')));
        assert_eq!(decode(b"\t"), Some(Key::Ignored));
        assert_eq!(decode(b"\x04"), Some(Key::CtrlD));
    }

    #[test]
    fn wrapped_lines_counts_physical_rows() {
        assert_eq!(wrapped_lines("abcdef", 3), 2);
        assert_eq!(wrapped_lines("", 3), 1);
        assert_eq!(wrapped_lines("abc", 3), 1);
    }

    #[test]
    fn filtering_keeps_default_when_it_is_still_visible() {
        let visible = vec![0, 2];
        assert_eq!(cursor_for_filter(&visible, Some(2)), 1);
        assert_eq!(cursor_for_filter(&visible, Some(1)), 0);
    }

    #[test]
    fn typed_number_uses_displayed_position() {
        let visible = vec![0, 2, 3];
        assert_eq!(number_to_index(1, &visible), Some(0));
        assert_eq!(number_to_index(3, &visible), Some(3));
        assert_eq!(number_to_index(4, &visible), None);
        assert_eq!(number_to_index(0, &visible), None);
    }

    #[test]
    fn emoji_modifier_keeps_combined_glyph_width() {
        assert_eq!(display_width("👍"), 2);
        assert_eq!(display_width("👍🏽"), 2);
        assert_eq!(display_width("🏽"), 2);
    }
}
