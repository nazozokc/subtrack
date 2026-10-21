//! Pure input validators and choice constants — port of
//! `packages/subtrack/src/validation.ts`.
//!
//! Returns `Result<(), String>` where `Err` carries the exact user-facing
//! message that the TS validators return as a string.

use crate::date::cycle_days;

/// Currency choice values (`CURRENCY_CHOICES.map(c => c.value)`).
pub const CURRENCY_VALUES: [&str; 37] = [
    "AED", "ARS", "AUD", "BRL", "CAD", "CHF", "CLP", "CNY", "COP", "CZK", "DKK", "EGP", "EUR",
    "GBP", "HKD", "HUF", "IDR", "ILS", "INR", "JPY", "KRW", "MXN", "MYR", "NGN", "NOK", "NZD",
    "PHP", "PLN", "SAR", "SEK", "SGD", "THB", "TRY", "TWD", "USD", "VND", "ZAR",
];

/// Status choice values.
pub const STATUS_VALUES: [&str; 4] = ["active", "paused", "cancelled", "archived"];

/// Named cycle choice values.
pub const CYCLE_VALUES: [&str; 6] = [
    "weekly",
    "bi-weekly",
    "monthly",
    "quarterly",
    "semi-annual",
    "yearly",
];

/// Port of `isValidCurrency` (uppercase 3-letter code within the choice list).
pub fn is_valid_currency(v: &str) -> bool {
    v.len() == 3 && v.bytes().all(|b| b.is_ascii_uppercase()) && CURRENCY_VALUES.contains(&v)
}

/// Port of `isValidCycle` (named preset, or an `Nd` day cycle).
pub fn is_valid_cycle(v: &str) -> bool {
    CYCLE_VALUES.contains(&v) || cycle_days(v).is_some()
}

/// Port of `isValidStatus`.
pub fn is_valid_status(v: &str) -> bool {
    STATUS_VALUES.contains(&v)
}

/// Mirror of the private `isValidIsoDate` helper.
pub fn is_valid_iso_date(v: &str) -> bool {
    chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d")
        .map(|d| d.format("%Y-%m-%d").to_string() == v)
        .unwrap_or(false)
}

pub fn validate_name(v: &str) -> Result<(), String> {
    if v.trim().is_empty() {
        return Err("Name cannot be empty".to_string());
    }
    if v.chars().count() > 100 {
        return Err("Name too long (max 100 chars)".to_string());
    }
    Ok(())
}

pub fn validate_price(v: &str) -> Result<(), String> {
    if v.trim().is_empty() {
        return Err("Please enter a valid number".to_string());
    }
    let n: f64 = v
        .trim()
        .parse()
        .map_err(|_| "Please enter a valid non-negative number".to_string())?;
    if !n.is_finite() || n < 0.0 {
        return Err("Please enter a valid non-negative number".to_string());
    }
    if n > 99999999.0 {
        return Err("Price too high (max 99,999,999)".to_string());
    }
    Ok(())
}

/// Port of `validateCycleDays` (custom `Nd` cycles).
pub fn validate_cycle_days(v: &str) -> Result<(), String> {
    if v.trim().is_empty() {
        return Err("Days per cycle cannot be empty".to_string());
    }
    let n: f64 = v
        .trim()
        .parse()
        .map_err(|_| "Enter a number between 1 and 365".to_string())?;
    if n.fract() != 0.0 || !(1.0..=365.0).contains(&n) {
        return Err("Enter a number between 1 and 365".to_string());
    }
    Ok(())
}

pub fn validate_billing_day(v: &str) -> Result<(), String> {
    if v.trim().is_empty() {
        return Ok(());
    }
    let n: f64 = v
        .trim()
        .parse()
        .map_err(|_| "Enter a number between 1 and 31".to_string())?;
    if n.fract() != 0.0 || n < 1.0 || n > 31.0 {
        return Err("Enter a number between 1 and 31".to_string());
    }
    Ok(())
}

