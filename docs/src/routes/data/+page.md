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

Three tables with a many-to-many relationship:

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
```

Deleting a subscription automatically removes its tag associations via `ON DELETE CASCADE`. Orphaned tags (with no subscriptions) are not automatically cleaned up.

## Prices are stored as integers

Prices are stored as whole numbers (integers) in the database. This avoids floating-point precision issues. For display, prices are formatted with the appropriate currency symbol and decimal places using `Intl.NumberFormat`.

## Backup

Use the `backup` command to create a timestamped copy of your database:

```bash
subtrack backup ~/backups
# Creates: ~/backups/subtrack_20260617_143000.db
```

Backups use exclusive file creation, so they will never overwrite an existing file. See the [Commands](/commands) reference for full details.

## Restore from backup

To restore from a backup, simply copy the backup file to the database location:

```bash
# Stop subtrack (close all running instances)
cp ~/backups/subtrack_20260617_143000.db ~/.config/subtrack/subtrack.db
```

Make sure subtrack is not running when you restore, as changes are written to the database file on every command.

## Custom database directory

Override the default `~/.config/subtrack` directory with the `SUBSC_CLI_DB_DIR` environment variable:

```bash
SUBSC_CLI_DB_DIR=/path/to/custom/dir subtrack list
```

See [Configuration](/configuration) for more details.
