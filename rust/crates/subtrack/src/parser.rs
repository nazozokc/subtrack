//! Argument parser — port of `packages/subtrack/src/cli/parser.ts`
//! (resolveArgs) for the flag surface used by the list command.

use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub enum ArgValue {
    Bool(bool),
    Str(String),
}

#[derive(Clone, Debug)]
pub struct ArgError {
    /// Full option token, e.g. "--bogus-flag".
    pub name: String,
    pub message: String,
}

pub struct ParsedArgs {
    pub values: HashMap<String, ArgValue>,
    /// All validation-error messages, in order (usually one).
    pub errors: Vec<ArgError>,
}

pub struct ArgSpec {
    pub booleans: &'static [&'static str],
    pub strings: &'static [&'static str],
    pub shorts: &'static [(&'static str, &'static str)],
}

pub const LIST_SPEC: ArgSpec = ArgSpec {
    booleans: &[
        "help", "version", "desc", "api", "notes", "method", "contract", "vendor", "json",
        "include-archived",
    ],
    strings: &[
        "currency", "sort", "status", "min-price", "max-price", "tags", "limit", "offset",
    ],
    shorts: &[
        ("h", "help"),
        ("v", "version"),
        ("d", "desc"),
        ("a", "api"),
        ("n", "notes"),
        ("m", "method"),
        ("j", "json"),
        ("c", "currency"),
    ],
};

/// kebabize: lowercase-cases uppercase letters only (mirror).
fn kebabize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_uppercase() { c.to_ascii_lowercase() } else { c })
        .collect()
}

fn short_key(spec: &ArgSpec, short: &str) -> Option<&'static str> {
    spec.shorts.iter().find(|(s, _)| *s == short).map(|(_, k)| *k)
}

/// Port of resolveArgs(args, spec, { skip, strict: true, toKebab: false }).
pub fn resolve_args(args: &[String], spec: &ArgSpec, skip: usize) -> ParsedArgs {
    let mut values: HashMap<String, ArgValue> = HashMap::new();
    let mut errors: Vec<ArgError> = Vec::new();

    let unknown = |raw: &str| ArgError {
        name: raw.to_string(),
        message: format!("Unknown option '{}'", raw),
    };

    let mut i = skip;
    while i < args.len() {
        let tok = &args[i];

        if tok == "--" {
            i += 1;
            continue;
        }

        if let Some(rest) = tok.strip_prefix("--") {
            let (name, inline) = match rest.find('=') {
                Some(eq) => (&rest[..eq], Some(rest[eq + 1..].to_string())),
                None => (rest, None),
            };
            let key = kebabize(name);
            if spec.booleans.contains(&key.as_str()) {
                if let Some(inline) = inline {
                    values.insert(key.clone(), ArgValue::Bool(inline == "true"));
                } else {
                    values.insert(key.clone(), ArgValue::Bool(true));
                }
                i += 1
            } else if let Some(neg) = key.strip_prefix("no-") {
                if spec.booleans.contains(&neg) {
                    values.insert(neg.to_string(), ArgValue::Bool(false));
                    i += 1
                } else {
                    errors.push(unknown(tok));
                    i += 1
                }
            } else if spec.strings.contains(&key.as_str()) {
                let value = inline
                    .as_ref()
                    .filter(|v| !v.is_empty())
                    .cloned()
                    .or_else(|| args.get(i + 1).map(|s| s.clone()));
                match value {
                    Some(v) if !v.is_empty() => {
                        values.insert(key, ArgValue::Str(v));
                        i += 2
                    }
                    _ => {
                        // empty/"missing" value → undefined (JS falsy)
                        i += if inline.is_some() { 1 } else { 2 }
                    }
                }
            } else {
                errors.push(unknown(tok));
                i += 1
            }
            continue;
        }

        if let Some(rest) = tok.strip_prefix('-') {
            if rest.is_empty() {
                i += 1;
                continue;
            }
            let short = &rest[..1];
            match short_key(spec, short) {
                Some(key) => {
                    if spec.booleans.contains(&key) {
                        values.insert(key.to_string(), ArgValue::Bool(true));
                        i += 1
                    } else {
                        // string short: "-cJPY" attached value, else next token
                        let attached = &rest[1..];
                        let value = if !attached.is_empty() {
                            Some(attached.to_string())
                        } else {
                            args.get(i + 1).map(|s| s.clone())
                        };
                        match value {
                            Some(v) if !v.is_empty() => {
                                values.insert(key.to_string(), ArgValue::Str(v));
                                i += if attached.is_empty() { 2 } else { 1 }
                            }
                            _ => i += 1,
                        }
                    }
                }
                None => {
                    errors.push(unknown(tok));
                    i += 1
                }
            }
            continue;
        }

        // positional — ignored by resolveArgs (path handled by the router)
        i += 1
    }

    ParsedArgs { values, errors }
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
}