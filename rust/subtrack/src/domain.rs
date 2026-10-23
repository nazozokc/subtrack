//! Domain value objects — the typed core of subtrack.
//!
//! Everything the user can type (`--status`, `--currency`, `--cycle`,
//! `--sort`, numeric flags) is *parsed once* into these values at the flag
//! boundary. After that point the command layer works on `match`-exhaustive
//! enums instead of magic strings, and the compiler catches any missing
//! variant.
//!
//! Two invariants keep this refactor behavior-preserving:
//! 1. `as_str`-style accessors produce the exact legacy strings (SQL column
//!    names included), so DB reads and rendered output stay byte-identical.
//! 2. [`parse_number`] keeps JavaScript `parseFloat` acceptance semantics
//!    (leading-whitespace skip, longest-valid-prefix, case-sensitive
//!    `Infinity` literal), so flag validation fires on exactly the same
//!    inputs as the TypeScript original did.

use std::fmt;
use std::str::FromStr;

use crate::parser::ParsedArgs;

// ── Status ──────────────────────────────────────────────────────────────────

/// Subscription lifecycle status.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Active,
    Paused,
    Cancelled,
    Archived,
}

impl Status {
    pub const ALL: [Status; 4] = [
        Status::Active,
        Status::Paused,
        Status::Cancelled,
        Status::Archived,
    ];

    /// The DB/display spelling (equals the legacy literal).
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Active => "active",
            Status::Paused => "paused",
            Status::Cancelled => "cancelled",
            Status::Archived => "archived",
        }
    }

    /// Parse a status word. Same acceptance as the legacy validator.
    pub fn parse(s: &str) -> Result<Self, Invalid> {
        s.parse()
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Status {
    type Err = Invalid;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .copied()
            .find(|v| v.as_str() == s)
            .ok_or_else(|| Invalid(s.to_owned()))
    }
}

// ── Cycle ───────────────────────────────────────────────────────────────────

/// Billing cycle: a named preset or a custom `Nd` day-count (1..=365).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cycle {
    Weekly,
    BiWeekly,
    Monthly,
    Quarterly,
    SemiAnnual,
    Yearly,
    /// Custom cycle spelled `"{days}d"` in the DB, e.g. `"30d"`.
    Days(u16),
}

impl Cycle {
    pub const ALL: [Cycle; 6] = [
        Cycle::Weekly,
        Cycle::BiWeekly,
        Cycle::Monthly,
        Cycle::Quarterly,
        Cycle::SemiAnnual,
        Cycle::Yearly,
    ];

    /// DB/display spelling for named presets; `None` for [`Cycle::Days`].
    pub fn named_str(self) -> Option<&'static str> {
        match self {
            Cycle::Weekly => Some("weekly"),
            Cycle::BiWeekly => Some("bi-weekly"),
            Cycle::Monthly => Some("monthly"),
            Cycle::Quarterly => Some("quarterly"),
            Cycle::SemiAnnual => Some("semi-annual"),
            Cycle::Yearly => Some("yearly"),
            Cycle::Days(_) => None,
        }
    }

    /// Parse a named preset or an `Nd` day cycle (1..=365).
    pub fn parse(s: &str) -> Result<Self, Invalid> {
        if let Some(name) = Self::parse_named(s) {
            return Ok(name);
        }
        if let Some(days) = js_day_suffix(s) {
            return Ok(Cycle::Days(days));
        }
        Err(Invalid(s.to_owned()))
    }

    /// Parse a *named* preset only — the set shared by the `--period`
    /// flags of `payment`/`compare`/`usage total` (where `"30d"` is NOT a
    /// valid period).
    pub fn parse_named(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|c| c.named_str() == Some(s))
    }
}

impl fmt::Display for Cycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(name) = self.named_str() {
            f.write_str(name)
        } else if let Cycle::Days(d) = self {
            write!(f, "{d}d")
        } else {
            unreachable!("Days is the only unnamed variant")
        }
    }
}

impl FromStr for Cycle {
    type Err = Invalid;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Cycle::parse(s)
    }
}

/// `"3d"` → 3 (mirrors `cycle_days`: integer 1..=365; `"1.5d"` rejected).
fn js_day_suffix(s: &str) -> Option<u16> {
    let rest = s.strip_suffix('d')?;
    let n = rest.parse::<i64>().ok()?;
    u16::try_from(n).ok().filter(|d| (1..=365).contains(d))
}

