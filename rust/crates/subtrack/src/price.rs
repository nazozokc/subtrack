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
fn group_whole(n: i64) -> String {
    let neg = n < 0;
    let abs = n.unsigned_abs();
    let digits = abs.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',')
        }
        out.push(c)
    }
    if neg {
        format!("-{}", out)
    } else {
        out
    }
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