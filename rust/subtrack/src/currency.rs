//! `currency` command — port of `currency.ts` (handleCurrencyList).

use crate::json::quote_string;
use crate::logger::{self, log};
use crate::parser::ParsedArgs;
use crate::table::section_title;

/// Port of `CURRENCY_CHOICES` (validation.ts).
pub const CURRENCY_CHOICES: &[(&str, &str)] = &[
    ("AED", "AED (UAE Dirham)"),
    ("ARS", "ARS (Argentine Peso)"),
    ("AUD", "AUD (Australian Dollar)"),
    ("BRL", "BRL (Brazilian Real)"),
    ("CAD", "CAD (Canadian Dollar)"),
    ("CHF", "CHF (Swiss Franc)"),
    ("CLP", "CLP (Chilean Peso)"),
    ("CNY", "CNY (Chinese Yuan)"),
    ("COP", "COP (Colombian Peso)"),
    ("CZK", "CZK (Czech Koruna)"),
    ("DKK", "DKK (Danish Krone)"),
    ("EGP", "EGP (Egyptian Pound)"),
    ("EUR", "EUR (Euro)"),
    ("GBP", "GBP (British Pound)"),
    ("HKD", "HKD (Hong Kong Dollar)"),
    ("HUF", "HUF (Hungarian Forint)"),
    ("IDR", "IDR (Indonesian Rupiah)"),
    ("ILS", "ILS (Israeli Shekel)"),
    ("INR", "INR (Indian Rupee)"),
    ("JPY", "JPY (日本円)"),
    ("KRW", "KRW (South Korean Won)"),
    ("MXN", "MXN (Mexican Peso)"),
    ("MYR", "MYR (Malaysian Ringgit)"),
    ("NGN", "NGN (Nigerian Naira)"),
    ("NOK", "NOK (Norwegian Krone)"),
    ("NZD", "NZD (New Zealand Dollar)"),
    ("PHP", "PHP (Philippine Peso)"),
    ("PLN", "PLN (Polish Zloty)"),
    ("SAR", "SAR (Saudi Riyal)"),
    ("SEK", "SEK (Swedish Krona)"),
    ("SGD", "SGD (Singapore Dollar)"),
    ("THB", "THB (Thai Baht)"),
    ("TRY", "TRY (Turkish Lira)"),
    ("TWD", "TWD (Taiwan Dollar)"),
    ("USD", "USD (US Dollar)"),
    ("VND", "VND (Vietnamese Dong)"),
    ("ZAR", "ZAR (South African Rand)"),
];

pub fn run_currency(parsed: &ParsedArgs) -> i32 {
    let json = parsed.bool("json") == Some(true);

    if json {
        let mut out = String::from("[\n");
        for (i, (value, name)) in CURRENCY_CHOICES.iter().enumerate() {
            out.push_str("  {\n");
            out.push_str(&format!("    \"name\": {},\n", quote_string(name)));
            out.push_str(&format!("    \"value\": {}\n", quote_string(value)));
            out.push_str(if i + 1 == CURRENCY_CHOICES.len() {
                "  }\n"
            } else {
                "  },\n"
            });
        }
        out.push_str("]\n");
        logger::out_str(&out);
        return 0;
    }

    let theme = crate::config::display_theme();
    log(&section_title("Supported Currencies", &theme));
    for (value, name) in CURRENCY_CHOICES {
        log(&format!("  {:<5} {}", value, name));
    }
    log(&format!(
        "\n{} currencies supported",
        CURRENCY_CHOICES.len()
    ));
    0
}
