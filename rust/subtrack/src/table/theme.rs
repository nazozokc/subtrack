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
    if !crate::ansi::enabled() {
        return TableStyle {
            border: [String::new(), String::new()],
            head: [String::new(), String::new()],
            padding_left: 1,
            padding_right: 1,
        };
    }
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
    if !crate::ansi::enabled() {
        return cells.to_vec();
    }
    match zebra {
        Some(name) => {
            let open = crate::ansi::bg_code(name);
            if open.is_empty() {
                return cells.to_vec();
            }
            // Re-open the row background after any nested SGR reset (for
            // example the reset emitted by a colored status cell).
            cells
                .iter()
                .map(|cell| {
                    let cell = cell.replace("\x1b[0m", &format!("\x1b[0m{open}"));
                    format!("{open}{cell}\x1b[0m")
                })
                .collect()
        }
        None => cells.to_vec(),
    }
}

/// Colorize a subscription status consistently across all views.
pub fn status_color(status: &str, theme: &Theme) -> String {
    if !crate::ansi::enabled() {
        return status.to_string();
    }
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
    if !crate::ansi::enabled() {
        return format!("── {} ──", text);
    }
    match theme.accent {
        Some(c) => format!("\x1b[1m{}── {} ──\x1b[0m", crate::ansi::fg_code(c), text),
        None => format!("── {} ──", text),
    }
}

/// Horizontal divider line (mirror of display-constants `divider`): dimmed
/// unless the resolved theme emits no colors at all.
pub fn divider(length: usize, theme: &Theme) -> String {
    let plain = !crate::ansi::enabled()
        || (theme.border.is_none()
            && theme.header.is_none()
            && theme.zebra.is_none()
            && theme.accent.is_none());
    if plain {
        return "─".repeat(length);
    }
    format!("\x1b[2m{}\x1b[0m", "─".repeat(length))
}
