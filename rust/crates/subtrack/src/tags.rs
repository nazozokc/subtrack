//! `tags` / `tag list` commands — port of `commands/tag.ts` (tagsCommand,
//! tagListCmd) + `subscription/core.ts` handleTags + `tag.ts` handleTagList.

use crate::db::{QueryOptions, get_subscriptions, get_tags_with_count};
use crate::display::spread_subscription;
use crate::logger::{self, fail, info, out_str};
use crate::parser::ParsedArgs;

/// `subtrack tags <names...>` — AND-filtered subscription list.
pub fn run_tags(parsed: &ParsedArgs) -> i32 {
    let names = parsed.str_array("names").unwrap_or_default();
    if names.is_empty() {
        fail("Please specify at least one tag");
        return 1;
    }
    let list = match get_subscriptions(&QueryOptions {
        tags: names.clone(),
        include_archived: true,
        ..Default::default()
    }) {
        Ok(v) => v,
        Err(e) => {
            logger::fail(&format!("Failed to load subscriptions: {}", e));
            return 1;
        }
    };
    spread_subscription(&list, None, false, false, false, false);
    0
}

/// `subtrack tag list [--sort name|count]` — name + usage count.
pub fn run_tag_list(parsed: &ParsedArgs) -> i32 {
    let sort = parsed.str("sort");
    if let Some(s) = &sort
        && s != "name"
        && s != "count"
    {
        fail("sort must be one of: name, count");
        return 1;
    }
    let mut tags = match get_tags_with_count() {
        Ok(v) => v,
        Err(e) => {
            logger::fail(&format!("Failed to load tags: {}", e));
            return 1;
        }
    };
    if sort.as_deref() == Some("count") {
        tags.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    } else {
        tags.sort_by(|a, b| a.0.cmp(&b.0));
    }
    if tags.is_empty() {
        info("No tags found");
        return 0;
    }
    let max_name_len = tags.iter().map(|(n, _)| n.len()).max().unwrap_or(0).max(4);
    let mut out = String::new();
    out.push_str(&format!(
        "{}{}\n",
        "Name".pad_end(max_name_len),
        "  Subscriptions"
    ));
    out.push_str(&format!("{}\n", "─".repeat(max_name_len + 14)));
    for (name, count) in &tags {
        out.push_str(&format!("{}  {}\n", name.pad_end(max_name_len), count));
    }
    out_str(&out);
    0
}

trait PadEnd {
    fn pad_end(&self, len: usize) -> String;
}

impl PadEnd for str {
    fn pad_end(&self, len: usize) -> String {
        let mut s = self.to_string();
        while s.len() < len {
            s.push(' ')
        }
        s
    }
}
/// Port of `handleTagDelete`.
pub fn run_tag_delete(parsed: &ParsedArgs) -> i32 {
    let name = parsed.str("name").unwrap_or_default();
    if name.is_empty() {
        fail("Usage: subtrack tag delete <name>");
        return 1;
    }
    let conn = match crate::db::open_db() {
        Ok(c) => c,
        Err(e) => {
            fail(&format!("Failed to delete tag: {}", e));
            return 1;
        }
    };
    match crate::db::remove_tag(&conn, &name, true) {
        Ok(true) => {
            let _ = crate::db::add_audit_log(
                &conn,
                "tag.delete",
                None,
                None,
                Some(&format!("\"{}\"", name)),
                true,
            );
            logger::success(&format!("Deleted tag: \"{}\"", name));
            0
        }
        Ok(false) => {
            fail(&format!("Tag \"{}\" not found", name));
            1
        }
        Err(e) => {
            fail(&format!("Failed to delete tag: {}", e));
            1
        }
    }
}

/// Port of `handleTagMerge`.
pub fn run_tag_merge(parsed: &ParsedArgs) -> i32 {
    let source = parsed.str("source").unwrap_or_default();
    let target = parsed.str("target").unwrap_or_default();
    if source.is_empty() || target.is_empty() {
        fail("Usage: subtrack tag merge <source> <target>");
        return 1;
    }
    match crate::db::rename_tag(&source, &target) {
        Ok(true) => {
            if let Ok(conn) = crate::db::open_db() {
                let _ = crate::db::save_db(&conn);
                let _ = crate::db::add_audit_log(
                    &conn,
                    "tag.merge",
                    None,
                    None,
                    Some(&format!("\"{}\" → \"{}\"", source, target)),
                    true,
                );
            }
            logger::success(&format!("Merged tag: \"{}\" → \"{}\"", source, target));
            0
        }
        Ok(false) => {
            fail(&format!("Tag \"{}\" not found", source));
            1
        }
        Err(e) => {
            fail(&format!("Failed to merge tag: {}", e));
            1
        }
    }
}

/// Port of `handleTagPrune`.
pub fn run_tag_prune(_parsed: &ParsedArgs) -> i32 {
    let conn = match crate::db::open_db() {
        Ok(c) => c,
        Err(e) => {
            fail(&format!("Failed to prune tags: {}", e));
            return 1;
        }
    };
    match crate::db::prune_tags(&conn, true) {
        Ok(0) => {
            info("No orphaned tags found");
            0
        }
        Ok(n) => {
            let _ = crate::db::add_audit_log(
                &conn,
                "tag.prune",
                None,
                None,
                Some(&format!("{} orphaned tags removed", n)),
                true,
            );
            logger::success(&format!(
                "Removed {} orphaned tag{}",
                n,
                if n > 1 { "s" } else { "" }
            ));
            0
        }
        Err(e) => {
            fail(&format!("Failed to prune tags: {}", e));
            1
        }
    }
}
pub fn run_tag_rename(parsed: &ParsedArgs) -> i32 {
    let old_name = parsed.str("old").unwrap_or_default();
    let new_name = parsed.str("new").unwrap_or_default();
    if old_name.is_empty() || new_name.is_empty() {
        fail("Usage: subtrack tag rename <old> <new>");
        return 1;
    }
    match crate::db::rename_tag(&old_name, &new_name) {
        Ok(true) => {
            crate::logger::success(&format!("Renamed tag: \"{}\" → \"{}\"", old_name, new_name));
            0
        }
        Ok(false) => {
            fail(&format!("Tag \"{}\" not found", old_name));
            1
        }
        Err(e) => {
            fail(&format!("Failed to rename tag: {}", e));
            1
        }
    }
}