pub fn validate_model_name(v: &str) -> Result<(), String> {
    if v.trim().is_empty() {
        return Err("Model name cannot be empty".to_string());
    }
    if v.chars().count() > 200 {
        return Err("Model name too long (max 200 chars)".to_string());
    }
    Ok(())
}

pub fn validate_tokens(v: &str) -> Result<(), String> {
    if v.trim().is_empty() {
        return Err("Please enter a number".to_string());
    }
    let n: i64 = v
        .trim()
        .parse()
        .map_err(|_| "Please enter a non-negative integer".to_string())?;
    if n < 0 {
        return Err("Please enter a non-negative integer".to_string());
    }
    if n > 9_999_999_999 {
        return Err("Number too large (max 9,999,999,999)".to_string());
    }
    Ok(())
}

pub fn validate_date(v: &str) -> Result<(), String> {
    if v.trim().is_empty() {
        return Ok(());
    }
    if !is_iso_shape(v) {
        return Err("Use YYYY-MM-DD format".to_string());
    }
    if !is_valid_iso_date(v) {
        return Err("Invalid date".to_string());
    }
    Ok(())
}

pub fn validate_expires_at(v: &str) -> Result<(), String> {
    if v.trim().is_empty() {
        return Err("Expiration date cannot be empty".to_string());
    }
    if !is_iso_shape(v) {
        return Err("Use YYYY-MM-DD format".to_string());
    }
    if !is_valid_iso_date(v) {
        return Err("Invalid date".to_string());
    }
    Ok(())
}

pub fn validate_date_string(v: &str) -> Result<(), String> {
    if v.trim().is_empty() {
        return Ok(());
    }
    if !is_iso_shape(v) {
        return Err("Use YYYY-MM-DD format".to_string());
    }
    if !is_valid_iso_date(v) {
        return Err("Invalid date".to_string());
    }
    Ok(())
}

pub fn validate_notes(v: &str) -> Result<(), String> {
    if v.chars().count() > 500 {
        return Err("Notes too long (max 500 chars)".to_string());
    }
    Ok(())
}

/// Port of `validateTags`.
pub fn validate_tags(v: &str) -> Result<(), String> {
    if v.trim().is_empty() {
        return Ok(());
    }
    let tags: Vec<&str> = v
        .split(',')
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect();
    if tags.len() > 10 {
        return Err("Maximum 10 tags allowed".to_string());
    }
    for tag in tags {
        if tag.chars().count() > 50 {
            return Err(format!("Tag too long: \"{}\" (max 50 chars)", tag));
        }
    }
    Ok(())
}

pub fn validate_payment_method(v: &str) -> Result<(), String> {
    if v.chars().count() > 100 {
        return Err("Payment method too long (max 100 chars)".to_string());
    }
    Ok(())
}

pub fn validate_vendor_name(v: &str) -> Result<(), String> {
    if v.chars().count() > 100 {
        return Err("Vendor name too long (max 100 chars)".to_string());
    }
    Ok(())
}

pub fn validate_vendor_url(v: &str) -> Result<(), String> {
    if v.trim().is_empty() {
        return Ok(());
    }
    if v.chars().count() > 500 {
        return Err("URL too long (max 500 chars)".to_string());
    }
    if !(v.starts_with("http://") || v.starts_with("https://")) {
        return Err("Invalid URL (must start with http:// or https://)".to_string());
    }
    Ok(())
}

pub fn validate_plan_tier(v: &str) -> Result<(), String> {
    if v.chars().count() > 100 {
        return Err("Plan tier too long (max 100 chars)".to_string());
    }
    Ok(())
}

pub fn validate_discount_value(v: &str) -> Result<(), String> {
    if v.trim().is_empty() {
        return Ok(());
    }
    let n: f64 = v
        .trim()
        .parse()
        .map_err(|_| "Please enter a non-negative number".to_string())?;
    if !n.is_finite() || n < 0.0 {
        return Err("Please enter a non-negative number".to_string());
    }
    if n > 99999999.0 {
        return Err("Value too high".to_string());
    }
    Ok(())
}

