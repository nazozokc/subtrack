//! Price formatting — port of `packages/subtrack/src/price.ts`.

/// Round to 2 decimal places (mirror of `roundCurrency`).
pub fn round_currency(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Currency symbol for en-US Intl.NumberFormat "symbol" display.
/// Ambiguous/unknown currencies fall back to `None` → "{CODE} {amount}".
fn symbol_for(currency: &str) -> Option<&'static str> {
    match currency {
        "USD" => Some("$"),
        "JPY" => Some("¥"),
        "EUR" => Some("€"),
        "GBP" => Some("£"),
        "KRW" => Some("₩"),
        "INR" => Some("₹"),
        "BRL" => Some("R$"),
        "CNY" => Some("CN¥"),
        "AUD" => Some("A$"),
        "CAD" => Some("CA$"),
        "HKD" => Some("HK$"),
        "SGD" => Some("S$"),
        "TWD" => Some("NT$"),
        "MXN" => Some("MX$"),
        "TRY" => Some("₺"),
        _ => None,
    }
}

/// Group integer part with "," thousands separators.
pub fn group_whole(n: i64) -> String {
    let neg = n < 0;
    let abs = n.unsigned_abs();
    let digits = abs.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',')
        }
        out.push(c)
    }
    if neg { format!("-{}", out) } else { out }
}

/// Format a price in a currency, mimicking en-US `Intl.NumberFormat`
/// with `minimumFractionDigits: 0, maximumFractionDigits: 2`.
pub fn format_price(price: f64, currency: &str) -> String {
    let cents = (price * 100.0).round() as i64;
    let whole = cents.div_euclid(100);
    let frac = cents.rem_euclid(100);
    let mut amount = group_whole(whole);
    if frac != 0 {
        let mut frac_str = format!("{:02}", frac);
        // Intl drops a single trailing zero: 2.50 → "2.5"
        if frac_str.ends_with('0') {
            frac_str.pop();
        }
        amount.push('.');
        amount.push_str(&frac_str)
    }
    match symbol_for(currency) {
        Some(sym) => format!("{}{}", sym, amount),
        None => format!("{} {}", currency, amount),
    }
}

/// Format USD cost from cents: `$${(cents / 100).toFixed(digits)}`.
pub fn format_usd_cost(cents: i64, digits: usize) -> String {
    let v = cents as f64 / 100.0;
    format!("${:.1$}", v, digits)
}

/// Like [`format_usd_cost`] but preserves fractional cents (LLM API costs).
pub fn format_usd_cost_f(cents: f64, digits: usize) -> String {
    let v = cents / 100.0;
    format!("${:.1$}", v, digits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_currency_rounds_half_away_from_zero() {
        assert_eq!(round_currency(12.987), 12.99);
        assert_eq!(round_currency(12.321), 12.32);
        assert_eq!(round_currency(99.999), 100.0);
        assert_eq!(round_currency(-1.006), -1.01);
        assert_eq!(round_currency(0.0), 0.0);
    }

    #[test]
    fn group_whole_inserts_thousands_separators() {
        assert_eq!(group_whole(0), "0");
        assert_eq!(group_whole(12), "12");
        assert_eq!(group_whole(1234), "1,234");
        assert_eq!(group_whole(1234567), "1,234,567");
        assert_eq!(group_whole(-1234567), "-1,234,567");
    }

    #[test]
    fn format_price_mimics_intl_en_us() {
        // trailing zeros are dropped: 2.50 → "2.5"
        assert_eq!(format_price(12.5, "USD"), "$12.5");
        assert_eq!(format_price(2.0, "USD"), "$2");
        // f64 2.3 truncation still rounds to cents 230 → "2.3"
        assert_eq!(format_price(2.3, "USD"), "$2.3");
        assert_eq!(format_price(12.99, "USD"), "$12.99");
        assert_eq!(format_price(1000.0, "USD"), "$1,000");
        assert_eq!(format_price(1234.56, "EUR"), "€1,234.56");
        assert_eq!(format_price(1234.567, "GBP"), "£1,234.57");
        assert_eq!(format_price(9999.99, "CHF"), "CHF 9,999.99");
        assert_eq!(format_price(500.0, "JPY"), "¥500");
        // unknown currencies fall back to "CODE amount"
        assert_eq!(format_price(42.0, "XXX"), "XXX 42");
    }

    #[test]
    fn format_usd_cost_renders_fixed_digits() {
        assert_eq!(format_usd_cost(1234, 2), "$12.34");
        assert_eq!(format_usd_cost(5, 2), "$0.05");
        assert_eq!(format_usd_cost(0, 2), "$0.00");
        assert_eq!(format_usd_cost_f(123.456, 2), "$1.23");
        assert_eq!(format_usd_cost_f(1.5, 4), "$0.0150");
    }
}
