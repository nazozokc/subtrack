//! Argument tokenizer + resolver — port of `packages/subtrack/src/cli/parser.ts`
//! (self-contained replacement for gunshi's `args-tokens` + `resolver`).
//! Behavior mirrors the verified gunshi semantics: short groups, `--x=v`,
//! `-x=v`, `--` terminator, kebabized option names, array accumulation.

mod resolve;
mod tokenize;

#[cfg(test)]
mod tests;

pub use resolve::{ArgValue, ParsedArgs, resolve_args};
pub use tokenize::{Token, tokenize};