pub fn validate_discount_type(v: &str) -> Result<(), String> {
    if v.trim().is_empty() {
        return Ok(());
    }
    if v != "percentage" && v != "fixed" {
        return Err("Must be \"percentage\" or \"fixed\"".to_string());
    }
    Ok(())
}

pub fn validate_auto_renewal(v: &str) -> Result<(), String> {
    if v != "true" && v != "false" {
        return Err("Must be \"true\" or \"false\"".to_string());
    }
    Ok(())
}

pub fn validate_trial_name(v: &str) -> Result<(), String> {
    if v.trim().is_empty() {
        return Err("Name cannot be empty".to_string());
    }
    if v.chars().count() > 100 {
        return Err("Name too long (max 100 chars)".to_string());
    }
    Ok(())
}

/// Approximate JavaScript `Number(v)`: empty/whitespace → 0, valid number →
/// value, otherwise `None` (used by config/trial flag validation).
pub fn js_number(v: &str) -> Option<f64> {
    let t = v.trim();
    if t.is_empty() {
        return Some(0.0);
    }
    if let Ok(n) = t.parse::<f64>() {
        return Some(n);
    }
    match t {
        "Infinity" | "+Infinity" => Some(f64::INFINITY),
        "-Infinity" => Some(f64::NEG_INFINITY),
        _ => None,
    }
}

