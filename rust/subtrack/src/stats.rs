//! `stats` command — port of `stats.ts` + `db/stats.ts` (collectStats).

use crate::json::quote_string;
use crate::logger::{self, log};
use crate::parser::ParsedArgs;
use crate::price::format_price;

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
    price_ranges: Vec<PriceRange>,
}

struct PriceRange {
    currency: String,
    min: f64,
    max: f64,
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

    let mut price_ranges: Vec<PriceRange> = Vec::new();
    for (price, currency) in &price_rows {
        match price_ranges.iter_mut().find(|r| &r.currency == currency) {
            Some(range) => {
                range.min = range.min.min(*price);
                range.max = range.max.max(*price);
            }
            None => price_ranges.push(PriceRange {
                currency: currency.clone(),
                min: *price,
                max: *price,
            }),
        }
    }
    price_ranges.sort_by(|a, b| a.currency.cmp(&b.currency));

    let count_of = |sql: &str| -> i64 { conn.query_row(sql, [], |r| r.get(0)).unwrap_or(0) };

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
        price_ranges,
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
    if !stats.price_ranges.is_empty() {
        log("  Active price range:");
        for range in &stats.price_ranges {
            if range.min == range.max {
                log(&format!(
                    "    {}: {}",
                    range.currency,
                    format_price(range.min, &range.currency)
                ));
            } else {
                log(&format!(
                    "    {}: {} – {}",
                    range.currency,
                    format_price(range.min, &range.currency),
                    format_price(range.max, &range.currency)
                ));
            }
        }
    }
    log(&format!(
        "  Database size:   {}",
        format_bytes(stats.db_size_bytes)
    ));
    0
}

/// Mirror of `sectionTitle`: bold + accent FG around `── text ──`, else plain.
fn section_title(theme: &crate::table::Theme, text: &str) -> String {
    crate::table::section_title(text, theme)
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
    let mut ranges = serde_json::Map::new();
    for range in &s.price_ranges {
        ranges.insert(
            range.currency.clone(),
            serde_json::json!({
                "min": crate::json::num_value(range.min),
                "max": crate::json::num_value(range.max),
            }),
        );
    }
    f.push(kv(
        "priceRanges",
        serde_json::to_string(&ranges).unwrap_or_else(|_| "{}".to_string()),
    ));
    format!("{{\n  {}\n}}", f.join(",\n  "))
}
