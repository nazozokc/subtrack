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
    v.len() == 3
        && v.bytes().all(|b| b.is_ascii_uppercase())
        && CURRENCY_VALUES.contains(&v)
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
    let n: f64 = v.trim().parse().map_err(|_| "Please enter a valid non-negative number".to_string())?;
    if !n.is_finite() || n < 0.0 {
        return Err("Please enter a valid non-negative number".to_string());
    }
    if n > 99999999.0 {
        return Err("Price too high (max 99,999,999)".to_string());
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