fn is_iso_shape(v: &str) -> bool {
    let b = v.as_bytes();
    b.len() == 10
        && b[0..4].iter().all(|c| c.is_ascii_digit())
        && b[4] == b'-'
        && b[5..7].iter().all(|c| c.is_ascii_digit())
        && b[7] == b'-'
        && b[8..10].iter().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitelists_accept_known_values_only() {
        assert!(is_valid_currency("USD"));
        assert!(!is_valid_currency("XXX"));
        assert!(is_valid_cycle("monthly"));
        assert!(is_valid_cycle("7d"));
        assert!(!is_valid_cycle("workly"));
        assert!(is_valid_status("active"));
        assert!(!is_valid_status("draft"));
    }

    #[test]
    fn iso_dates_use_shape_and_calendar() {
        assert!(is_valid_iso_date("2026-10-08"));
        assert!(is_valid_iso_date("2024-02-29"));
        assert!(!is_valid_iso_date("2026-2-8")); // shape
        assert!(!is_valid_iso_date("2026-02-30")); // calendar
        assert!(!is_valid_iso_date("not-a-date"));
    }

    #[test]
    fn validate_name_rejects_blank_or_oversized() {
        assert!(validate_name("Netflix").is_ok());
        assert_eq!(
            validate_name("   "),
            Err("Name cannot be empty".to_string())
        );
        assert_eq!(
            validate_name(&"x".repeat(101)),
            Err("Name too long (max 100 chars)".to_string())
        );
    }

    #[test]
    fn validate_price_requires_parseable_non_negative() {
        assert!(validate_price("9.99").is_ok());
        assert_eq!(
            validate_price(""),
            Err("Please enter a valid number".to_string())
        );
        assert_eq!(
            validate_price("abc"),
            Err("Please enter a valid non-negative number".to_string())
        );
        assert_eq!(
            validate_price("-1"),
            Err("Please enter a valid non-negative number".to_string())
        );
        assert_eq!(
            validate_price("100000000"),
            Err("Price too high (max 99,999,999)".to_string())
        );
    }

    #[test]
    fn validate_billing_day_bounds_1_to_31() {
        assert!(validate_billing_day("").is_ok()); // optional
        assert!(validate_billing_day("1").is_ok());
        assert!(validate_billing_day("31").is_ok());
        assert_eq!(
            validate_billing_day("0"),
            Err("Enter a number between 1 and 31".to_string())
        );
        assert_eq!(
            validate_billing_day("32"),
            Err("Enter a number between 1 and 31".to_string())
        );
        assert_eq!(
            validate_billing_day("x"),
            Err("Enter a number between 1 and 31".to_string())
        );
    }

    #[test]
    fn validate_dates() {
        assert!(validate_date("").is_ok()); // optional
        assert!(validate_date("2026-10-08").is_ok());
        assert_eq!(
            validate_date("2026/10/08"),
            Err("Use YYYY-MM-DD format".to_string())
        );
        assert_eq!(validate_date("2026-02-30"), Err("Invalid date".to_string()));
        assert_eq!(
            validate_expires_at(""),
            Err("Expiration date cannot be empty".to_string())
        );
        assert_eq!(
            validate_expires_at("2026/10/08"),
            Err("Use YYYY-MM-DD format".to_string())
        );
    }

    #[test]
    fn validate_notes_and_limits() {
        assert!(validate_notes("").is_ok());
        assert_eq!(
            validate_notes(&"x".repeat(501)),
            Err("Notes too long (max 500 chars)".to_string())
        );
        assert_eq!(
            validate_payment_method(&"x".repeat(101)),
            Err("Payment method too long (max 100 chars)".to_string())
        );
        assert_eq!(
            validate_vendor_name(&"x".repeat(101)),
            Err("Vendor name too long (max 100 chars)".to_string())
        );
    }

    #[test]
    fn validate_tags_splits_and_limits() {
        assert!(validate_tags("").is_ok());
        assert!(validate_tags("ai, tools").is_ok());
        assert_eq!(
            validate_tags(&["a"; 11].join(",")),
            Err("Maximum 10 tags allowed".to_string())
        );
        assert_eq!(
            validate_tags(&format!("ok, {}", "b".repeat(51))),
            Err(format!(
                "Tag too long: \"{}\" (max 50 chars)",
                "b".repeat(51)
            ))
        );
    }

    #[test]
    fn validate_vendor_url_requires_scheme() {
        assert!(validate_vendor_url("").is_ok()); // optional
        assert!(validate_vendor_url("https://example.com").is_ok());
        assert!(validate_vendor_url("http://example.com").is_ok());
        assert_eq!(
            validate_vendor_url("ftp://example.com"),
            Err("Invalid URL (must start with http:// or https://)".to_string())
        );
        assert_eq!(
            validate_vendor_url(&format!("https://{}", "x".repeat(500))),
            Err("URL too long (max 500 chars)".to_string())
        );
    }

    #[test]
    fn validate_discount_fields() {
        assert!(validate_discount_value("").is_ok());
        assert!(validate_discount_value("10").is_ok());
        assert_eq!(
            validate_discount_value("abc"),
            Err("Please enter a non-negative number".to_string())
        );
        assert!(validate_discount_type("").is_ok());
        assert!(validate_discount_type("percentage").is_ok());
        assert_eq!(
            validate_discount_type("percent"),
            Err("Must be \"percentage\" or \"fixed\"".to_string())
        );
        assert!(validate_auto_renewal("true").is_ok());
        assert_eq!(
            validate_auto_renewal("1"),
            Err("Must be \"true\" or \"false\"".to_string())
        );
    }

    #[test]
    fn validate_tokens_limits() {
        assert!(validate_tokens("123").is_ok());
        assert_eq!(
            validate_tokens(""),
            Err("Please enter a number".to_string())
        );
        assert_eq!(
            validate_tokens("-1"),
            Err("Please enter a non-negative integer".to_string())
        );
        assert_eq!(
            validate_tokens("10000000000"),
            Err("Number too large (max 9,999,999,999)".to_string())
        );
    }

    #[test]
    fn js_number_mirrors_number_cast() {
        assert_eq!(js_number(""), Some(0.0)); // empty string → 0
        assert_eq!(js_number("1.5"), Some(1.5));
        assert_eq!(js_number("abc"), None);
        assert_eq!(js_number("Infinity"), Some(f64::INFINITY));
        assert_eq!(js_number("-Infinity"), Some(f64::NEG_INFINITY));
    }
}
