---
title: Commands
description: Full reference for all subtrack CLI commands.
---

subtrack provides the following commands. Most support both interactive and non-interactive modes.

<details>
<summary><strong>Quick navigation</strong></summary>

- [`list`](#list)
- [`add`](#add)
- [`edit`](#edit)
- [`delete`](#delete)
- [`upcoming`](#upcoming)
- [`payment`](#payment)
- [`summary`](#summary)
- [`analytics`](#analytics)
- [`tags`](#tags)
- [`tag`](#tag)
- [`export`](#export)
- [`import`](#import)
- [`backup`](#backup)
- [`restore`](#restore)
- [`config`](#config)
- [`usage`](#usage)

</details>

## `list`

Lists all subscriptions in a formatted table. Subscriptions are grouped by currency by default, with a subtotal row per group.

| Option | Description |
|--------|-------------|
| `-c, --currency <C>` | Convert all prices to the given currency using live exchange rates |
| `--sort <field>` | Sort by field: `name`, `price`, `currency`, `cycle`, `id` (default) |
| `-d, --desc` | Sort in descending order (use with `--sort`) |
| `-a, --api` | Include LLM API usage costs for the current month |

### Examples

```bash
# List all subscriptions (grouped by currency)
subtrack list

# Convert all prices to JPY
subtrack list --currency JPY

# Sort by price (cheapest first)
subtrack list --sort price

# Sort by name, descending
subtrack list --sort name --desc

# Include LLM API usage for current month
subtrack list --api

# Combine with currency conversion
subtrack list --api --currency JPY
```

When `--currency` is used, all prices are converted to the target currency (fetched from [open.er-api.com](https://open.er-api.com)) and displayed as a single group with a grand total.

When `--api` is used, LLM API usage costs for the current month are fetched from the `llm_usage` table and displayed in a separate table below the subscription list, including a provider breakdown.

## `add`

Adds a new subscription. Without flags, prompts for all fields interactively. Providing all flags skips prompts entirely (useful for scripts).

| Option | Description |
|--------|-------------|
| `--name <name>` | Subscription name (max 100 characters) |
| `--price <price>` | Payment amount — integer, non-negative, max 99,999,999 |
| `--currency <C>` | Currency code (ISO 4217). Accepts any 3-letter code; interactive mode provides a curated list |
| `--cycle <cycle>` | Billing cycle. One of: weekly, bi-weekly, monthly, quarterly, semi-annual, yearly |
| `--tags <tags>` | Comma-separated tags (max 10 tags, each max 50 characters) |
| `--status <status>` | Subscription status: `active`, `paused`, `cancelled` (default: `active`) |
| `--billingDay <n>` | Billing day of month (1–31). If not set, defaults to the creation date |

### Examples

```bash
# Interactive mode
subtrack add

# Fully non-interactive (skips confirmation)
subtrack add \
  --name Spotify \
  --price 980 \
  --currency JPY \
  --cycle monthly \
  --tags music

# Partial flags — missing fields are prompted
subtrack add --name Netflix

# Tags with existing tag autocomplete
subtrack add --name "AWS" --price 50 --currency USD --cycle monthly

# Add a paused subscription
subtrack add \
  --name "Adobe CC" \
  --price 6980 \
  --currency JPY \
  --cycle monthly \
  --status paused

# Set a custom billing day
subtrack add \
  --name Netflix \
  --price 1980 \
  --currency JPY \
  --cycle monthly \
  --billingDay 15
```

## `edit`

Edits an existing subscription. Without flags, interactively selects a subscription and fields to change. Flags can be used for non-interactive partial updates.

| Option | Description |
|--------|-------------|
| `[id]` | Subscription ID (optional). If omitted, prompts for selection |
| `--name <name>` | New subscription name |
| `--price <price>` | New payment amount |
| `--currency <C>` | New currency code |
| `--cycle <cycle>` | New billing cycle |
| `--tags <tags>` | New comma-separated tags |
| `--status <status>` | New status: `active`, `paused`, `cancelled` |
| `--billingDay <n>` | New billing day (1–31, or empty to clear) |

### Examples

```bash
# Interactive: select subscription, then pick fields to edit
subtrack edit

# Edit a specific subscription by ID
subtrack edit 3

# Non-interactive: update price only
subtrack edit 3 --price 1500

# Update multiple fields
subtrack edit 3 --name "Netflix Premium" --price 2500 --currency JPY

# Pause a subscription
subtrack edit 3 --status paused

# Change billing day
subtrack edit 3 --billingDay 1

# Clear billing day (use cycle creation date instead)
subtrack edit 3 --billingDay ""
```

Without flags, `edit` shows a multi-select of fields to change. Each selected field is prompted with the current value as default.

## `delete [ids...]`

Shows an interactive checkbox list of all subscriptions. Select one or more to delete. Confirmation is required before deletion.

You can also specify subscription IDs as positional arguments for non-interactive deletion.

### Examples

```bash
# Interactive: checkbox selection
subtrack delete

# Non-interactive: delete by ID(s)
subtrack delete 3
subtrack delete 2 5 7
```

## `upcoming [days]`

Shows subscriptions that are due for billing within the specified number of days (default: 7). Only active and paused subscriptions are included. Bills are calculated based on each subscription's billing cycle and `billingDay` (or creation date if not set).

| Argument | Description |
|----------|-------------|
| `[days]` | Number of days to look ahead (default: 7). Must be a non-negative integer. |

### Examples

```bash
# Bills due in the next 7 days (default)
subtrack upcoming

# Bills due in the next 30 days
subtrack upcoming 30

# Bills due today
subtrack upcoming 0
```

Output is sorted by due date and shows each subscription's name, amount, cycle, and tags. A total row is displayed when multiple subscriptions are shown.

## `payment [period]`

Calculates and displays how much you pay over a given billing period. All subscriptions are automatically converted to the target period based on their billing cycle.

The `period` argument defaults to `monthly`. Valid values:

| Period | Alias |
|--------|-------|
| `weekly` | per week |
| `bi-weekly` | per two weeks |
| `monthly` | per month (default) |
| `quarterly` | per 3 months |
| `semi-annual` | per 6 months |
| `yearly` | per year |

| Option | Description |
|--------|-------------|
| `-c, --currency <C>` | Convert all prices to the given currency using live exchange rates |
| `-a, --api` | Include LLM API usage costs in the total |

### Examples

```bash
# Monthly total (default)
subtrack payment

# Yearly total
subtrack payment yearly

# Weekly total in JPY
subtrack payment weekly --currency JPY

# Include LLM API usage costs
subtrack payment monthly --api

# API costs in a specific currency
subtrack payment monthly --api --currency JPY
```

When `--currency` is used, the total is displayed as a single amount in the target currency. Without it, totals are grouped by currency.

When `--api` is used, API usage costs for the current period are fetched from the `llm_usage` table and added to the subscription totals. API costs are stored in USD cents and are shown both individually and as part of the grand total.

If exchange rates cannot be fetched (e.g. offline), the command falls back to per-currency display without conversion.

## `summary`

Shows a summary of all subscriptions including:

- Total number of subscriptions
- Most expensive subscription
- Monthly spending by currency
- Monthly spending by tag (sorted by cost)

### Example

```bash
subtrack summary
```

Output:

```
Total subscriptions:  5
Most expensive:       AWS ($50.00/month)

Monthly by currency:
  JPY    ¥4,940
  USD    $85.00

Monthly by tag:
  hosting           $50.00/month (1 sub)
  video             $47.00/month (3 subs)
  ...
```

## `analytics`

Shows detailed subscription analytics, including a status breakdown (active/paused/cancelled), monthly spending by currency and tag, and budget tracking if a monthly budget has been configured.

### Example

```bash
subtrack analytics
```

Output:

```
📊 Subscription Analytics

Overview:
  Total subscriptions:  5
  Status breakdown:
    active: 4
    paused: 1

Monthly spending:
  JPY    ¥4,940
  USD    $85.00
  ──────────────────────────────
  Budget:     $500.00
  Remaining:  $415.00

Monthly by tag:
  hosting           $50.00/month (1 sub)
  video             $47.00/month (3 subs)
  ...
```

The budget display requires a monthly budget set via `subtrack config set monthlyBudget <amount>`.

## `tags <taglist...>`

Filters and displays subscriptions that have **all** specified tags (AND logic).

### Examples

```bash
# Subscriptions tagged with "music"
subtrack tags music

# Subscriptions tagged with both "music" AND "video"
subtrack tags music video

# Subscriptions tagged with "entertainment", "video", and "kids"
subtrack tags entertainment video kids
```

## `tag`

Manages tags with the following subcommands:

### `tag list`

Lists all tags with their subscription count.

```bash
subtrack tag list
```

### `tag rename <old> <new>`

Renames a tag. If the new name already exists, the old tag is merged into it.

```bash
subtrack tag rename entertainment fun
```

### `tag delete <name>`

Deletes a tag and removes its associations from subscriptions.

```bash
subtrack tag delete fun
```

### `tag prune`

Removes orphaned tags (tags not associated with any subscription).

```bash
subtrack tag prune
```

## `import <file>`

Imports subscriptions from a CSV file. The CSV must have a header row with exactly `name,cycle,tags,price,currency`.

| Argument | Description |
|----------|-------------|
| `<file>` | Path to the CSV file |
| `--dry-run` | Validate rows without importing |

### CSV format

```
name,cycle,tags,price,currency
Netflix,monthly,video;entertainment,1980,JPY
GitHub Copilot,monthly,development,10,USD
AWS,monthly,cloud;hosting,50,USD
```

- Tags are separated by `;` (semicolon) in the CSV
- Price is an integer (smallest currency unit)
- Currency is an ISO 4217 code
- Cycle must be one of: weekly, bi-weekly, monthly, quarterly, semi-annual, yearly

### Examples

```bash
# Import from file
subtrack import subscriptions.csv

# Dry-run: validate without importing
subtrack import subscriptions.csv --dry-run
```

## `export <format>`

Exports subscriptions to the specified format.

| Argument | Description |
|----------|-------------|
| `<format>` | Export format: `csv`, `json`, or `md` |

| Option | Description |
|--------|-------------|
| `-c, --currency <C>` | Convert all prices to the given currency before exporting |
| `--tags <tags>` | Filter by comma-separated tags before exporting |
| `-o, --output <path>` | Write to file instead of stdout |

### Examples

```bash
# Export as CSV
subtrack export csv

# Export as JSON
subtrack export json

# Export as Markdown
subtrack export md

# Export only tagged subscriptions, converted to JPY
subtrack export csv --tags music,video --currency JPY

# Write to a file instead of stdout
subtrack export csv --output subscriptions.csv
```

## `backup [destination]`

Creates a timestamped gzip-compressed backup of the SQLite database. The backup filename follows the format `subtrack_YYYYMMDD_HHmmss.db.gz`.

If no destination is specified, backups are saved to `~/.config/subtrack/backups/` (created automatically). Backups use exclusive file creation and will never overwrite existing files.

| Option | Description |
|--------|-------------|
| `-e, --encrypt` | Encrypt the backup using AES-256-GCM with your database encryption key |

Encrypted backups use the `.db.enc` extension and require the same derived key material to restore (either the same `.key` file, or the same passphrase **and** persisted salt).

### Examples

```bash
# Backup to default directory (~/.config/subtrack/backups/)
subtrack backup

# Backup to a specific directory
subtrack backup ~/backups

# Backup to current directory
subtrack backup .

# Create an encrypted backup
subtrack backup --encrypt
```

## `restore [file]`

Restores the database from a backup file. If no file is specified, shows an interactive list of available backups from the default backup directory (`~/.config/subtrack/backups/`).

Before restoring, the current database is automatically backed up (timestamped with `_before_restore.db.gz` suffix) as a safety measure.

Each backup has a SHA-256 hash sidecar file (`<backup>.sha256`) for integrity verification. The restore command checks this hash before proceeding and warns if it doesn't match.

| Option | Description |
|--------|-------------|
| `-f, --force` | Skip confirmation prompt and hash check warnings |
| `--dir <path>` | Scan a custom directory for backup files |

### Examples

```bash
# Interactive: select a backup from the default directory
subtrack restore

# Restore from a specific file
subtrack restore ~/backups/subtrack_20260617_143000.db.gz

# Force restore without confirmation
subtrack restore ~/backups/subtrack_20260617_143000.db.gz --force

# List backups from a custom directory
subtrack restore --dir ~/custom-backups
```

## `config`

Manages subtrack configuration. Configuration is stored in `~/.config/subtrack/config.json`.

| Subcommand | Description |
|------------|-------------|
| `list` | List all config values |
| `get <key>` | Get a specific config value |
| `set <key> <value>` | Set a config value |
| `reset` | Reset config to defaults |

### Config keys

| Key | Description | Default |
|-----|-------------|---------|
| `defaultCurrency` | Default currency for display and analytics | `USD` |
| `monthlyBudget` | Monthly spending budget in USD (0 = disabled) | `0` |
| `theme` | Display theme | `default` |

### Examples

```bash
# List all config values
subtrack config list

# Get a specific config value
subtrack config get defaultCurrency

# Set a monthly budget of $500
subtrack config set monthlyBudget 500

# Set default display currency
subtrack config set defaultCurrency JPY

# Reset all config to defaults
subtrack config reset
```

The `config set` command validates input (e.g., currency codes must be ISO 4217, budget must be non-negative).

## `usage`

Tracks LLM API usage costs. Costs are auto-calculated from model pricing when available, with manual fallback.

### `usage add`

Records an LLM API usage entry. Without flags, prompts for all fields interactively.

| Option | Description |
|--------|-------------|
| `--provider <name>` | Provider: `openai`, `anthropic`, `google-ai`, `mistral`, `groq`, `together`, `deepseek`, `cohere`, or custom |
| `--model <name>` | Model name (e.g. `gpt-4o`, `claude-3-opus-20240229`) |
| `--inputTokens <n>` | Input token count |
| `--outputTokens <n>` | Output token count |
| `--date <YYYY-MM-DD>` | Date of usage (default: today) |
| `--description <text>` | Optional description |
| `--cost <amount>` | Total cost in USD (e.g. `0.50` for 50 cents; overrides auto-pricing) |

```bash
# Interactive mode
subtrack usage add

# Non-interactive
subtrack usage add \
  --provider openai \
  --model gpt-4o \
  --inputTokens 500 \
  --outputTokens 200 \
  --date 2026-06-19 \
  --description "Chat completion"

# Override auto-calculated cost
subtrack usage add \
  --provider openai \
  --model gpt-4o \
  --inputTokens 500 \
  --outputTokens 200 \
  --cost 0.15
```

Cost is calculated automatically via the LiteLLM pricing cache (fetched from GitHub, cached for 24 hours). If pricing is not found, the tool falls back to querying the LiteLLM Model Catalog API, then prompts for manual cost input. Use `--cost` to override auto-pricing entirely.

### `usage list`

Lists LLM API usage entries with optional filtering.

| Option | Description |
|--------|-------------|
| `--provider <name>` | Filter by provider |
| `--from <YYYY-MM-DD>` | Start date (inclusive) |
| `--to <YYYY-MM-DD>` | End date (inclusive) |

```bash
# List all entries
subtrack usage list

# Filter by provider and date range
subtrack usage list --provider openai --from 2026-01-01 --to 2026-06-30
```

Shows up to 100 entries with provider, model, token counts, cost, date, and description. Displays a total cost at the bottom.

### `usage delete`

Interactively selects and deletes LLM API usage entries.

```bash
subtrack usage delete
```

Multi-select via checkbox → confirm → batch delete. You can also pass entry IDs directly:

```bash
subtrack usage delete 3 5 7
```

### `usage import`

Imports LLM API usage from JSONL or JSON response log files. Supports importing from provider API response formats (e.g. OpenAI, Anthropic) by parsing token usage from the response body.

| Option | Description |
|--------|-------------|
| `<file>` | JSONL/JSON file to import (use `-` for stdin) |
| `--dry-run` | Validate without importing |

```bash
# Import from a JSONL file
subtrack usage import ./openai-responses.jsonl

# Import from stdin
cat responses.jsonl | subtrack usage import -

# Dry-run to validate
subtrack usage import ./responses.jsonl --dry-run
```

### `usage refresh`

Auto-scans known AI tool sources to find and import LLM usage data. Scans the following sources by default:

- **OpenCode DB** — reads from OpenCode's own database
- **Claude Code** — parses Claude Code CLI usage logs
- **Codex CLI** — parses Codex CLI logs
- **Cursor** — parses Cursor editor usage history
- **GitHub Copilot** — parses Copilot CLI usage
- **Windsurf** — parses Windsurf editor usage

By default, scans the current month. Also refreshes the LiteLLM pricing cache from GitHub.

| Option | Description |
|--------|-------------|
| `--from <YYYY-MM-DD>` | Start date (inclusive) |
| `--to <YYYY-MM-DD>` | End date (inclusive) |
| `--all` | Scan all historical data (ignore date range) |

```bash
# Scan current month (default)
subtrack usage refresh

# Scan a specific date range
subtrack usage refresh --from 2026-01-01 --to 2026-06-30

# Scan all available history
subtrack usage refresh --all

# Force-refresh pricing cache only (no scanner data import)
# (This is the default behavior of the old refresh — pricing is auto-refreshed daily)
```
