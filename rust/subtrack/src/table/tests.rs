use super::text::truncate;
use super::*;

#[test]
fn strlen_ignores_ansi_and_counts_wide_chars_double() {
    assert_eq!(strlen("abc"), 3);
    assert_eq!(strlen(""), 0);
    assert_eq!(strlen("\u{1b}[31mhi\u{1b}[0m"), 2);
    assert_eq!(strlen("日本"), 4);
    assert_eq!(strlen("a日本b"), 6);
}

#[test]
fn truncate_appends_ellipsis_when_cut() {
    assert_eq!(truncate("short", 100, "…"), "short");
    assert_eq!(truncate("design-system", 7, "…"), "design…");
    // an ellipsis wider than nothing keeps the whole string
    assert_eq!(truncate("日本語", 5, "…"), "日本…");
    assert_eq!(strlen(&truncate("日本語", 3, "…")), 3);
}

#[test]
fn truncate_preserves_ansi_state() {
    let out = truncate("\u{1b}[31mabcdefgh\u{1b}[0m", 4, "…");
    // visible width must stay at the requested width
    assert_eq!(strlen(&out), 4);
    // leading color code survives, content is cut at 3 visible chars
    assert!(out.starts_with("\u{1b}[31mabc"));
    assert!(out.ends_with('…'));
}

#[test]
fn truncate_uses_given_placeholder_width() {
    // a 2-cell placeholder leaves 3 cells for the content
    assert_eq!(truncate("abcdefgh", 5, ".."), "abc..");
}
