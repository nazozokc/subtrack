use super::*;

use crate::parser::{ArgKind, ArgSpec};

fn argv(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| a.to_string()).collect()
}

fn spec(name: &'static str, kind: ArgKind, short: Option<char>) -> ArgSpec<'static> {
    ArgSpec {
        name,
        kind,
        short,
        description: "",
        array: false,
        required: None,
    }
}

fn required_spec(name: &'static str, kind: ArgKind) -> ArgSpec<'static> {
    ArgSpec {
        name,
        kind,
        short: None,
        description: "",
        array: false,
        required: Some(true),
    }
}

#[test]
fn tokenize_classifies_short_long_and_inline() {
    let t = tokenize(&argv(&["--json"]));
    assert!(matches!(
        &t[0],
        Token::Option {
            name,
            raw_name,
            value,
            inline_value
        }
        if name == "json" && raw_name == "--json" && value.is_none() && !*inline_value
    ));

    let t = tokenize(&argv(&["--name=John"]));
    assert!(matches!(
        &t[0],
        Token::Option {
            name,
            value,
            inline_value,
            ..
        }
        if name == "name" && value.as_deref() == Some("John") && *inline_value
    ));

    let t = tokenize(&argv(&["-x"]));
    assert!(matches!(&t[0], Token::Option { name, .. } if name == "x"));
}

#[test]
fn tokenize_expands_short_groups_in_order() {
    let t = tokenize(&argv(&["-xyz"]));
    assert_eq!(t.len(), 3);
    let names: Vec<&str> = t
        .iter()
        .filter_map(|tk| match tk {
            Token::Option { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(names, ["x", "y", "z"]);
    assert!(t.iter().all(|tk| matches!(
        tk,
        Token::Option {
            inline_value: false,
            value: None,
            ..
        }
    )));
}

#[test]
fn tokenize_keeps_inline_short_value() {
    let t = tokenize(&argv(&["-x=v"]));
    assert!(matches!(
        &t[0],
        Token::Option {
            name,
            value,
            inline_value,
            ..
        }
        if name == "x" && value.as_deref() == Some("v") && *inline_value
    ));
}

#[test]
fn tokenize_marks_terminator_and_positionals() {
    let t = tokenize(&argv(&["hello", "--", "-x", "rest"]));
    assert!(matches!(&t[0], Token::Positional { value } if value == "hello"));
    assert!(matches!(t[1], Token::Terminator));
    assert!(matches!(&t[2], Token::Positional { value } if value == "-x"));
    assert!(matches!(&t[3], Token::Positional { value } if value == "rest"));
}

#[test]
fn resolve_maps_kebab_long_and_short_values() {
    let specs = [
        spec("minPrice", ArgKind::String, Some('m')),
        spec("json", ArgKind::Boolean, None),
    ];
    // kebabized inline long
    let r = resolve_args(&argv(&["--min-price=5"]), &specs, 0, true, true);
    assert!(r.errors.is_empty());
    assert_eq!(r.values.get("minPrice"), Some(&ArgValue::Str("5".into())));

    // separate value after a short option
    let r = resolve_args(&argv(&["-m", "12", "--json"]), &specs, 0, true, true);
    assert!(r.errors.is_empty());
    assert_eq!(r.values.get("minPrice"), Some(&ArgValue::Str("12".into())));

    // boolean without value → true
    assert_eq!(r.values.get("json"), Some(&ArgValue::Bool(true)));
}

#[test]
fn resolve_negates_with_no_long_options() {
    let specs = [spec("json", ArgKind::Boolean, None)];
    let r = resolve_args(&argv(&["--no-json"]), &specs, 0, true, true);
    assert!(r.errors.is_empty());
    assert_eq!(r.values.get("json"), Some(&ArgValue::Bool(false)));

    let r = resolve_args(&argv(&["--json"]), &specs, 0, true, true);
    assert_eq!(r.values.get("json"), Some(&ArgValue::Bool(true)));
}

#[test]
fn resolve_drops_unknown_options_and_reports_in_strict_mode() {
    let specs = [spec("json", ArgKind::Boolean, None)];

    // strict: unknown option surfaces as an error and nothing else breaks
    // (the error name keeps the option name only, never the inline value)
    let r = resolve_args(&argv(&["--min-price=5"]), &specs, 0, true, true);
    assert_eq!(r.errors.len(), 1);
    assert_eq!(r.errors[0].name, "--min-price");
    assert_eq!(r.errors[0].message, "Unknown option '--min-price'");
    assert_eq!(r.values.get("json"), None);

    // non-strict: silently ignored
    let r = resolve_args(&argv(&["--min-price=5"]), &specs, 0, true, false);
    assert!(r.errors.is_empty());
}

#[test]
fn resolve_requires_string_option_values() {
    let specs = [required_spec("name", ArgKind::String)];
    let r = resolve_args(&argv(&[]), &specs, 0, true, true);
    assert_eq!(
        r.errors[0].message,
        "Optional argument '--name' is required"
    );

    // value-less required string also fails
    let r = resolve_args(&argv(&["--name"]), &specs, 0, true, true);
    assert_eq!(
        r.errors[0].message,
        "Optional argument '--name' is required"
    );

    // inline value resolves
    let r = resolve_args(&argv(&["--name=Alice"]), &specs, 0, true, true);
    assert!(r.errors.is_empty());
    assert_eq!(r.values.get("name"), Some(&ArgValue::Str("Alice".into())));
}

#[test]
fn resolve_collects_positionals_and_rejects_unexpected() {
    let specs = [required_spec("id", ArgKind::Positional)];
    let r = resolve_args(&argv(&["42"]), &specs, 0, true, true);
    assert!(r.errors.is_empty());
    assert_eq!(r.values.get("id"), Some(&ArgValue::Str("42".into())));

    let r = resolve_args(&argv(&[]), &specs, 0, true, true);
    assert_eq!(r.errors[0].message, "Positional argument 'id' is required");

    let r = resolve_args(&argv(&["42", "extra"]), &specs, 0, true, true);
    assert_eq!(
        r.errors[0].message,
        "Unexpected positional argument 'extra'"
    );
}

#[test]
fn resolve_collects_rest_after_terminator() {
    let specs = [spec("json", ArgKind::Boolean, None)];
    let r = resolve_args(&argv(&["--json", "--", "--x"]), &specs, 0, true, true);
    assert!(r.errors.is_empty());
    assert_eq!(r.rest, vec!["--x".to_string()]);
    assert_eq!(r.values.get("json"), Some(&ArgValue::Bool(true)));
}
