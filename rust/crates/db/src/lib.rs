//! SQLite persistence layer — port of `db/*` queries. The on-disk
//! `subtrack.db` is decrypted to a temp backing file and opened with
//! rusqlite; the temp file is removed after opening. Row *types* (the
//! domain models) live in `subtrack-core`; this crate owns storage,
//! migrations, encrypted persistence and history tables.

pub mod db;
