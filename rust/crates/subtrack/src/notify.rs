//! `notify` command — port of `notify.ts` (upcoming-bill notifications).

use crate::config::load_config;
use crate::db::{get_subscriptions, QueryOptions, Subscription};
use crate::date::{
    calculate_next_billing_inclusive, format_date, format_short_date, today,
};
use crate::json::quote_string;
use crate::logger::{self, log, out_str};
use crate::parser::ParsedArgs;
use crate::price::format_price;
use chrono::NaiveDate;

/// Port of `calcUpcoming(days)` (upcoming.ts).
fn calc_upcoming(days: i64) -> Vec<(Subscription, NaiveDate)> {
    let subs: Vec<Subscription> = get_subscriptions(&QueryOptions::default())
        .map(|mut v| {
            v.retain(|s| s.status != "cancelled");
            v
        })
        .unwrap_or_default();
    if subs.is_empty() {
        return Vec::new()
    }
    let now = today();
    let end = now + chrono::Duration::days(days);
    let mut entries: Vec<(usize, NaiveDate)> = Vec::new();
    for (idx, sub) in subs.iter().enumerate() {
        let next = calculate_next_billing_inclusive(sub.billing_day, &sub.created_at, &sub.cycle, now);
        if next >= now && next <= end {
            entries.push((idx, next));
        }
    }
    entries.sort_by_key(|e| e.1);
    entries
        .into_iter()
        .map(|(idx, next)| (subs[idx].clone(), next))
        .collect()
}

fn days_label(days: i64) -> String {
    format!("{} day{}", days, if days > 1 { "s" } else { "" })
}

pub fn run_notify(parsed: &ParsedArgs) -> i32 {
    let config = load_config();
    let days = match parsed.str("days") {
        Some(s) => match s.parse::<i64>() {
            Ok(v) if v >= 0 => v,
            _ => {
                logger::fail("days must be a non-negative integer");
                return 1;
            }
        },
        None => config.notify_days,
    };

    let entries = calc_upcoming(days);

    if parsed.bool("json") == Some(true) {
        let mut out = String::from("{\n  \"days\": ");
        out.push_str(&crate::json::number(days as f64));
        out.push_str(&format!(",\n  \"count\": {},\n  \"entries\": [\n", entries.len()));
        for (i, (sub, next)) in entries.iter().enumerate() {
            out.push_str("    {\n");
            out.push_str(&format!("      \"name\": {},\n", quote_string(&sub.name)));
            out.push_str(&format!("      \"price\": {},\n", crate::json::number(sub.price)));
            out.push_str(&format!("      \"currency\": {},\n", quote_string(&sub.currency)));
            out.push_str(&format!("      \"cycle\": {},\n", quote_string(&sub.cycle)));
            out.push_str(&format!("      \"nextDate\": {},\n", quote_string(&format_date(*next))));
            if sub.tags.is_empty() {
                out.push_str("      \"tags\": []\n");
            } else {
                out.push_str("      \"tags\": [\n");
                for (ti, t) in sub.tags.iter().enumerate() {
                    let comma = if ti + 1 == sub.tags.len() { "" } else { "," };
                    out.push_str(&format!("        {}{}\n", quote_string(t), comma));
                }
                out.push_str("      ]\n");
            }
            out.push_str(if i + 1 == entries.len() { "    }\n" } else { "    },\n" });
        }
        out.push_str("  ]\n}\n");
        out_str(&out);
        return 0
    }

    if entries.is_empty() {
        if parsed.bool("dry-run") != Some(true) {
            return 0 // no notification needed
        }
        logger::info(&format!("No upcoming bills in the next {}", days_label(days)));
        return 0
    }

    if parsed.bool("dry-run") == Some(true) {
        logger::info(&format!("Upcoming bills (next {}):", days_label(days)));
        let iso = config.date_format != "short";
        for (sub, next) in &entries {
            let date = if iso {
                format_date(*next)
            } else {
                format_short_date(*next)
            };
            log(&format!(
                "  {}  {}  {}/{}",
                date,
                sub.name,
                format_price(sub.price, &sub.currency),
                sub.cycle
            ));
        }
        return 0
    }

    let channels: Vec<String> = match parsed.str("channel") {
        Some(channel) => vec![channel],
        None => {
            if !config.notify_channels.is_empty() {
                config.notify_channels.clone()
            } else {
                vec!["os".to_string()]
            }
        }
    };

    for channel in &channels {
        match channel.as_str() {
            "os" => send_os_notification(&entries, days),
            "slack" => send_slack_notification(&entries, days, config.slack_webhook.as_deref()),
            "webhook" => send_webhook_notification(&entries, days, config.webhook_url.as_deref()),
            _ => {}
        }
    }
    0
}

