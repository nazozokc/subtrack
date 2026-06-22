---
title: Data & Storage
description: How subtrack stores data, database location, backup and restore.
---

## Database location

All subscription data is stored in a single SQLite database file at:

```
~/.config/subtrack/subtrack.db
```

The database is created automatically on first use. No database server or configuration is required.

## Database structure

Four tables — three for subscriptions with a many-to-many relationship, and one for LLM API usage tracking:

```
subscriptions
├── id        INTEGER PRIMARY KEY AUTOINCREMENT
├── name      TEXT NOT NULL
├── price     INTEGER NOT NULL
├── currency  TEXT NOT NULL
└── cycle     TEXT NOT NULL

tags
├── id   INTEGER PRIMARY KEY AUTOINCREMENT
└── name TEXT NOT NULL UNIQUE

subscription_tags
├── subscription_id  INTEGER NOT NULL (FK → subscriptions.id)
└── tag_id           INTEGER NOT NULL (FK → tags.id)
│                    PRIMARY KEY (subscription_id, tag_id)

llm_usage
├── id             INTEGER PRIMARY KEY AUTOINCREMENT
├── provider       TEXT NOT NULL
├── model          TEXT NOT NULL
├── input_tokens   INTEGER NOT NULL DEFAULT 0
├── output_tokens  INTEGER NOT NULL DEFAULT 0
├── cost           REAL NOT NULL
├── date           TEXT NOT NULL
└── description    TEXT
```

Deleting a subscription automatically removes its tag associations via `ON DELETE CASCADE`. Orphaned tags (with no subscriptions) can be cleaned up with `subtrack tag prune`.

## Prices are stored as integers

Prices are stored as whole numbers (integers) in the database. This avoids floating-point precision issues. For display, prices are formatted with the appropriate currency symbol and decimal places using `Intl.NumberFormat`.

## Backup

Use the `backup` command to create a timestamped gzip-compressed copy of your database:

```bash
# Backup to the default directory (~/.config/subtrack/backups/)
subtrack backup
# Creates: ~/.config/subtrack/backups/subtrack_20260617_143000.db.gz

# Backup to a custom directory
subtrack backup ~/backups
# Creates: ~/backups/subtrack_20260617_143000.db.gz
```

If no destination is specified, backups are saved to `~/.config/subtrack/backups/` (created automatically). Backups use exclusive file creation, so they will never overwrite an existing file. See the [Commands](/commands) reference for full details.

## Restore from backup

Use the `restore` command to restore from a backup. Without arguments, it interactively lists available backups:

```bash
# Interactive: select from available backups
subtrack restore

# Restore from a specific file
subtrack restore ~/backups/subtrack_20260617_143000.db.gz
```

Before restoring, the current database is automatically backed up as a safety measure. You can also restore manually:

```bash
# Stop subtrack (close all running instances)
cp ~/backups/subtrack_20260617_143000.db.gz ~/.config/subtrack/subtrack.db.gz
gunzip -k ~/.config/subtrack/subtrack.db.gz
```

Make sure subtrack is not running when you restore, as changes are written to the database file on every command.

## Custom database directory

Override the default `~/.config/subtrack` directory with the `SUBSC_CLI_DB_DIR` environment variable:

```bash
SUBSC_CLI_DB_DIR=/path/to/custom/dir subtrack list
```

See [Configuration](/configuration) for more details.
