//! `bulk` commands — port of the non-interactive error path of
//! `bulk.ts` (`handleBulkStatus`).

use crate::logger;
use crate::parser::ParsedArgs;
use crate::validation::is_valid_status;

/// Port of `handleBulkStatus`.
pub fn run_bulk_status(parsed: &ParsedArgs) -> i32 {
    let target = parsed.str("set").unwrap_or_default();
    if target.is_empty() || !is_valid_status(&target) {
        logger::fail(&format!(
            "Invalid status \"{}\". Must be active, paused, or cancelled",
            target
        ));
        return 1;
    }

    // Filtering + status updates are deferred to the mutating phase.
    0
}