fn send_os_notification(
    entries: &[(Subscription, NaiveDate)],
    days: i64,
) {
    let count = entries.len();
    let mut message: String;
    if count <= 5 {
        message = entries
            .iter()
            .map(|(sub, _)| {
                format!("{}: {}/{}", sub.name, format_price(sub.price, &sub.currency), sub.cycle)
            })
            .collect::<Vec<_>>()
            .join("\n");
    } else {
        let shown = &entries[..5];
        message = shown
            .iter()
            .map(|(sub, _)| {
                format!("{}: {}/{}", sub.name, format_price(sub.price, &sub.currency), sub.cycle)
            })
            .collect::<Vec<_>>()
            .join("\n");
        message.push_str(&format!("\n... and {} more", count - 5));
    }
    let title = format!(
        "subtrack: {} upcoming bill{} in {}",
        count,
        if count > 1 { "s" } else { "" },
        days_label(days)
    );
    // linux: notify-send
    let _ = std::process::Command::new("notify-send")
        .args([&title, &message, "-t", "10000"])
        .status();
}

fn send_slack_notification(
    entries: &[(Subscription, NaiveDate)],
    days: i64,
    webhook_url: Option<&str>,
) {
    let Some(url) = webhook_url else {
        logger::warn(
            "Slack notification configured but no slackWebhook set. Use: subtrack config set slackWebhook https://hooks.slack.com/services/...",
        );
        return
    };
    let attachments: Vec<String> = entries
        .iter()
        .map(|(sub, _)| {
            format!(
                "{{\"color\":\"#36a64f\",\"text\":\"{}: {}/{}\"}}",
                escape_json(&sub.name),
                format_price(sub.price, &sub.currency),
                sub.cycle
            )
        })
        .collect();
    let payload = format!(
        "{{\"text\":\"subtrack: *{} upcoming bill{}* in {}\",\"attachments\":[{}]}}",
        entries.len(),
        if entries.len() > 1 { "s" } else { "" },
        days_label(days),
        attachments.join(",")
    );
    match http_post(url, &payload) {
        Some(status) if (200..300).contains(&status) => {
            logger::success("Slack notification sent");
        }
        Some(status) => logger::warn(&format!(
            "Slack webhook returned {}: {}",
            status,
            status_text(status)
        )),
        None => logger::warn("Failed to send Slack notification: fetch failed"),
    }
}

fn send_webhook_notification(
    entries: &[(Subscription, NaiveDate)],
    days: i64,
    webhook_url: Option<&str>,
) {
    let Some(url) = webhook_url else {
        logger::warn(
            "Webhook configured but no webhookUrl set. Use: subtrack config set webhookUrl https://example.com/hook",
        );
        return
    };
    let entries_json: Vec<String> = entries
        .iter()
        .map(|(sub, _)| {
            format!(
                "{{\"name\":\"{}\",\"price\":{},\"currency\":\"{}\",\"cycle\":\"{}\"}}",
                escape_json(&sub.name),
                crate::json::number(sub.price),
                sub.currency,
                sub.cycle
            )
        })
        .collect();
    let payload = format!(
        "{{\"event\":\"upcoming_bills\",\"days\":{},\"count\":{},\"entries\":[{}]}}",
        days,
        entries.len(),
        entries_json.join(",")
    );
    match http_post(url, &payload) {
        Some(status) if status >= 200 && status < 300 => {
            logger::success("Webhook notification sent");
        }
        Some(status) => logger::warn(&format!(
            "Webhook returned {}: {}",
            status,
            status_text(status)
        )),
        None => logger::warn("Failed to send webhook notification: fetch failed"),
    }
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// One-shot HTTP POST via `curl` (no TLS stack in the binary). Returns the
/// HTTP status code, or None on transport failure.
fn http_post(url: &str, payload: &str) -> Option<i64> {
    let out = std::process::Command::new("curl")
        .args([
            "-s", "-o", "/dev/null", "-w", "%{http_code}",
            "-X", "POST",
            "-H", "Content-Type: application/json",
            "--max-time", "10",
            "-d", payload,
            url,
        ])
        .output()
        .ok()?;
    let code: i64 = std::str::from_utf8(&out.stdout).ok()?.trim().parse().ok()?;
    if code == 0 {
        None
    } else {
        Some(code)
    }
}

fn status_text(status: i64) -> &'static str {
    match status {
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Error",
    }
}