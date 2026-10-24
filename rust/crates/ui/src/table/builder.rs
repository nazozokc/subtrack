use super::ansi::colorize_lines;
use super::text::{multi_line_wrap, pad, repeat, strlen, truncate};
use super::theme::{TABLE_CHARS, TableStyle};

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
    /// When false (the default), the border between two consecutive regular
    /// rows is skipped: the zebra bands carry the row rhythm instead, and the
    /// table stays half as tall. The header separator, span-row borders, and
    /// the outer frame are always drawn.
    pub row_dividers: bool,
    rows: Vec<Vec<String>>,
}

/// A row with a single cell when the table has multiple columns is treated
/// as a full-width `colSpan` row (used by compare's divider rows).
fn is_span_row(row: &[String], col_count: usize) -> bool {
    row.len() == 1 && col_count > 1
}

/// Inner width of a full-width span row: sum of column widths plus the
/// separator characters between columns.
fn span_inner_width(widths: &[i64]) -> i64 {
    widths.iter().sum::<i64>() + (widths.len() as i64 - 1)
}

/// Port of the TS class's `toString()` — `.to_string()` via `Display`.
impl std::fmt::Display for TableBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.render())
    }
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
            row_dividers: false,
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

    /// Whether `all_rows[y]` (head-relative) is a full-width span row.
    fn is_span_row_at(&self, y: usize, col_count: usize) -> bool {
        let rows_index = if self.head.is_empty() {
            y
        } else if y == 0 {
            return false; // the header row is never a span row
        } else {
            y - 1
        };
        match self.rows.get(rows_index) {
            Some(r) => is_span_row(r, col_count),
            None => false,
        }
    }

    pub fn push(&mut self, row: Vec<String>) {
        self.rows.push(row)
    }

    /// Switch the top border to the footer variant (├ ┴ ┤), used by
    /// `renderUsageTable`'s TOTAL footer (mirror of TABLE_CHARS_FOOTER).
    pub fn footer_top(&mut self) {
        for (k, v) in &mut self.chars {
            match *k {
                "top-left" => *v = '\u{251c}',
                "top-mid" => *v = '\u{2534}',
                "top-right" => *v = '\u{2524}',
                _ => {}
            }
        }
    }

    pub fn render(&self) -> String {
        let mut all_rows: Vec<&Vec<String>> = Vec::new();
        if !self.head.is_empty() {
            all_rows.push(&self.head)
        }
        for r in &self.rows {
            all_rows.push(r)
        }
        if all_rows.is_empty() {
            return String::new();
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
                if is_span_row(all_rows[y], col_count) {
                    return 1;
                }
                (0..all_rows[y].len())
                    .map(|x| cell_lines[y][x].len())
                    .max()
                    .unwrap_or(1)
                    .max(1)
            })
            .collect();

        let mut out: Vec<String> = Vec::new();
        for (y, height) in heights.iter().enumerate() {
            let is_top = y == 0;
            let is_header_sep = y == 1 && !self.head.is_empty();
            let beside_span = self.is_span_row_at(y, col_count)
                || (y >= 1 && self.is_span_row_at(y - 1, col_count));
            // Only the border above a row is optional: skip it at plain
            // seams (between two consecutive regular rows) in compact mode,
            // but draw it at every structural one — the top frame, the
            // header separator, span-row junctions, and when full row
            // dividers are requested. The bottom frame is drawn after the
            // loop, so the last row still closes cleanly.
            if self.row_dividers || is_top || is_header_sep || beside_span {
                out.push(self.draw_border(&widths, y, false));
            }
            // The row's own content lines are always emitted — skipping the
            // border must never swallow the data beneath it.
            for line_num in 0..*height {
                out.push(self.draw_line(&cell_lines, &widths, y, line_num, col_count))
            }
        }
        out.push(self.draw_border(&widths, all_rows.len() - 1, true));
        out.join("\n")
    }

    fn draw_border(&self, widths: &[i64], row_index: usize, bottom: bool) -> String {
        let top_row = !bottom && row_index == 0;
        let line_char = if bottom {
            "bottom"
        } else if top_row {
            "top"
        } else {
            "mid"
        };
        let cols = widths.len();
        // A mid border drawn directly above a full-width span row uses
        // `┴` junctions (mirror of cli-table3 colSpan row borders).
        let above_span = !bottom && !top_row && self.is_span_row_at(row_index, cols);

        let mut out = String::new();
        for (x, width) in widths.iter().enumerate() {
            if bottom {
                out.push(if x == 0 {
                    self.char("bottom-left")
                } else {
                    self.char("bottom-mid")
                })
            } else if x == 0 {
                out.push(if top_row {
                    self.char("top-left")
                } else {
                    self.char("left-mid")
                })
            } else {
                out.push(if top_row {
                    self.char("top-mid")
                } else if above_span {
                    '\u{2534}'
                } else {
                    self.char("mid-mid")
                })
            }
            out.push_str(&repeat(
                &self.char(line_char).to_string(),
                (*width).max(0) as usize,
            ))
        }
        out.push(if bottom {
            self.char("bottom-right")
        } else if top_row {
            self.char("top-right")
        } else {
            self.char("right-mid")
        });

        if !self.style.border[0].is_empty() {
            return format!("{}{}{}", self.style.border[0], out, self.style.border[1]);
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

        // Full-width span row: single cell stretched across all columns,
        // separator characters included in the pad width (no padding).
        if cell_lines[y].len() == 1 && col_count > 1 {
            let colorize = |s: String| -> String {
                if !self.style.border[0].is_empty() {
                    format!("{}{}{}", self.style.border[0], s, self.style.border[1])
                } else {
                    s
                }
            };
            let line = cell_lines[y][0].get(line_num).cloned().unwrap_or_default();
            let span_width = span_inner_width(widths);
            let content = pad(
                &line,
                span_width - padding_left - padding_right,
                ' ',
                "left",
            );
            out.push_str(&colorize(self.char("left").to_string()));
            out.push_str(&" ".repeat(padding_left as usize));
            out.push_str(&content);
            out.push_str(&" ".repeat(padding_right as usize));
            out.push_str(&colorize(self.char("right").to_string()));
            return out;
        }

        for (x, col_width) in widths.iter().enumerate().take(col_count) {
            let content_width = *col_width - padding_left - padding_right;
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
            let left = colorize(
                self.char(if x == 0 { "left" } else { "middle" })
                    .to_string(),
            );
            let padding = " ".repeat(padding_left as usize);
            let right_padding = " ".repeat(padding_right as usize);
            let right = if x + 1 == col_count {
                colorize(self.char("right").to_string())
            } else {
                String::new()
            };

            let mut formatted = format!("{}{}{}", padding, content, right_padding);
            if is_head && !self.style.head[0].is_empty() {
                formatted = format!("{}{}{}", self.style.head[0], formatted, self.style.head[1])
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

/// Calculate column widths that fit the terminal width (mirror of
/// display-constants.ts calcColumnWidths).
///
/// The table border consumes one cell per column plus a closing border
/// (`cols + 1`); the remaining width is distributed across columns.
pub fn calc_column_widths(
    rows: &[Vec<String>],
    config: &ColumnConfig,
    term_width: i64,
    table_min_width: i64,
) -> Vec<i64> {
    let cols = config.headers.len() as i64;
    let target = if term_width > 0 {
        term_width
    } else {
        config.min_avail.unwrap_or(table_min_width.max(40)).max(1)
    };
    let avail = (target - cols - 1).max(1);

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

    // Adjust to exactly fit avail. When the terminal is too narrow to honor
    // every minimum, shrink below it instead of overflowing the screen.
    let sum: i64 = widths.iter().sum();
    let mut diff = sum - avail;
    let mut iterations = 0;

    while diff > 0 && iterations < 1000 {
        let mut idx: Option<usize> = None;
        for i in 0..widths.len() {
            if widths[i] > 1 && (idx.is_none() || widths[i] > widths[idx.unwrap()]) {
                idx = Some(i)
            }
        }
        let Some(i) = idx else { break };
        widths[i] -= 1;
        diff -= 1;
        iterations += 1
    }

    iterations = 0;
    while diff < 0 && iterations < 1000 {
        let mut idx: Option<usize> = None;
        for i in 0..widths.len() {
            if widths[i] < config.max_widths[i]
                && (idx.is_none() || weights[i] > weights[idx.unwrap()])
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
