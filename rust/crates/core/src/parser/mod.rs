//! Argument tokenizer + resolver — port of `packages/subtrack/src/cli/parser.ts`
//! (self-contained replacement for gunshi's `args-tokens` + `resolver`).
//! Behavior mirrors the verified gunshi semantics: short groups, `--x=v`,
//! `-x=v`, `--` terminator, kebabized option names, array accumulation.

mod resolve;
mod tokenize;

#[cfg(test)]
mod tests;

/// Argument / option kinds appearing in subtrack commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgKind {
    Boolean,
    String,
    Positional,
}

/// One argument/option schema entry (help surface).
#[derive(Clone, Copy, Debug)]
pub struct ArgSpec<'a> {
    pub name: &'a str,
    pub kind: ArgKind,
    pub short: Option<char>,
    pub description: &'a str,
    pub array: bool,
    pub required: Option<bool>,
}

pub use resolve::{ArgValue, ParsedArgs, resolve_args};
pub use tokenize::{Token, tokenize};
