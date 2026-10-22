//! `search` command — port of `commands/core.ts` (searchCommand) + `search.ts`.

use crate::db::search_subscriptions;
use crate::display::spread_subscription;
use crate::json::list_json;
use crate::logger::{self, info, out_str};
use crate::parser::ParsedArgs;

pub fn run_search(parsed: &ParsedArgs) -> i32 {
    let json = parsed.flag("json");

    // Interactive prompt if no query provided; non-TTY sessions skip it
    // (bare `subtrack search` prints only the banner, matching the golden).
    let query = match parsed.str("query") {
        Some(q) => q,
        None if !json && crate::prompt::interactive() => {
            match crate::prompt::input("Search query", None, None) {
                Some(q) if !q.trim().is_empty() => q,
                _ => return 0,
            }
        }
        None => {
            if json {
                out_str("[]\n");
            } else {
                logger::info("Provide a search query: subtrack search <query>");
            }
            return 0;
        }
    };

    let names = parsed.flag("names");
    let notes = parsed.flag("notes");
    let tags = parsed.flag("tags");

    let results = match search_subscriptions(&query, names, notes, tags) {
        Ok(v) => v,
        Err(e) => {
            logger::fail(&format!("Search failed: {}", e));
            return 1;
        }
    };

    if json {
        out_str(&format!("{}\n", list_json(&results)));
        return 0;
    }

    if results.is_empty() {
        info(&format!("No results for \"{}\"", query));
        return 0;
    }

    info(&format!(
        "Found {} result{} for \"{}\":",
        results.len(),
        if results.len() > 1 { "s" } else { "" },
        query
    ));
    spread_subscription(&results, None, false, false, false, false);
    0
}