// ── Currency ────────────────────────────────────────────────────────────────

/// One of the 37 currencies accepted by `add`/`edit`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Currency {
    Aed,
    Ars,
    Aud,
    Brl,
    Cad,
    Chf,
    Clp,
    Cny,
    Cop,
    Czk,
    Dkk,
    Egp,
    Eur,
    Gbp,
    Hkd,
    Huf,
    Idr,
    Ils,
    Inr,
    Jpy,
    Krw,
    Mxn,
    Myr,
    Ngn,
    Nok,
    Nzd,
    Php,
    Pln,
    Sar,
    Sek,
    Sgd,
    Thb,
    Try,
    Twd,
    Usd,
    Vnd,
    Zar,
}

impl Currency {
    pub const ALL: [Currency; 37] = [
        Currency::Aed,
        Currency::Ars,
        Currency::Aud,
        Currency::Brl,
        Currency::Cad,
        Currency::Chf,
        Currency::Clp,
        Currency::Cny,
        Currency::Cop,
        Currency::Czk,
        Currency::Dkk,
        Currency::Egp,
        Currency::Eur,
        Currency::Gbp,
        Currency::Hkd,
        Currency::Huf,
        Currency::Idr,
        Currency::Ils,
        Currency::Inr,
        Currency::Jpy,
        Currency::Krw,
        Currency::Mxn,
        Currency::Myr,
        Currency::Ngn,
        Currency::Nok,
        Currency::Nzd,
        Currency::Php,
        Currency::Pln,
        Currency::Sar,
        Currency::Sek,
        Currency::Sgd,
        Currency::Thb,
        Currency::Try,
        Currency::Twd,
        Currency::Usd,
        Currency::Vnd,
        Currency::Zar,
    ];

    /// The DB/display spelling (equals the legacy literal).
    pub fn as_str(self) -> &'static str {
        match self {
            Currency::Aed => "AED",
            Currency::Ars => "ARS",
            Currency::Aud => "AUD",
            Currency::Brl => "BRL",
            Currency::Cad => "CAD",
            Currency::Chf => "CHF",
            Currency::Clp => "CLP",
            Currency::Cny => "CNY",
            Currency::Cop => "COP",
            Currency::Czk => "CZK",
            Currency::Dkk => "DKK",
            Currency::Egp => "EGP",
            Currency::Eur => "EUR",
            Currency::Gbp => "GBP",
            Currency::Hkd => "HKD",
            Currency::Huf => "HUF",
            Currency::Idr => "IDR",
            Currency::Ils => "ILS",
            Currency::Inr => "INR",
            Currency::Jpy => "JPY",
            Currency::Krw => "KRW",
            Currency::Mxn => "MXN",
            Currency::Myr => "MYR",
            Currency::Ngn => "NGN",
            Currency::Nok => "NOK",
            Currency::Nzd => "NZD",
            Currency::Php => "PHP",
            Currency::Pln => "PLN",
            Currency::Sar => "SAR",
            Currency::Sek => "SEK",
            Currency::Sgd => "SGD",
            Currency::Thb => "THB",
            Currency::Try => "TRY",
            Currency::Twd => "TWD",
            Currency::Usd => "USD",
            Currency::Vnd => "VND",
            Currency::Zar => "ZAR",
        }
    }

    /// Symbol shown by `Intl.NumberFormat` for en-US; ambiguous/unknown
    /// currencies fall back to the `{CODE} {amount}` rendering.
    pub fn symbol(self) -> Option<&'static str> {
        match self {
            Currency::Usd => Some("$"),
            Currency::Jpy => Some("¥"),
            Currency::Eur => Some("€"),
            Currency::Gbp => Some("£"),
            Currency::Krw => Some("₩"),
            Currency::Inr => Some("₹"),
            Currency::Brl => Some("R$"),
            Currency::Cny => Some("CN¥"),
            Currency::Aud => Some("A$"),
            Currency::Cad => Some("CA$"),
            Currency::Hkd => Some("HK$"),
            Currency::Sgd => Some("S$"),
            Currency::Twd => Some("NT$"),
            Currency::Mxn => Some("MX$"),
            Currency::Try => Some("₺"),
            _ => None,
        }
    }

    /// Parse an uppercase ISO-like code. Same acceptance as the legacy
    /// validator (3-char uppercase within the choice list).
    pub fn parse(s: &str) -> Result<Self, Invalid> {
        s.parse()
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Currency {
    type Err = Invalid;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .copied()
            .find(|v| v.as_str() == s)
            .ok_or_else(|| Invalid(s.to_owned()))
    }
}

