//! Self-contained table renderer — port of `@subtrack/lib/table.ts`
//! plus the width/theme logic from `packages/subtrack/src/display-constants.ts`
//! (calcColumnWidths, TABLE_CHARS, themes).

mod ansi;
mod builder;
mod text;
mod theme;

#[cfg(test)]
mod tests;

pub use builder::{ColumnConfig, TableBuilder, calc_column_widths};
pub use text::strlen;
pub use theme::{
    Theme, default_theme, divider, section_title, status_color, table_style, zebra_row,
};
