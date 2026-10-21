//! `audit` command — port of `audit.ts` (handleAuditList / handleAuditPrune).

use crate::ansi;
use crate::db::{self};
use crate::json::quote_string;
use crate::logger::{self, info, log};
use crate::parser::ParsedArgs;
use crate::table::{
    ColumnConfig, TableBuilder, calc_column_widths, default_theme, table_style, zebra_row,
};

const ACTION_LABELS: &[(&str, &str)] = &[
    ("subscription.add", "Add"),
    ("subscription.edit", "Edit"),
    ("subscription.delete", "Delete"),
    ("subscription.archive", "Archive"),
    ("subscription.unarchive", "Unarchive"),
    ("subscription.restore", "Restore"),
    ("subscription.import", "Import"),
    ("subscription.bulk_status", "Bulk Status"),
    ("subscription.bulk_delete", "Bulk Delete"),
    ("subscription.bulk_tag_add", "Bulk Tag+"),
    ("subscription.bulk_tag_remove", "Bulk Tag-"),
    ("subscription.clone", "Clone"),
    ("subscription.pause", "Pause"),
    ("subscription.resume", "Resume"),
    ("subscription.renew", "Renew"),
    ("trial.add", "Trial Add"),
    ("trial.delete", "Trial Delete"),
    ("tag.rename", "Tag Rename"),
    ("tag.delete", "Tag Delete"),
    ("tag.prune", "Tag Prune"),
    ("tag.merge", "Tag Merge"),
    ("config.set", "Config Set"),
    ("config.reset", "Config Reset"),
    ("backup.restore", "Restore"),
    ("usage.add", "Usage Add"),
    ("usage.delete", "Usage Delete"),
    ("cleanup", "Cleanup"),
    ("suggestion.receipt", "Receipt"),
    ("template.add", "Template Add"),
    ("template.edit", "Template Edit"),
    ("template.delete", "Template Delete"),
    ("template.use", "Template Use"),
];

fn format_action(action: &str) -> String {
    ACTION_LABELS
        .iter()
        .find(|(k, _)| *k == action)
        .map(|(_, v)| v.to_string())
        .unwrap_or_else(|| action.to_string())
}

/// "2026-07-05 12:34:56" → "Jul 05 12:34" (UTC).
fn format_timestamp(ts: &str) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    if ts.len() < 16 {
        return ts.to_string();
    }
    let month: usize = ts[5..7].parse().unwrap_or(1);
    let day = &ts[8..10];
    let time = &ts[11..16];
    format!(
        "{} {} {}",
        MONTHS.get(month - 1).copied().unwrap_or("Jan"),
        day,
        time
    )
}

pub fn run_audit(parsed: &ParsedArgs, depth: usize, path: &[String]) -> i32 {
    // `audit list` or bare `audit` both render the list (depth keeps parity).
    let _ = depth;
    let action = parsed.str("action").filter(|a| !a.is_empty());
    let limit = parsed.str("limit").and_then(|l| l.parse::<i64>().ok());
    let json = parsed.bool("json") == Some(true);
    let from = parsed.str("from").filter(|a| !a.is_empty());
    let to = parsed.str("to").filter(|a| !a.is_empty());

    if path.last().map(|s| s.as_str()) == Some("prune") && path.len() >= 2 {
        return run_audit_prune(parsed);
    }

    list(&action, limit, json, &from, &to);
    0
}

