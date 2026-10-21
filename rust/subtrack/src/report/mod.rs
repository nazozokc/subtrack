//! `report` / `yearly` commands — port of `report.ts` (handleReport) + yearly.

mod calc;
mod render;

pub use render::{run_report, run_yearly};