// ── Sort field ──────────────────────────────────────────────────────────────

/// `list --sort` field. The accepted set mirrors the TS `SORT_FIELDS`
/// exactly — these happen to be real `subscriptions` columns, so `as_str`
/// doubles as the `ORDER BY` identifier. Unknown values fall back to `id`
/// at the query layer (TS behavior; a broadened list would emit a SQLite
/// `no such column` error).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortField {
    Id,
    Name,
    Price,
    Currency,
    Cycle,
    Status,
}

impl SortField {
    pub const ALL: [SortField; 6] = [
        SortField::Id,
        SortField::Name,
        SortField::Price,
        SortField::Currency,
        SortField::Cycle,
        SortField::Status,
    ];

    /// SQL column spelling. Accepted `--sort` values happen to be real
    /// `subscriptions` columns, so this is also the `ORDER BY` identifier.
    pub fn as_str(self) -> &'static str {
        match self {
            SortField::Id => "id",
            SortField::Name => "name",
            SortField::Price => "price",
            SortField::Currency => "currency",
            SortField::Cycle => "cycle",
            SortField::Status => "status",
        }
    }

    /// Parse a `--sort` value. Unknown values fail; the caller decides
    /// whether they fall back to the default sort or exit 1 (the TS port
    /// varies per command).
    pub fn parse(s: &str) -> Result<Self, Invalid> {
        s.parse()
    }
}

impl fmt::Display for SortField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for SortField {
    type Err = Invalid;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .copied()
            .find(|v| v.as_str() == s)
            .ok_or_else(|| Invalid(s.to_owned()))
    }
}

// ── Parse errors ────────────────────────────────────────────────────────────

/// Legacy `listActive` alive-filter: drop rows whose status says `cancelled`.
/// Unknown status values never match and are kept, exactly like the original
/// string comparison.
pub fn not_cancelled(status: &str) -> bool {
    !matches!(Status::parse(status), Ok(Status::Cancelled))
}

/// Archived rows are hidden from most listings.
pub fn is_archived(status: &str) -> bool {
    matches!(Status::parse(status), Ok(Status::Archived))
}

/// Only rows explicitly marked `active`.
pub fn is_active(status: &str) -> bool {
    matches!(Status::parse(status), Ok(Status::Active))
}

/// Rows that keep billing money: neither `cancelled` nor `paused`. Unknown
/// status values count as billable, matching the original string checks.
pub fn not_cancelled_or_paused(status: &str) -> bool {
    !matches!(
        Status::parse(status),
        Ok(Status::Cancelled) | Ok(Status::Paused)
    )
}

/// Rows visible in listings: anything but `cancelled` or `archived`. Unknown
/// status values stay visible, matching the original string checks.
pub fn listed(status: &str) -> bool {
    !matches!(
        Status::parse(status),
        Ok(Status::Cancelled) | Ok(Status::Archived)
    )
}

/// Rejected user input; carries the offending string for message building.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Invalid(pub String);

impl fmt::Display for Invalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid value: {:?}", self.0)
    }
}

// ── Numeric flag parsing ────────────────────────────────────────────────────

/// JavaScript-`parseFloat`-compatible string → number.
///
/// Returns `None` where `parseFloat` yields `NaN` (no valid prefix).
/// Acceptance details kept identical:
/// - leading `White_Space` chars *and* U+FEFF are skipped;
/// - a single ASCII `+`/`-` sign follows, then either the case-sensitive
///   literal `Infinity` or the longest decimal prefix (`3abc` → 3,
///   `1_000` → 1, `0x10` → 0, `1e5e5` → 100000).
pub fn parse_number(s: &str) -> Option<f64> {
    let s = s.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{FEFF}');
    let (neg, s) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => match s.strip_prefix('+') {
            Some(r) => (false, r),
            None => (false, s),
        },
    };
    if s.starts_with("Infinity") {
        return Some(if neg {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        });
    }
    parse_mantissa(s).map(|v| if neg { -v } else { v })
}

