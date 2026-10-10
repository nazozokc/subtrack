//! `stats` command — port of `stats.ts` + `db/stats.ts` (collectStats).

use crate::json::{number, quote_string};
use crate::logger::{self, log};
use crate::parser::ParsedArgs;

/// Mirrors `formatBytes` in `@subtrack/lib/format`.
fn format_bytes(bytes: u64) -> String {
    let units = ["B", "kB", "MB", "GB"];
    let mut i = 0usize;
    let mut size = bytes as f64;
    while size >= 1024.0 && i < units.len() - 1 {
        size /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{:.0} {}", size, units[i])
    } else {
        format!("{:.1} {}", size, units[i])
    }
}

struct Stats {
    total: i64,
    active: i64,
    paused: i64,
    cancelled: i64,
    archived: i64,
    total_tags: i64,
    total_trials: i64,
    total_usage: i64,
    db_size_bytes: u64,
    price_min: f64,
    price_max: f64,
    currencies: Vec<String>,
}

fn collect_stats() -> std::io::Result<Stats> {
    let conn = crate::db::open_db()?;

    let mut status_rows: Vec<(String, i64)> = Vec::new();
    {
        let mut stmt = conn
            .prepare("SELECT status, COUNT(*) AS count FROM subscriptions GROUP BY status")
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        for r in rows {
            status_rows.push(r.map_err(|e| std::io::Error::other(e.to_string()))?);
        }
    }
    let get = |name: &str| -> i64 {
        status_rows
            .iter()
            .find(|(s, _)| s == name)
            .map(|(_, c)| *c)
            .unwrap_or(0)
    };

    let mut price_rows: Vec<(f64, String)> = Vec::new();
    {
        let mut stmt = conn
            .prepare("SELECT price, currency FROM subscriptions WHERE status = 'active'")
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, f64>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        for r in rows {
            price_rows.push(r.map_err(|e| std::io::Error::other(e.to_string()))?);
        }
    }

    let mut currencies: Vec<String> = Vec::new();
    for (_, c) in &price_rows {
        if !currencies.contains(c) {
            currencies.push(c.clone());
        }
    }

    let count_of = |sql: &str| -> i64 {
        conn.query_row(sql, [], |r| r.get(0)).unwrap_or(0)
    };

    let db_path = crate::crypto::config_dir().join("subtrack.db");
    let db_size_bytes = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

    Ok(Stats {
        total: status_rows.iter().map(|(_, c)| *c).sum(),
        active: get("active"),
        paused: get("paused"),
        cancelled: get("cancelled"),
        archived: get("archived"),
        total_tags: count_of("SELECT COUNT(*) AS count FROM tags"),
        total_trials: count_of("SELECT COUNT(*) AS count FROM trials"),
        total_usage: count_of("SELECT COUNT(*) AS count FROM llm_usage"),
        db_size_bytes,
        price_min: if price_rows.is_empty() {
            0.0
        } else {
            price_rows.iter().map(|(p, _)| *p).fold(f64::INFINITY, f64::min)
        },
        price_max: if price_rows.is_empty() {
            0.0
        } else {
            price_rows
                .iter()
                .map(|(p, _)| *p)
                .fold(f64::NEG_INFINITY, f64::max)
        },
        currencies,
    })
}

pub fn run_stats(parsed: &ParsedArgs) -> i32 {
    let json = parsed.bool("json") == Some(true);
    let stats = match collect_stats() {
        Ok(s) => s,
        Err(e) => {
            logger::fail(&format!("Failed to collect statistics: {}", e));
            return 1;
        }
    };

    if json {
        logger::out_str(&stats_json(&stats));
        logger::out_str("\n");
        return 0;
    }

    let theme = crate::config::display_theme();
    log(&section_title(&theme, "Database Statistics"));
    log(&format!("  Subscriptions: {}", stats.total));
    log(&format!("    Active:       {}", stats.active));
    log(&format!("    Paused:       {}", stats.paused));
    log(&format!("    Cancelled:    {}", stats.cancelled));
    log(&format!("    Archived:     {}", stats.archived));
    log(&format!("  Tags:           {}", stats.total_tags));
    log(&format!("  Trials:         {}", stats.total_trials));
    log(&format!("  LLM Usage entries: {}", stats.total_usage));
    if !stats.currencies.is_empty() {
        log("  Active price range:");
        log(&format!(
            "    Min:  {} ({})",
            number(stats.price_min),
            stats.currencies.join(", ")
        ));
        log(&format!(
            "    Max:  {} ({})",
            number(stats.price_max),
            stats.currencies.join(", ")
        ));
    }
    log(&format!("  Database size:   {}", format_bytes(stats.db_size_bytes)));
    0
}

/// Mirror of `sectionTitle`: bold + accent FG around `── text ──`, else plain.
fn section_title(theme: &crate::table::Theme, text: &str) -> String {
    match theme.accent {
        Some(accent) => format!(
            "\x1b[1m{}── {} ──\x1b[0m",
            crate::ansi::fg_code(accent),
            text
        ),
        None => format!("── {} ──", text),
    }
}

fn stats_json(s: &Stats) -> String {
    let mut f: Vec<String> = Vec::new();
    let kv = |k: &str, v: String| format!("{}: {}", quote_string(k), v);
    f.push(kv("total", s.total.to_string()));
    f.push(kv("active", s.active.to_string()));
    f.push(kv("paused", s.paused.to_string()));
    f.push(kv("cancelled", s.cancelled.to_string()));
    f.push(kv("archived", s.archived.to_string()));
    f.push(kv("totalTags", s.total_tags.to_string()));
    f.push(kv("totalTrials", s.total_trials.to_string()));
    f.push(kv("totalUsage", s.total_usage.to_string()));
    f.push(kv("dbSizeBytes", s.db_size_bytes.to_string()));
    let price_range = format!(
        "{{\n    \"min\": {},\n    \"max\": {},\n    \"currencies\": [\n{}\n    ]\n  }}",
        number(s.price_min),
        number(s.price_max),
        s.currencies
            .iter()
            .map(|c| format!("      {}", quote_string(c)))
            .collect::<Vec<_>>()
            .join(",\n")
    );
    f.push(kv("priceRange", price_range));
    format!("{{\n  {}\n}}", f.join(",\n  "))
}
