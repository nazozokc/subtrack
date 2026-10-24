use super::text::truncate;
use super::*;

fn plain_builder() -> TableBuilder {
    let theme = Theme {
        border: None,
        header: None,
        zebra: None,
        accent: None,
        status_active: None,
        status_paused: None,
        status_cancelled: None,
        status_archived: None,
    };
    TableBuilder::new(table_style(&theme))
}

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

#[test]
fn compact_render_keeps_every_row() {
    let mut t = plain_builder();
    t.head = vec!["name".to_string()];
    for name in ["A", "B", "C", "D", "E"] {
        t.push(vec![name.to_string()]);
    }
    let out = t.to_string();

    // Skipping a divider must never swallow the row beneath it.
    for name in ["A", "B", "C", "D", "E"] {
        assert!(
            out.contains(&format!("\n│ {name} ")),
            "row {name} missing from compact table:\n{out}"
        );
    }

    // Structural seams survive: top frame, header separator, bottom frame.
    assert!(out.starts_with('\u{256d}'), "top frame:\n{out}");
    assert!(out.contains('\u{251c}'), "header separator:\n{out}");
    assert!(out.trim_end().ends_with('\u{256f}'), "bottom frame:\n{out}");

    // The header separator is the only interior seam in compact mode.
    let seams = out.lines().filter(|l| l.starts_with('\u{251c}')).count();
    assert_eq!(seams, 1, "interior dividers leaked:\n{out}");
}

#[test]
fn row_dividers_render_every_seam() {
    let mut t = plain_builder();
    t.row_dividers = true;
    t.head = vec!["name".to_string()];
    for name in ["A", "B", "C"] {
        t.push(vec![name.to_string()]);
    }
    let out = t.to_string();

    // cli-table3 parity: every data row keeps its border above it.
    let seams = out.lines().filter(|l| l.starts_with('\u{251c}')).count();
    assert_eq!(seams, 3, "dividers collapsed despite row_dividers:\n{out}");
}

#[test]
fn span_rows_keep_borders_on_both_sides() {
    let mut t = plain_builder();
    t.head = vec!["a".to_string(), "b".to_string()];
    t.push(vec!["x".to_string(), "1".to_string()]);
    t.push(vec!["── section ──".to_string()]);
    t.push(vec!["y".to_string(), "2".to_string()]);
    let out = t.to_string();

    for fragment in ["x", "section", "y"] {
        assert!(out.contains(fragment), "row missing: {out}");
    }
    // `┴` junctions above the span row, plus a seam below it.
    assert!(out.contains('\u{2534}'), "span junction missing:\n{out}");
    let seams = out.lines().filter(|l| l.starts_with('\u{251c}')).count();
    // header separator + the span row's upper border + its lower border
    assert_eq!(seams, 3, "span-row borders missing:\n{out}");
}