/// Scan the longest ECMA-262 `StrDecimalLiteral` mantissa. The scanned
/// prefix must contain at least one digit (`""`, `"+"`, `"."`, `".e5"` → no
/// number), and a trailing exponent without digits is ignored (`"1e"` → 1).
fn parse_mantissa(s: &str) -> Option<f64> {
    let bytes = s.as_bytes();
    let n = bytes.len();
    let int_end = digit_run(bytes, 0);
    let mut end = int_end;
    let mut digits = int_end;
    if end < n && bytes[end] == b'.' {
        // Always advance past the dot so `"1.e3"` keeps its exponent.
        let frac_start = end + 1;
        let frac_len = digit_run(bytes, frac_start);
        digits += frac_len;
        end = frac_start + frac_len;
    }
    if digits == 0 {
        return None;
    }
    if end < n && (bytes[end] | 0x20) == b'e' {
        let mut j = end + 1;
        if j < n && (bytes[j] == b'+' || bytes[j] == b'-') {
            j += 1;
        }
        let k = j + digit_run(bytes, j);
        if k > j {
            end = k;
        }
    }
    let prefix = &s[..end];
    // Scanned prefixes are always valid Rust float syntax (`"1."` and
    // `"5.e3"` included, asserted by the tests below). The dot-stripping
    // fallback exists in case the standard library tightens its grammar.
    match prefix.parse::<f64>() {
        Ok(v) => Some(v),
        Err(_) => prefix.replace('.', "").parse::<f64>().ok(),
    }
}

/// Count of consecutive ASCII digits starting at `from` (saturating).
fn digit_run(bytes: &[u8], from: usize) -> usize {
    bytes
        .get(from..)
        .unwrap_or(&[])
        .iter()
        .copied()
        .take_while(|b| b.is_ascii_digit())
        .count()
}

/// Result of reading a numeric flag value: [`NumArg::Missing`] = flag
/// absent (OK), [`NumArg::Value`] = parsed (never NaN), [`NumArg::Bad`] =
/// present but unparsable (caller prints its message).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NumArg {
    Missing,
    Value(f64),
    Bad,
}

/// Read `--key` as a numeric flag with JS acceptance semantics.
pub fn num_arg(args: &ParsedArgs, key: &str) -> NumArg {
    match args.str(key) {
        None => NumArg::Missing,
        Some(s) => parse_number(&s).map_or(NumArg::Bad, NumArg::Value),
    }
}

/// Require `--key` to be a whole number between satisfying `pred` (the
/// whole-number check is built in). Prints `msg` and returns `None` on any
/// violation; absent flag → `None` without printing.
///
/// Require `--key` to be a whole number satisfying `pred` (the whole-number
/// check is built in). Prints `msg` and returns `Err(())` on any violation;
/// an absent flag is fine and yields `Ok(None)`.
///
/// ```ignore
/// let limit = match int_arg(&parsed, "limit", |v| v >= 1.0,
///     "limit must be a positive integer") { Err(()) => return 1, Ok(l) => l };
/// ```
pub fn int_arg(
    args: &ParsedArgs,
    key: &str,
    pred: impl Fn(f64) -> bool,
    msg: &str,
) -> Result<Option<i64>, ()> {
    f64_arg(args, key, |v| pred(v) && v.fract() == 0.0, msg).map(|v| v.map(|n| n as i64))
}

