//! Table rendering engine — self-contained `table/` heritage port: box
//! borders, section titles, squiggly underline rows, compact/quad modes
//! and an ANSI-aware text measuring module. Themes are injected by
//! callers (they are resolved from `subtrack-config`), so this crate
//! depends only on `subtrack-core`'s ANSI helpers.

pub mod table;
