//! Argument tokenizer + resolver — port of `packages/subtrack/src/cli/parser.ts`
//! (self-contained replacement for gunshi's `args-tokens` + `resolver`).
//! Behavior mirrors the verified gunshi semantics: short groups, `--x=v`,
//! `-x=v`, `--` terminator, kebabized option names, array accumulation.

use std::collections::{HashMap, HashSet};

use crate::help::{ArgKind, ArgSpec};

// ── Tokens ────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Option {
        name: String,
        raw_name: String,
        value: Option<String>,
        inline_value: bool,
    },
    Positional {
        value: String,
    },
    Terminator,
}

fn is_short_option(arg: &str) -> bool {
    let b = arg.as_bytes();
    b.len() == 2 && b[0] == b'-' && b[1] != b'-'
}

fn is_short_option_group(arg: &str) -> bool {
    let b = arg.as_bytes();
    b.len() > 2 && b[0] == b'-' && b[1] != b'-'
}

/// `--x` / `--x=v` long-option detection. The JS checks `arg.includes("=", 3)`
/// (a `=` at full-string index >= 3), i.e. a `=` at or after the 2nd name char.
fn is_long_option(arg: &str) -> bool {
    if !arg.starts_with("--") {
        return false;
    }
    let s = &arg[2..];
    !s.is_empty() && !s[1..].contains('=')
}

fn is_long_option_and_value(arg: &str) -> bool {
    if !arg.starts_with("--") {
        return false;
    }
    let s = &arg[2..];
    !s.is_empty() && s[1..].contains('=')
}

/// Split argv into tokens (port of tokenize).
pub fn tokenize(argv: &[String]) -> Vec<Token> {
    let mut tokens: Vec<Token> = Vec::new();
    let mut remainings: Vec<String> = argv.to_vec();
    let mut terminated = false;

    while let Some(arg) = remainings.first().cloned() {
        remainings.remove(0);

        if terminated {
            tokens.push(Token::Positional { value: arg });
            continue;
        }
        if arg == "--" {
            tokens.push(Token::Terminator);
            terminated = true;
            continue;
        }
        if is_short_option(&arg) {
            tokens.push(Token::Option {
                name: arg[1..2].to_string(),
                raw_name: arg.clone(),
                value: None,
                inline_value: false,
            });
            continue;
        }
        if is_short_option_group(&arg) {
            let eq_idx = arg.find('=');
            if eq_idx == Some(2) {
                // `-x=v` : single short option with an inline value
                tokens.push(Token::Option {
                    name: arg[1..2].to_string(),
                    raw_name: format!("-{}", &arg[1..2]),
                    value: Some(arg[3..].to_string()),
                    inline_value: true,
                });
                continue;
            }
            let mut expanded: Vec<String> = Vec::new();
            let mut has_separator = false;
            let mut short_value = String::new();
            for ch in arg[1..].chars() {
                if has_separator {
                    short_value.push(ch);
                } else if ch == '=' {
                    has_separator = true;
                } else {
                    expanded.push(format!("-{}", ch));
                }
            }
            if !short_value.is_empty() {
                expanded.push(short_value);
            }
            // unshift in reverse so the first element is processed first
            while let Some(e) = expanded.pop() {
                remainings.insert(0, e);
            }
            continue;
        }
        if is_long_option(&arg) {
            tokens.push(Token::Option {
                name: arg[2..].to_string(),
                raw_name: arg.clone(),
                value: None,
                inline_value: false,
            });
            continue;
        }
        if is_long_option_and_value(&arg) {
            let eq = arg.find('=').unwrap();
            let name = arg[2..eq].to_string();
            tokens.push(Token::Option {
                name: name.clone(),
                raw_name: format!("--{}", name),
                value: Some(arg[eq + 1..].to_string()),
                inline_value: true,
            });
            continue;
        }
        tokens.push(Token::Positional { value: arg });
    }
    tokens
}

// ── Resolver ─────────────────────────────────────────────────────────────