fn list(
    action: &Option<String>,
    limit: Option<i64>,
    json: bool,
    from: &Option<String>,
    to: &Option<String>,
) {
    let entries = db::get_audit_logs(
        action.as_deref(),
        limit.unwrap_or(50),
        0,
        from.as_deref(),
        to.as_deref(),
    );
    let total = db::get_audit_log_count(action.as_deref(), from.as_deref(), to.as_deref());

    if json {
        let mut out = String::from("[\n");
        for (i, e) in entries.iter().enumerate() {
            out.push_str("  {\n");
            out.push_str(&format!("    \"id\": {},\n", e.id));
            out.push_str(&format!("    \"action\": {},\n", quote_string(&e.action)));
            out.push_str(&format!(
                "    \"targetType\": {},\n",
                e.target_type
                    .as_deref()
                    .map(quote_string)
                    .unwrap_or_else(|| "null".to_string())
            ));
            out.push_str(&format!(
                "    \"targetId\": {},\n",
                e.target_id
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "null".to_string())
            ));
            out.push_str(&format!(
                "    \"details\": {},\n",
                e.details
                    .as_deref()
                    .map(quote_string)
                    .unwrap_or_else(|| "null".to_string())
            ));
            out.push_str(&format!(
                "    \"createdAt\": {}\n",
                quote_string(&e.created_at)
            ));
            out.push_str(if i + 1 == entries.len() {
                "  }\n"
            } else {
                "  },\n"
            });
        }
        out.push_str("]\n");
        logger::out_str(&out);
        return;
    }

    if entries.is_empty() {
        info("No audit log entries found");
        return;
    }

    let min_widths: Vec<i64> = vec![6, 14, 18, 30];
    let max_widths: Vec<i64> = vec![8, 20, 30, 80];
    let rows: Vec<Vec<String>> = entries
        .iter()
        .map(|e| {
            vec![
                e.id.to_string(),
                format_action(&e.action),
                e.target_type
                    .as_deref()
                    .map(|t| {
                        e.target_id
                            .map(|id| format!("{} #{}", t, id))
                            .unwrap_or_else(|| t.to_string())
                    })
                    .unwrap_or_default(),
                e.details
                    .as_deref()
                    .map(|d| {
                        if d.len() > 60 {
                            format!("{}...", &d[..57])
                        } else {
                            d.to_string()
                        }
                    })
                    .unwrap_or_default(),
            ]
        })
        .collect();

    let col_widths = calc_column_widths(
        &rows,
        &ColumnConfig {
            headers: vec![
                "ID".to_string(),
                "Action".to_string(),
                "Target".to_string(),
                "Details".to_string(),
            ],
            min_widths,
            max_widths,
            min_avail: None,
        },
        crate::ui::term_width(),
        40,
    );

    let theme = default_theme();
    let mut table = TableBuilder::new(table_style(&theme));
    table.col_widths = col_widths;
    table.head = vec![
        "ID".to_string(),
        "Action".to_string(),
        "Target".to_string(),
        "Details".to_string(),
    ];
    table.col_aligns = vec![
        "right".to_string(),
        "left".to_string(),
        "left".to_string(),
        "left".to_string(),
    ];

    let raw: Vec<Vec<String>> = rows.clone();
    for (i, row) in raw.iter().enumerate() {
        let action = &row[1];
        let target = &row[2];
        let details = &row[3];
        let ts = format_timestamp(&entries[i].created_at);
        let cells = vec![
            row[0].clone(),
            action.clone(),
            target.clone(),
            format!("{} {}", ts, details),
        ];
        if i % 2 == 0 {
            table.push(zebra_row(&cells, theme.zebra));
        } else {
            table.push(cells);
        }
    }

    log(&table.to_string());
    log(&ansi::dim(&format!(
        "  {} total entries · showing {}",
        total,
        entries.len()
    )));
}

fn run_audit_prune(parsed: &ParsedArgs) -> i32 {
    let days = parsed
        .str("days")
        .and_then(|d| d.parse::<i64>().ok())
        .unwrap_or(90);
    let force = parsed.bool("force") == Some(true);
    let before = (chrono::Utc::now() - chrono::Duration::days(days))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    let count = db::count_audit_before(&before);
    if count == 0 {
        info(&format!("No audit log entries older than {} days", days));
        return 0;
    }
    if !force {
        logger::warn(&format!(
            "This will delete {} audit log entr{} older than {} days.\n  Use --force to proceed.",
            count,
            if count != 1 { "ies" } else { "y" },
            days
        ));
        return 0;
    }
    let conn = match db::open_db() {
        Ok(c) => c,
        Err(e) => {
            logger::fail(&e.to_string());
            return 1;
        }
    };
    let deleted = db::prune_audit_logs(&conn, &before, true).unwrap_or(0);
    logger::success(&format!(
        "Pruned {} audit log entr{}",
        deleted,
        if deleted != 1 { "ies" } else { "y" }
    ));
    0
}