/// Require `--key` to be a number satisfying `pred`; prints `msg` and
/// returns `Err(())` on any violation.
pub fn f64_arg(
    args: &ParsedArgs,
    key: &str,
    pred: impl Fn(f64) -> bool,
    msg: &str,
) -> Result<Option<f64>, ()> {
    match num_arg(args, key) {
        NumArg::Value(v) if pred(v) => Ok(Some(v)),
        NumArg::Missing => Ok(None),
        NumArg::Bad | NumArg::Value(_) => {
            crate::logger::fail(msg);
            Err(())
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_matches_legacy_literals() {
        assert_eq!(Status::parse("active"), Ok(Status::Active));
        assert_eq!(Status::parse("paused"), Ok(Status::Paused));
        assert_eq!(Status::parse("cancelled"), Ok(Status::Cancelled));
        assert_eq!(Status::parse("archived"), Ok(Status::Archived));
        assert!(Status::parse("Active").is_err());
        // Display round-trips to the legacy string.
        assert_eq!(Status::Active.to_string(), "active");
        assert_eq!(
            Status::ALL.map(|s| s.as_str()),
            ["active", "paused", "cancelled", "archived"]
        );
    }

    #[test]
    fn cycle_matches_legacy_literals() {
        assert_eq!(Cycle::parse("monthly"), Ok(Cycle::Monthly));
        assert_eq!(Cycle::parse("30d"), Ok(Cycle::Days(30)));
        assert_eq!(Cycle::Days(1).to_string(), "1d");
        assert!(Cycle::parse("0d").is_err());
        assert!(Cycle::parse("366d").is_err());
        assert!(Cycle::parse("1.5d").is_err());
        assert_eq!(Cycle::parse("365d"), Ok(Cycle::Days(365)));
        assert_eq!(Cycle::BiWeekly.named_str(), Some("bi-weekly"));
    }

    #[test]
    fn currency_roundtrip_and_symbols() {
        assert!(
            Currency::ALL.len() == 37 && {
                Currency::ALL
                    .iter()
                    .all(|c| Currency::parse(c.as_str()) == Ok(*c))
            }
        );
        assert!(Currency::parse("usd").is_err());
        assert_eq!(Currency::Usd.symbol(), Some("$"));
        assert_eq!(Currency::Aed.symbol(), None);
    }

    #[test]
    fn parse_number_matches_parse_float() {
        // (input, output); None ⇔ JS NaN
        let cases: &[(&str, Option<f64>)] = &[
            ("", None),
            ("   ", None),
            ("+", None),
            ("-", None),
            (".", None),
            (".e5", None),
            ("e5", None),
            ("- .5", None),
            ("3", Some(3.0)),
            ("3.0", Some(3.0)),
            ("3abc", Some(3.0)),
            (" 3", Some(3.0)),
            ("\u{FEFF}-3", Some(-3.0)),
            ("-3", Some(-3.0)),
            ("+3", Some(3.0)),
            ("Infinity", Some(f64::INFINITY)),
            ("-Infinity", Some(f64::NEG_INFINITY)),
            ("+Infinity", Some(f64::INFINITY)),
            ("infinity", None), // case-sensitive in JS
            ("Infinityx", Some(f64::INFINITY)),
            ("NaN", None),
            ("0x10", Some(0.0)),
            ("1_000", Some(1.0)),
            ("1e3", Some(1e3)),
            ("1E3", Some(1e3)),
            ("1e+3", Some(1e3)),
            ("1e-3", Some(1e-3)),
            ("1ex", Some(1.0)),
            ("1e", Some(1.0)),
            (".5", Some(0.5)),
            ("+.5", Some(0.5)),
            ("5.", Some(5.0)),
            ("5.e3", Some(5000.0)), // 5e3 == 5000 in JS too
            ("1..3", Some(1.0)),
            ("1.2.3", Some(1.2)),
            ("1e5e5", Some(100000.0)),
            ("1e999999", Some(f64::INFINITY)),
        ];
        for &(input, expected) in cases {
            assert_eq!(parse_number(input), expected, "input {input:?}");
        }
    }

    #[test]
    fn num_arg_reports_missing_value_bad() {
        use crate::parser::ArgValue;
        let mut args = ParsedArgs::default();
        args.values
            .insert("a".into(), ArgValue::Str("x".to_owned()));
        args.values
            .insert("b".into(), ArgValue::Str("1.5".to_owned()));
        assert_eq!(num_arg(&args, "a"), NumArg::Bad);
        assert_eq!(num_arg(&args, "b"), NumArg::Value(1.5));
        assert_eq!(num_arg(&args, "c"), NumArg::Missing);
    }

    #[test]
    fn int_arg_treats_absent_flag_as_ok_none() {
        use crate::parser::ArgValue;
        let mut args = ParsedArgs::default();
        args.values
            .insert("limit".into(), ArgValue::Str("5".to_owned()));
        args.values
            .insert("frac".into(), ArgValue::Str("1.5".to_owned()));
        assert_eq!(int_arg(&args, "limit", |v| v >= 1.0, "msg"), Ok(Some(5)));
        assert_eq!(int_arg(&args, "absent", |v| v >= 1.0, "msg"), Ok(None));
        // Fractional values and out-of-range values both fail.
        assert_eq!(int_arg(&args, "frac", |v| v >= 1.0, "msg"), Err(()));
        assert_eq!(int_arg(&args, "limit", |v| v > 100.0, "msg"), Err(()));
    }
}
