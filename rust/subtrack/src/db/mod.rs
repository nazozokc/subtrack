//! Database access — port of `db/subscriptions.ts` + `db/usage.ts` queries.
//! The on-disk `subtrack.db` is decrypted to a temp backing file and opened
//! with rusqlite; the temp file is removed after opening.

mod audit;
mod backup;
mod history;
mod migrations;
mod store;
mod subscriptions;
mod suggestions;
mod tags;
mod trials;
mod usage;

pub use audit::{
    add_audit_log, count_audit_before, get_audit_log_count, get_audit_logs,
    get_audit_logs_filtered, prune_audit_logs,
};
pub use backup::{
    default_backup_dir, export_db_bytes, restore_db, verify_backup_hash, write_backup_hash,
};
pub use history::{
    PriceChange, get_all_price_changes, get_price_changes_recent, get_price_history,
};
pub use migrations::{open_db, run_migrations};
pub use store::{db_file_size, db_path, integrity_check, save_db, vacuum_db};
pub use subscriptions::{
    NewSubscription, QueryOptions, SqlValue, Subscription, archive_subscription,
    find_subscription_by_name, get_subscription_by_id, get_subscriptions, remove_subscription,
    replace_tags, search_subscriptions, set_subscription_tags, unarchive_subscription,
    update_subscription, write_price_history, write_subscription,
};
pub use suggestions::{
    NewSuggestion, dismiss_all_suggestions, dismiss_suggestion, get_suggestion, get_suggestions,
    mark_suggestion_added, write_suggestion_batch,
};
pub use tags::{get_tags_with_count, prune_tags, remove_tag, rename_tag};
pub use trials::{
    Trial, get_trial, get_trials, get_trials_expiring_soon, insert_trial, remove_trial,
};
pub use usage::{
    LlmUsage, add_llm_usage_from_log, get_llm_usage, get_llm_usage_token_total,
    get_llm_usage_total, get_llm_usage_total_by_model, get_llm_usage_total_by_provider,
    insert_llm_usage, remove_llm_usage, update_llm_usage,
};
