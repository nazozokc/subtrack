//! Command resolution layer — port of `commands.rs` (the registry), the
//! argument router and the usage renderer. No dispatching and no IO
//! beyond buffered logging: `subtrack`'s CLI calls
//! `router::resolve_command` + `help::render_usage`.

pub mod commands;
pub mod help;
pub mod router;