/// kebabize: `camelCase` → `camel-case` (uppercase → `-lower`), mirror.
fn kebabize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_uppercase() {
            out.push('-');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn get_option_name(raw_arg: &str, to_kebab: bool) -> String {
    if to_kebab {
        kebabize(raw_arg)
    } else {
        raw_arg.to_string()
    }
}

/// Option display name like `'--currency' or '-c'`.
fn create_option_display_name(option: &str, schema: &ArgSpec) -> String {
    match schema.short {
        Some(s) => format!("'--{}' or '-{}'", option, s),
        None => format!("'--{}'", option),
    }
}

/// Argument validation error. `name` mirrors the JS `ArgValidationError.name`
/// (the argument name), so `name: message` matches gunshi's rendering.
#[derive(Clone, Debug)]
pub struct ArgError {
    pub name: String,
    pub message: String,
}

fn create_require_error(option: &str, schema: &ArgSpec) -> ArgError {
    let message = if schema.kind == ArgKind::Positional {
        format!("Positional argument '{}' is required", option)
    } else {
        format!(
            "Optional argument {} is required",
            create_option_display_name(option, schema)
        )
    };
    ArgError {
        name: option.to_string(),
        message,
    }
}

fn create_type_error(option: &str, schema: &ArgSpec) -> ArgError {
    let type_name = match schema.kind {
        ArgKind::Boolean => "boolean",
        ArgKind::String => "string",
        ArgKind::Positional => "positional",
    };
    ArgError {
        name: option.to_string(),
        message: format!(
            "Optional argument {} should be '{}'",
            create_option_display_name(option, schema),
            type_name
        ),
    }
}

fn create_unknown_option_error(option: &str) -> ArgError {
    ArgError {
        name: option.to_string(),
        message: format!("Unknown option '{}'", option),
    }
}

fn create_unexpected_positional_error(value: &str) -> ArgError {
    ArgError {
        name: "positional".to_string(),
        message: format!("Unexpected positional argument '{}'", value),
    }
}

/// Undecorated positionals are required (no `default` support).
fn should_require_missing_single_positional(schema: &ArgSpec) -> bool {
    schema.required == Some(true) || schema.required.is_none()
}

/// Count of positionals after each positional arg that still need values
/// (so optional positionals do not steal them).
fn create_required_positionals_after<'a>(args: &'a [ArgSpec<'a>]) -> HashMap<&'a str, usize> {
    let mut required_after: HashMap<&str, usize> = HashMap::new();
    let mut minimum_required = 0;
    for spec in args.iter().rev() {
        if spec.kind != ArgKind::Positional {
            continue;
        }
        required_after.insert(spec.name, minimum_required);
        minimum_required += if spec.array {
            if spec.required == Some(true) {
                1
            } else {
                0
            }
        } else if should_require_missing_single_positional(spec) {
            1
        } else {
            0
        };
    }
    required_after
}

#[derive(Clone, Debug, PartialEq)]
pub enum ArgValue {
    Bool(bool),
    Str(String),
    StrArray(Vec<String>),
}

#[derive(Clone, Debug, Default)]
#[allow(dead_code)] // positionals/rest consumed by later command phases
pub struct ParsedArgs {
    pub values: HashMap<String, ArgValue>,
    /// All validation-error messages, in order.
    pub errors: Vec<ArgError>,
    /// Positional tokens (including the command path).
    pub positionals: Vec<String>,
    /// Tokens after the `--` terminator.
    pub rest: Vec<String>,
}

impl ParsedArgs {
    pub fn bool(&self, key: &str) -> Option<bool> {
        match self.values.get(key) {
            Some(ArgValue::Bool(b)) => Some(*b),
            _ => None,
        }
    }

    pub fn str(&self, key: &str) -> Option<String> {
        match self.values.get(key) {
            Some(ArgValue::Str(s)) => Some(s.clone()),
            _ => None,
        }
    }

    #[allow(dead_code)] // array values consumed by later command phases
    pub fn str_array(&self, key: &str) -> Option<Vec<String>> {
        match self.values.get(key) {
            Some(ArgValue::StrArray(v)) => Some(v.clone()),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
struct RawOption {
    name: String,
    raw_name: String,
    value: Option<String>,
    #[allow(dead_code)] // mirrors the TS token shape; not read during resolve
    inline_value: bool,
}

/// Flush a pending (value-less) option token, attaching `value` if given.
/// Mirrors the JS `flushLong`/`flushShort` (value ?? pending.value).
fn flush_pending(
    option_tokens: &mut Vec<RawOption>,
    pending: &mut Option<RawOption>,
    value: Option<String>,
) {
    if let Some(mut o) = pending.take() {
        if o.value.is_none() {
            o.value = value;
        }
        option_tokens.push(o);
    }
}

/// Port of resolveArgs. The returned `positionals` includes the full command
/// path; callers pass `skip` so value assignment starts after the path.
pub fn resolve_args(
    args: &[String],
    specs: &[ArgSpec],
    skip: usize,
    to_kebab: bool,
    strict: bool,
) -> ParsedArgs {
    let tokens = tokenize(args);

    // ── analyze phase: separate positionals, long and short options ──
    let mut option_tokens: Vec<RawOption> = Vec::new();
    let mut positional_tokens: Vec<String> = Vec::new();
    let mut rest: Vec<String> = Vec::new();

    let mut boolean_long_option_names: HashSet<String> = HashSet::new();
    let mut known_long_option_names: HashSet<String> = HashSet::new();
    let mut negated_boolean_option_names: HashMap<String, String> = HashMap::new();
    let mut short_to_schema: HashMap<char, usize> = HashMap::new();

    for (idx, spec) in specs.iter().enumerate() {
        if spec.kind == ArgKind::Positional {
            continue;
        }
        let option_names = [
            spec.name.to_string(),
            get_option_name(spec.name, to_kebab),
            kebabize(spec.name),
        ];
        for option_name in &option_names {
            known_long_option_names.insert(option_name.clone());
            if spec.kind == ArgKind::Boolean {
                let negated_name = format!("no-{}", option_name);
                known_long_option_names.insert(negated_name.clone());
                boolean_long_option_names.insert(option_name.clone());
                boolean_long_option_names.insert(negated_name.clone());
                negated_boolean_option_names.insert(negated_name, spec.name.to_string());
            }
        }
        if let Some(s) = spec.short {
            short_to_schema.insert(s, idx);
        }
    }

    let mut pending_long: Option<RawOption> = None;
    let mut pending_short: Option<RawOption> = None;
    let mut terminated = false;

    for token in &tokens {
        match token {
            Token::Terminator => {
                terminated = true;
                flush_pending(&mut option_tokens, &mut pending_long, None);
                flush_pending(&mut option_tokens, &mut pending_short, None);
            }
            Token::Positional { value } => {
                if terminated && !value.is_empty() {
                    rest.push(value.clone());
                    continue;
                }
                if let Some(p) = &pending_short {
                    let ch = p.name.chars().next().unwrap();
                    let is_bool = short_to_schema
                        .get(&ch)
                        .map(|i| specs[*i].kind == ArgKind::Boolean)
                        .unwrap_or(false);
                    if is_bool {
                        positional_tokens.push(value.clone());
                        flush_pending(&mut option_tokens, &mut pending_short, None);
                    } else {
                        flush_pending(&mut option_tokens, &mut pending_short, Some(value.clone()));
                    }
                } else if let Some(p) = &pending_long {
                    if boolean_long_option_names.contains(&p.name) {
                        positional_tokens.push(value.clone());
                        flush_pending(&mut option_tokens, &mut pending_long, None);
                    } else {
                        flush_pending(&mut option_tokens, &mut pending_long, Some(value.clone()));
                    }
                } else {
                    positional_tokens.push(value.clone());
                }
            }
            Token::Option {
                name,
                raw_name,
                value,
                inline_value,
            } => {
                let is_long = raw_name.starts_with("--");
                let is_short_opt = is_short_option(raw_name);
                if is_long {
                    flush_pending(&mut option_tokens, &mut pending_long, None);
                    if *inline_value {
                        option_tokens.push(RawOption {
                            name: name.clone(),
                            raw_name: raw_name.clone(),
                            value: value.clone(),
                            inline_value: true,
                        });
                    } else {
                        pending_long = Some(RawOption {
                            name: name.clone(),
                            raw_name: raw_name.clone(),
                            value: None,
                            inline_value: false,
                        });
                    }
                    flush_pending(&mut option_tokens, &mut pending_short, None);
                } else if is_short_opt {
                    flush_pending(&mut option_tokens, &mut pending_short, None);
                    if *inline_value {
                        option_tokens.push(RawOption {
                            name: name.clone(),
                            raw_name: raw_name.clone(),
                            value: value.clone(),
                            inline_value: true,
                        });
                    } else {
                        pending_short = Some(RawOption {
                            name: name.clone(),
                            raw_name: raw_name.clone(),
                            value: None,
                            inline_value: false,
                        });
                    }
                    flush_pending(&mut option_tokens, &mut pending_long, None);
                } else {
                    flush_pending(&mut option_tokens, &mut pending_long, None);
                }
            }
        }
    }
    flush_pending(&mut option_tokens, &mut pending_long, None);
    flush_pending(&mut option_tokens, &mut pending_short, None);

    // ── strict: unknown options ──
    let mut unknown_option_names: Vec<String> = Vec::new();
    if strict {
        let mut seen: HashSet<String> = HashSet::new();
        for o in &option_tokens {
            let known = if o.raw_name.starts_with("--") {
                known_long_option_names.contains(&o.name)
            } else {
                let ch = o.name.chars().next().unwrap();
                short_to_schema.contains_key(&ch)
            };
            if !known && !seen.contains(&o.raw_name) {
                seen.insert(o.raw_name.clone());
                unknown_option_names.push(o.raw_name.clone());
            }
        }
    }

    // ── resolve phase ──
    let mut values: HashMap<String, ArgValue> = HashMap::new();
    let mut errors: Vec<ArgError> = Vec::new();
    for option in &unknown_option_names {
        errors.push(create_unknown_option_error(option));
    }
    let required_positionals_after = create_required_positionals_after(specs);

    let mut positionals_count = skip;
    for spec in specs {
        let arg = get_option_name(spec.name, to_kebab);
        if spec.kind == ArgKind::Positional {
            if spec.array {
                let available = positional_tokens.len().saturating_sub(positionals_count);
                if available > 0 {
                    let required_after = required_positionals_after.get(spec.name).copied().unwrap_or(0);
                    let to_consume = available.saturating_sub(required_after);
                    if to_consume > 0 {
                        let end = positionals_count + to_consume;
                        let slice: Vec<String> =
                            positional_tokens[positionals_count..end].to_vec();
                        values.insert(spec.name.to_string(), ArgValue::StrArray(slice));
                        positionals_count = end;
                    } else if spec.required == Some(true) {
                        errors.push(create_require_error(&arg, spec));
                    }
                } else if spec.required == Some(true) {
                    errors.push(create_require_error(&arg, spec));
                }
            } else if should_require_missing_single_positional(spec) {
                if let Some(p) = positional_tokens.get(positionals_count) {
                    values.insert(spec.name.to_string(), ArgValue::Str(p.clone()));
                    positionals_count += 1;
                } else {
                    errors.push(create_require_error(&arg, spec));
                }
            } else {
                let positional = positional_tokens.get(positionals_count).cloned();
                let required_after = required_positionals_after.get(spec.name).copied().unwrap_or(0);
                if let Some(p) = positional {
                    let still_available = positional_tokens.len().saturating_sub(positionals_count);
                    if still_available > required_after {
                        values.insert(spec.name.to_string(), ArgValue::Str(p));
                        positionals_count += 1;
                    }
                }
            }
            continue;
        }

        // option
        let matches: Vec<&RawOption> = option_tokens
            .iter()
            .filter(|t| {
                let short_match = spec
                    .short
                    .map(|s| t.name == s.to_string() && is_short_option(&t.raw_name))
                    .unwrap_or(false);
                let long_match = t.raw_name.starts_with("--")
                    && (t.name == arg
                        || t.name == spec.name
                        || t.name == kebabize(spec.name)
                        || (spec.kind == ArgKind::Boolean
                            && negated_boolean_option_names.get(&t.name)
                                == Some(&spec.name.to_string())));
                short_match || long_match
            })
            .collect();

        if spec.required == Some(true) && matches.is_empty() {
            errors.push(create_require_error(&arg, spec));
            continue;
        }
        for t in matches {
            if spec.required == Some(true) && spec.kind != ArgKind::Boolean && t.value.is_none() {
                errors.push(create_require_error(&arg, spec));
                continue;
            }
            if spec.kind == ArgKind::Boolean {
                let value = negated_boolean_option_names.get(&t.name)
                    != Some(&spec.name.to_string());
                values.insert(spec.name.to_string(), ArgValue::Bool(value));
            } else if let Some(v) = &t.value {
                if !v.is_empty() {
                    values.insert(spec.name.to_string(), ArgValue::Str(v.clone()));
                }
            } else {
                errors.push(create_type_error(&arg, spec));
            }
        }
    }

    if strict {
        for p in positional_tokens.iter().skip(positionals_count) {
            errors.push(create_unexpected_positional_error(p));
        }
    }

    ParsedArgs {
        values,
        errors,
        positionals: positional_tokens,
        rest,
    }
}