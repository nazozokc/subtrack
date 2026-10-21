---
title: Commands
description: Full reference for all subtrack CLI commands.
---

subtrack provides the following 50 commands. Every command can run non-interactively from flags and positional arguments. When invoked without required arguments on a terminal, commands prompt for the missing values; the bare `subtrack` command opens a menu. See [Scripting notes](#scripting-notes).

## Scripting notes

- **TTY-aware output.** The `subtrack vX.Y.Z` header line is printed only when stdout is a terminal. Piped output is plain data with no banner or ANSI escapes, so `subtrack list | jq` works without extra flags. `-j, --json` and `subtrack mcp` always suppress the header.
- **Color control.** Colors follow `NO_COLOR`, `FORCE_COLOR`, `CLICOLOR`, `CLICOLOR_FORCE`, and `TERM=dumb`. Override per run with `--no-color` or `--color=auto|always|never`. `--no-color` wins if both are present.
- **Width.** Tables use the terminal width (or `COLUMNS` when set). `--wide` forces the wide layout, which is useful for long names and notes.
- **Interactive prompts.** Prompt flows run when stdin is a terminal (or `SUBTRACK_ASSUME_TTY=1`). Flags always take precedence, and `--json` never prompts. In a non-TTY session the scriptable behavior is unchanged: missing required input exits 0 without prompting (e.g. bare `subtrack add`, `edit`, `delete`, `restore`, `search`), or the command aborts without changes (e.g. `cancel` without `--force`).
- **Exit codes for automation.** `budget --check` exits 1 when over budget; `check --strict` exits 1 when data problems are found.
- **Diagnostics use stderr.** Errors and warnings are written to stderr; data and success messages go to stdout.
- **Parent commands have useful defaults.** `subtrack config`, `suggest`, `tag`, `template`, `profile`, and `usage` run their `list` subcommand. Bare `subtrack forecast` runs a 12-month forecast, and bare `subtrack search` prints a query hint in non-TTY sessions.
- **Unknown flags fail loudly.** A misspelled option aborts with an error and exit code 1: `subtrack list --min-pricee 500` → `✖ --min-pricee: Unknown option '--min-pricee'`. Typos are never silently dropped.

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
- [`clone`](#clone)
- [`archive`](#archive)
- [`unarchive`](#unarchive)
- [`search`](#search)
- [`trial`](#trial)
- [`bulk`](#bulk)
- [`forecast`](#forecast)
- [`history`](#history)
- [`timeline`](#timeline)
- [`optimize`](#optimize)
- [`compare`](#compare)
- [`calendar`](#calendar)
- [`notify`](#notify)
- [`profile`](#profile)
- [`audit`](#audit)
- [`maintenance`](#maintenance)
- [`cleanup`](#cleanup)
- [`stats`](#stats)
- [`currency`](#currency)
- [`backup`](#backup)
- [`restore`](#restore)
- [`config`](#config)
- [`usage`](#usage)
- [`mcp`](#mcp)
- [`pause`](#pause)
- [`resume`](#resume)
- [`renew`](#renew)
- [`review`](#review)
- [`yearly`](#yearly)
- [`check`](#check)
- [`changes`](#changes)
- [`receipt`](#receipt)
- [`template`](#template)
- [`suggest`](#suggest)
- [`budget`](#budget)
- [`dedupe`](#dedupe)
- [`cancel`](#cancel)
- [`report`](#report)

::: tip Not listed here?
`subtrack <command> --help` is always authoritative. Option names are
case-sensitive, and unknown flags abort with an error (exit 1) instead of
being dropped, so copy-paste mistakes fail loudly. Common traps:
`usage` uses kebab-case (`--input-tokens`), while `add` / `edit` / `forecast`
use camelCase (`--billingDay`, `--addName`).
:::

</details>

## `list`

Lists all subscriptions in a formatted table. Subscriptions are grouped by currency by default, with a subtotal row per group. Each row shows the subscription's `next` billing date (highlighted in red within 3 days, yellow within 7 days); cancelled/archived subscriptions show `-`.

| Option | Description |
|--------|-------------|
| `-c, --currency <C>` | Convert all prices to the given currency using live exchange rates |
| `--sort <field>` | Sort by field: `name`, `price`, `currency`, `cycle`, `status`, `id` (default) |
| `-d, --desc` | Sort in descending order (use with `--sort`) |
| `-a, --api` | Include LLM API usage costs for the current month |
| `-n, --notes` | Show notes column |
| `-m, --method` | Show payment method column |
| `--contract` | Show contract dates column |
| `--vendor` | Show vendor column |
| `--status <status>` | Filter by status: `active`, `paused`, `cancelled`, `archived` |
| `--min-price <n>` | Filter by minimum price (major currency units, e.g. `14.99`) |
| `--max-price <n>` | Filter by maximum price (major currency units, e.g. `14.99`) |
| `-j, --json` | Output as JSON |
| `--tags <tags>` | Comma-separated tag names to filter by (AND logic) |
| `--limit <n>` | Max number of items to show |
| `--offset <n>` | Number of items to skip |
| `--include-archived` | Include archived subscriptions |

Archived subscriptions are hidden unless `--include-archived` is passed or
`--status archived` is requested explicitly.

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

# Show payment method and notes columns
subtrack list --method --notes

# Show contract dates and vendor columns
subtrack list --contract --vendor

# Filter by tags
subtrack list --tags music,video

# Filter by status and price range
subtrack list --status active --min-price 500 --max-price 5000

# Page through results
subtrack list --limit 20 --offset 40

# JSON output for scripting
subtrack list --json

# Include archived subscriptions
subtrack list --include-archived
```

When `--currency` is used, all prices are converted to the target currency (fetched from [open.er-api.com](https://open.er-api.com)) and displayed as a single group with a grand total.

When `--api` is used, LLM API usage costs for the current month are fetched from the `llm_usage` table and displayed in a separate table below the subscription list, including a provider breakdown.

## `add`

Adds a new subscription. Requires at least `--name`, `--price`, `--currency`, `--cycle`, and `--status`. On a terminal, any field not supplied as a flag is prompted for — including optional metadata and a final save confirmation. Providing all flags skips every prompt (useful for scripts); in a non-TTY session missing required input exits 0 without creating anything.

| Option | Description |
|--------|-------------|
| `--name <name>` | Subscription name (max 100 characters) |
| `--price <price>` | Payment amount in major currency units — non-negative, max 99,999,999 |
| `--currency <C>` | Currency code (ISO 4217). Accepts any 3-letter code; run `subtrack currency` for the curated list of 37 |
| `--cycle <cycle>` | Billing cycle. One of: weekly, bi-weekly, monthly, quarterly, semi-annual, yearly, or `Nd` for a custom day count (e.g. `3d`, 1–365) |
| `--tags <tags>` | Comma-separated tags (max 10 tags, each max 50 characters) |
| `--status <status>` | Subscription status: `active`, `paused`, `cancelled` (default: `active`) |
| `--billingDay <n>` | Billing day of month (1–31). If not set, defaults to the creation date |
| `--paymentMethod <method>` | Payment method (e.g. `credit_card`, `paypal`) |
| `--vendorName <name>` | Vendor name (max 100 characters) |
| `--vendorUrl <url>` | Vendor URL |
| `--planTier <tier>` | Plan tier (e.g. `Pro`, `Family`) |
| `--discountAmount <n>` | Discount amount in major currency units — non-negative |
| `--discountType <type>` | Discount type: `percentage` or `fixed` |
| `--contractStart <date>` | Contract start date (`YYYY-MM-DD`) |
| `--contractEnd <date>` | Contract end date (`YYYY-MM-DD`) |
| `--autoRenewal <bool>` | Auto renew: `true` or `false` (default: `true`) |

### Examples

```bash
# Fully non-interactive
subtrack add \
  --name Spotify \
  --price 980 \
  --currency JPY \
  --cycle monthly \
  --status active \
  --tags music

# Minimal form: only the required flags
subtrack add \
  --name Netflix \
  --price 14.99 \
  --currency USD \
  --cycle monthly \
  --status active

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

# Set payment method
subtrack add \
  --name Spotify \
  --price 980 \
  --currency JPY \
  --cycle monthly \
  --paymentMethod credit_card
```

## `edit`

Edits an existing subscription. `[id]` selects the target subscription. On a terminal, omitting the ID prompts for one and then lets you pick the fields to change from a checkbox list; each selected field gets its own prompt. Flags skip the prompts. In a non-TTY session, omitting the ID exits 0 and does nothing.

| Option | Description |
|--------|-------------|
| `[id]` | Subscription ID (prompted on a terminal; without ID in a non-TTY session the command exits 0) |
| `--name <name>` | New subscription name |
| `--price <price>` | New payment amount |
| `--currency <C>` | New currency code |
| `--cycle <cycle>` | New billing cycle |
| `--tags <tags>` | New comma-separated tags |
| `--status <status>` | New status: `active`, `paused`, `cancelled` |
| `--billingDay <n>` | New billing day (1–31, or empty to clear) |
| `--paymentMethod <method>` | New payment method |
| `--vendorName <name>` | New vendor name (or empty to clear) |
| `--vendorUrl <url>` | New vendor URL (or empty to clear) |
| `--planTier <tier>` | New plan tier (or empty to clear) |
| `--discountAmount <n>` | New discount amount (or empty to clear) |
| `--discountType <type>` | New discount type: `percentage` or `fixed` |
| `--contractStart <date>` | New contract start date (or empty to clear) |
| `--contractEnd <date>` | New contract end date (or empty to clear) |
| `--autoRenewal <bool>` | Auto renew: `true` or `false` |

### Examples

```bash
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

# Change payment method
subtrack edit 3 --paymentMethod paypal
```

Without any field flags, `edit <id>` opens the field picker on a terminal. In a non-TTY session it prints the subscription's current values and exits without changing anything.

## `delete [ids...]`

Deletes one or more subscriptions by ID. On a terminal, omitting the IDs opens a checkbox list and asks for confirmation. Explicit IDs delete immediately (no confirmation) and are recorded in the audit log. In a non-TTY session, omitting the IDs exits 0 and does nothing.

### Examples

```bash
# Delete one subscription by ID
subtrack delete 3

# Delete several subscriptions
subtrack delete 2 5 7
```

## `upcoming [days]`

Shows subscriptions that are due for billing within the specified number of days (default: 7). Only active and paused subscriptions are included. Bills are calculated based on each subscription's billing cycle and `billingDay` (or creation date if not set).

| Argument | Description |
|----------|-------------|
| `[days]` | Number of days to look ahead (default: 7). Must be a non-negative integer. |

| Option | Description |
|--------|-------------|
| `-j, --json` | Output as JSON |
| `-c, --currency <C>` | Convert all prices to the given currency |

### Examples

```bash
# Bills due in the next 7 days (default)
subtrack upcoming

# Bills due in the next 30 days
subtrack upcoming 30

# Bills due today
subtrack upcoming 0

# Convert all amounts to JPY
subtrack upcoming 30 --currency JPY

# JSON output
subtrack upcoming --json
```

## `clone <id>`

Clones an existing subscription. Creates a new subscription pre-filled with all fields from the source, with optional overrides.

| Argument | Description |
|----------|-------------|
| `<id>` | Subscription ID to clone (required) |

| Option | Description |
|--------|-------------|
| `--name <name>` | New name (default: `<original> (copy)`) |
| `--price <price>` | Override price |
| `--currency <C>` | Override currency |
| `--cycle <cycle>` | Override billing cycle |
| `--tags <tags>` | Override tags (comma-separated) |

### Examples

```bash
# Clone subscription 3 with auto-naming
subtrack clone 3

# Clone with custom name
subtrack clone 3 --name "Netflix (Secondary)"

# Clone with overrides
subtrack clone 3 --price 1500 --tags "video,4k"
```

## `archive <id>`

Archives a subscription by setting its status to `archived`. Archived subscriptions are preserved in the database but excluded from all payment calculations and reports. Unlike `cancelled`, archived subscriptions are intended for long-term record-keeping.

| Argument | Description |
|----------|-------------|
| `<id>` | Subscription ID to archive (required) |

```bash
# Archive subscription 3
subtrack archive 3
```

## `unarchive <id>`

Unarchives a subscription, restoring its status to `active`.

| Argument | Description |
|----------|-------------|
| `<id>` | Subscription ID to unarchive (required) |

```bash
# Unarchive subscription 3
subtrack unarchive 3
```

Output is sorted by due date and shows each subscription's name, amount, cycle, and tags. A total row is displayed when multiple subscriptions are shown.

## `payment [period]`

Calculates and displays how much you pay over a given billing period. All subscriptions are automatically converted to the target period based on their billing cycle.

The `period` argument defaults to `monthly`. Valid values:

| Period | Alias | Suffix |
|--------|-------|--------|
| `weekly` | per week | `/week` |
| `bi-weekly` | per two weeks | `/2 weeks` |
| `monthly` | per month (default) | `/month` |
| `quarterly` | per 3 months | `/quarter` |
| `semi-annual` | per 6 months | `/6 months` |
| `yearly` | per year | `/year` |

| Option | Description |
|--------|-------------|
| `-c, --currency <C>` | Convert all prices to the given currency using live exchange rates |
| `-a, --api` | Include LLM API usage costs in the total |
| `-m, --method` | Group by payment method |
| `-j, --json` | Output as JSON |

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

# Group by payment method
subtrack payment --method

# JSON output
subtrack payment --json
```

When `--currency` is used, the total is displayed as a single amount in the target currency. Without it, totals are grouped by currency.

When `--api` is used, API usage costs for the current period are fetched from the `llm_usage` table and added to the subscription totals. API costs are stored in USD cents and are shown both individually and as part of the grand total.

If exchange rates cannot be fetched (e.g. offline), the command falls back to per-currency display without conversion.

## `summary`

Shows a summary of all subscriptions including:

- Total number of subscriptions
- Most expensive subscription per currency (raw prices are never compared across currencies)
- Monthly spending by currency
- Monthly spending by tag (sorted by cost)

When every subscription uses the same currency, the headline is `Most expensive:`. With mixed currencies it becomes `Most expensive by currency:` so a ¥250 yearly plan is never ranked against a $15.99 monthly plan. In JSON, the single-currency form uses `mostExpensive`; the mixed form uses `mostExpensiveByCurrency`.

| Option | Description |
|--------|-------------|
| `-j, --json` | Output as JSON |

### Example

```bash
subtrack summary

# JSON output
subtrack summary --json
```

Output:

```
Total subscriptions:  5
Most expensive by currency:
  JPY  AWS (¥4,940/month)
  USD  Adobe ($50.00/month)

Monthly by currency:
  JPY    ¥4,940
  USD    $85.00

Monthly by tag:
  hosting           $50.00/month (1 sub)
  video             $47.00/month (3 subs)
  ...
```

## `analytics`

Shows detailed subscription analytics, including a status breakdown (active/paused/cancelled/archived), monthly spending by currency and tag, and budget tracking if a monthly budget has been configured.

| Option | Description |
|--------|-------------|
| `-c, --currency <C>` | Convert all prices to target currency |
| `--period <period>` | Period: `monthly`, `yearly` (default: `monthly`) |
| `-j, --json` | Output as JSON |

### Example

```bash
subtrack analytics

# Yearly analytics converted to JPY
subtrack analytics --period yearly --currency JPY

# JSON output
subtrack analytics --json
```

Output:

```
Subscription Analytics

Overview:
  Total subscriptions:  5
  Status breakdown:
    active: 4
    paused: 1
  Most expensive by currency:
    JPY  AWS (¥4,940/month)
    USD  Adobe ($50.00/month)

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

With `--period yearly`, the same tag lines use `/year`. As with `summary`, mixed currencies are never ranked against each other: the overview lists one most-expensive subscription per currency. The budget display requires a monthly budget set via `subtrack config set monthlyBudget <amount>`.

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
# List all tags
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

### `tag merge <source> <target>`

Merges a source tag into a target tag. All subscriptions tagged with the source tag are reassigned to the target tag, and the source tag is deleted. If the target tag doesn't exist, it's created.

```bash
subtrack tag merge entertainment fun
```

## `import <file>`

Imports subscriptions from a CSV file. The CSV must have a header row with `name,cycle,tags,price,currency` (an optional `notes` column is also accepted). CSVs produced by `subtrack export csv` can be imported as-is (extra columns such as `status`, `billing_day`, `payment_method`, and contract fields are preserved).

| Argument | Description |
|----------|-------------|
| `<file>` | Path to the CSV file |
| `--dry-run` | Validate rows without importing |
| `--deduplicate` | Skip rows whose name already exists |

### CSV format

```
name,cycle,tags,price,currency
Netflix,monthly,video;entertainment,1980,JPY
GitHub Copilot,monthly,development,10,USD
AWS,monthly,cloud;hosting,50,USD
```

- Tags are separated by `;` (semicolon) in the CSV
- Price is a non-negative number in major currency units
- Currency is an ISO 4217 code
- Cycle must be one of: weekly, bi-weekly, monthly, quarterly, semi-annual, yearly, or `Nd` for a custom day count (e.g. `3d`, 1–365)
- Optional columns recognised by name: `status`, `notes`, `billing_day` (1–31), `payment_method`, `contract_start`, `contract_end`, `auto_renewal`, `vendor_name`, `vendor_url`, `plan_tier`, `discount_amount`, `discount_type`

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
| `<format>` | Export format: `csv`, `json`, `md`, `excel`, `ics` |

| Option | Description |
|--------|-------------|
| `-c, --currency <C>` | Convert all prices to the given currency before exporting |
| `--tags <tags>` | Filter by comma-separated tags before exporting |
| `--status <status>` | Filter by status (comma-separated: `active`, `paused`, `cancelled`) |
| `-o, --output <path>` | Write to file instead of stdout |

### Examples

```bash
# Export as CSV
subtrack export csv

# Export as JSON
subtrack export json

# Export as Markdown
subtrack export md

# Export as Excel (.xlsx)
subtrack export excel

# Export as iCalendar (.ics) — importable into calendar apps
subtrack export ics

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

Restores the database from a backup file. Pass `--force` to proceed — without it the command takes no action and the current database is left unchanged.

Before restoring, the current database is automatically backed up as a safety measure (timestamped with `_before_restore.db.enc` suffix, encrypted).

Each backup has a SHA-256 hash sidecar file (`<backup>.sha256`) for integrity verification. The restore command checks this hash before proceeding and warns if it doesn't match.

| Option | Description |
|--------|-------------|
| `-f, --force` | Skip confirmation prompt and hash check warnings |
| `--dir <dir>` | Directory to scan for backup files |

### Examples

```bash
# Restore from a specific file (confirmation step skipped)
subtrack restore ~/backups/subtrack_20260617_143000.db.gz --force
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
| `yearlyBudget` | Yearly spending budget (used by `budget --period yearly` and `report`) | — |
| `budgets` | JSON array of named budgets (see below) | — |
| `theme` | Display theme preset: `default`, `light`, `high-contrast`, `none` | `default` |
| `tableBorderColor` | Table border color override (color name) | theme default |
| `tableHeaderColor` | Table header color override (color name) | theme default |
| `tableZebraColor` | Zebra stripe background color override (color name) | theme default |
| `accentColor` | Accent color override for headings (color name) | theme default |
| `tableZebra` | Zebra striping on/off | `on` |
| `tableMinWidth` | Minimum table width in columns (20–200) | `40` |
| `dateFormat` | Date display format: `iso` or `short` | `iso` |
| `listShowNotes` | Show notes column in `list` by default | `off` |
| `listShowMethod` | Show payment method column in `list` by default | `off` |
| `profiles` | Saved filter profiles (JSON object) | `{}` |
| `activeProfile` | Currently active filter profile name | — |
| `templates` | Reusable subscription templates (JSON object) | `{}` |
| `notifyDays` | Notification look-ahead in days | `7` |
| `notifyChannels` | Comma-separated channels: `os`, `slack`, `webhook` | `os` |
| `slackWebhook` | Slack webhook URL for `slack` notifications | — |
| `webhookUrl` | Generic webhook URL for `webhook` notifications | — |

Named budgets accept a JSON array of entries with `name`, `amount`, `currency`,
and optional `period` (`monthly`/`yearly`) and `categories` (tag filter):

```bash
subtrack config set budgets \
  '[{"name":"streaming","amount":3000,"currency":"JPY","categories":["video","music"]}]'
```

### Examples

```bash
# List all config values
subtrack config list

# Get a specific config value
subtrack config get defaultCurrency

# Set a monthly budget of $500
subtrack config set monthlyBudget 500

# Set a yearly budget
subtrack config set yearlyBudget 60000

# Set default display currency
subtrack config set defaultCurrency JPY

# Send notifications to both desktop and Slack
subtrack config set notifyChannels os,slack
subtrack config set slackWebhook https://hooks.slack.com/services/...

# Switch to the light theme (for light terminal backgrounds)
subtrack config set theme light

# Override a single color on top of a preset
subtrack config set accentColor yellow

# Disable zebra striping entirely
subtrack config set tableZebra off

# Reset all config to defaults
subtrack config reset
```

The `config set` command validates input (e.g., currency codes must be ISO 4217, budget must be non-negative, channels must be one of `os`/`slack`/`webhook`).

The `theme` key switches between four presets: `default` (dark backgrounds), `light`, `high-contrast`, and `none` (plain monochrome). Individual color keys (`tableBorderColor`, `tableHeaderColor`, `tableZebraColor`, `accentColor`) override the preset. Valid color names: `black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`, `gray`, and the `bright*` variants.

`dateFormat` controls human-facing dates in `cancel` and `notify` (`short` uses `MM/DD` style); machine-readable output (JSON) always uses ISO 8601. `listShowNotes` / `listShowMethod` enable the notes and payment method columns in `subtrack list` by default — CLI flags still take precedence.

## `usage`

Tracks LLM API usage costs. Costs are auto-calculated from model pricing when available, with manual fallback.

### `usage add`

Records an LLM API usage entry. Flags supply any field; on a terminal, omitted values (provider, model, cost, tokens, date, description) are prompted for. In a non-TTY session `--provider` and `--model` are required, and `--cost` is required when no pricing can be resolved — see below.

| Option | Description |
|--------|-------------|
| `--provider <name>` | Provider: `openai`, `anthropic`, `google-ai`, `mistral`, `groq`, `together`, `deepseek`, `cohere`, or custom |
| `--model <name>` | Model name (e.g. `gpt-4o`, `claude-3-opus-20240229`) |
| `--input-tokens <n>` | Input token count |
| `--output-tokens <n>` | Output token count |
| `--date <YYYY-MM-DD>` | Date of usage (default: today) |
| `--description <text>` | Optional description |
| `--cost <amount>` | Total cost in USD (e.g. `0.50` for 50 cents; overrides auto-pricing) |

```bash
# Non-interactive
subtrack usage add \
  --provider openai \
  --model gpt-4o \
  --input-tokens 500 \
  --output-tokens 200 \
  --date 2026-06-19 \
  --description "Chat completion"

# Override auto-calculated cost
subtrack usage add \
  --provider openai \
  --model gpt-4o \
  --input-tokens 500 \
  --output-tokens 200 \
  --cost 0.15
```

Cost is calculated automatically via the LiteLLM pricing cache (fetched from GitHub, cached for 24 hours). If no pricing can be resolved for the model (unknown to the cache/catalog, or offline), the command fails with an error instructing you to pass `--cost`. Use `--cost` to override auto-pricing entirely.

### `usage list`

Lists LLM API usage entries with optional filtering.

| Option | Description |
|--------|-------------|
| `--provider <name>` | Filter by provider |
| `--from <YYYY-MM-DD>` | Start date (inclusive) |
| `--to <YYYY-MM-DD>` | End date (inclusive) |
| `--limit <n>` | Max entries to show (default: 100) |
| `--offset <n>` | Skip the first N entries (for paging) |
| `-j, --json` | Output as JSON |

```bash
# List all entries
subtrack usage list

# Filter by provider and date range
subtrack usage list --provider openai --from 2026-01-01 --to 2026-06-30

# Page through entries
subtrack usage list --limit 50 --offset 100

# JSON output
subtrack usage list --json
```

Shows up to 100 entries by default (configurable with `--limit`) with provider, model, token counts, cost, date, and description. Displays a total cost at the bottom.

### `usage edit`

Updates fields of an existing LLM API usage entry. Only the fields you pass as flags are changed; everything else is left untouched.

| Option | Description |
|--------|-------------|
| `<id>` | Entry ID to edit (required) |
| `--provider <name>` | New provider name |
| `--model <name>` | New model name |
| `--input-tokens <n>` | New input token count |
| `--output-tokens <n>` | New output token count |
| `--date <YYYY-MM-DD>` | New date |
| `--description <text>` | New description (empty string clears it) |
| `--cost <USD>` | New total cost in USD (e.g. `0.50` for 50 cents) |

```bash
# Fix the cost of entry 3
subtrack usage edit 3 --cost 0.75

# Update model and tokens
subtrack usage edit 5 --model gpt-4o-mini --input-tokens 500 --output-tokens 250

# Clear the description
subtrack usage edit 7 --description ""
```

### `usage delete`

Deletes one or more usage entries by ID in a single batch.

```bash
subtrack usage delete 3 5 7
```

### `usage import`

Imports LLM API usage from JSONL or JSON response log files. Supports importing from provider API response formats (e.g. OpenAI, Anthropic) by parsing token usage from the response body.

| Option | Description |
|--------|-------------|
| `<file>` | JSONL/JSON file to import (use `-` for stdin) |
| `--dry-run` | Validate without importing |

Input size protections:
- **File import**: max 50 MB
- **Stdin import**: max 10 MB, with a 30-second timeout
- Exceeding these limits will abort with an error message

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
```

### `usage total`

Shows aggregated LLM API usage for a given period: cost broken down by provider and by model, plus total input/output tokens.

| Option | Description |
|--------|-------------|
| `--from <YYYY-MM-DD>` | Start date (inclusive) |
| `--to <YYYY-MM-DD>` | End date (inclusive) |
| `--period <period>` | Period: `monthly`, `quarterly`, `yearly` (default: `monthly`) |
| `-j, --json` | Output as JSON |

When neither `--from`/`--to` nor `--period` is specified, defaults to the current month. The JSON output includes `total` (cost in USD cents), `tokens` (`inputTokens`/`outputTokens`), `byProvider`, and `byModel` (per-model cost and token totals).

```bash
# Current month total
subtrack usage total

# Specific date range
subtrack usage total --from 2026-01-01 --to 2026-06-30

# Yearly total with provider breakdown
subtrack usage total --period yearly

# JSON output
subtrack usage total --json
```

## `search`

Searches subscriptions by name, notes, or tags. Useful for quickly finding subscriptions without scrolling through the full list.

| Option | Description |
|--------|-------------|
| `[query]` | Search query (required to return results — without it the command does nothing) |
| `--names` | Search in names only |
| `--notes` | Search in notes only |
| `--tags` | Search in tags only |
| `-j, --json` | Output as JSON |

### Examples

```bash
# Search by keyword
subtrack search netflix

# Search in notes only
subtrack search "family plan" --notes

# JSON output
subtrack search aws --json
```

## `trial`

Track free trial periods and set reminders before they expire.

### `trial add`

| Option | Description |
|--------|-------------|
| `--name <name>` | Trial name |
| `--expires-at <YYYY-MM-DD>` | Expiration date |
| `--price <price>` | Price after trial ends |
| `--currency <C>` | Currency |
| `--cycle <cycle>` | Billing cycle |
| `--notes <text>` | Notes |

### `trial list`

Lists all tracked trial periods.

### `trial expiring [days]`

Shows trials expiring within the given number of days (default: 7).

### `trial delete [ids...]`

Deletes trial entries (one or more IDs; batch deletion with multiple IDs).

```bash
# Add a trial
subtrack trial add \
  --name "Spotify Premium" \
  --expires-at 2026-08-01 \
  --price 980 \
  --currency JPY \
  --cycle monthly

# List all trials
subtrack trial list

# Trials expiring within 7 days
subtrack trial expiring

# Trials expiring within 30 days
subtrack trial expiring 30

# Delete trials by ID
subtrack trial delete 1 2
```

## `bulk`

Perform bulk operations on multiple subscriptions at once, with optional filtering by tag, status, or name pattern.

### `bulk status`

| Option | Description |
|--------|-------------|
| `--set <status>` | **Required.** Target status: `active`, `paused`, `cancelled` |
| `--tag <name>` | Only affect subscriptions with this tag |
| `--status <status>` | Only affect subscriptions with this current status |
| `--name <pattern>` | Only affect subscriptions whose name contains this |
| `-f, --force` | Skip confirmation prompt |

### `bulk delete`

| Option | Description |
|--------|-------------|
| `--tag <name>` | Only affect subscriptions with this tag |
| `--status <status>` | Only affect subscriptions with this status |
| `--name <pattern>` | Only affect subscriptions whose name contains this |
| `-f, --force` | Skip confirmation prompt |

### `bulk tag add`

| Option | Description |
|--------|-------------|
| `--add <tag>` | **Required.** Tag to add |
| `--tag <name>` | Filter by existing tag |
| `--status <status>` | Filter by status |
| `--name <pattern>` | Filter by name pattern |

### `bulk tag remove`

| Option | Description |
|--------|-------------|
| `--remove <tag>` | **Required.** Tag to remove |
| `--tag <name>` | Filter by existing tag |
| `--status <status>` | Filter by status |
| `--name <pattern>` | Filter by name pattern |

```bash
# Pause all subscriptions tagged "unused"
subtrack bulk status --set paused --tag unused

# Bulk delete cancelled subscriptions
subtrack bulk delete --status cancelled --force

# Add "essential" tag to all active subscriptions
subtrack bulk tag add --add essential --status active

# Remove "trial" tag from subscriptions expiring soon
subtrack bulk tag remove --remove trial --name "Free"
```

## `forecast`

Projects subscription spending over a given number of months. Supports what-if scenarios (cancelling or adding hypothetical subscriptions) and currency conversion.

| Option | Description |
|--------|-------------|
| `--months <n>` | Number of months to forecast (default: 12) |
| `-c, --currency <C>` | Convert all prices to target currency |
| `--cancel <names>` | Comma-separated subscription names to exclude |
| `--addName <name>` | Hypothetical subscription name to add |
| `--addPrice <price>` | Hypothetical subscription price |
| `--addCurrency <C>` | Hypothetical subscription currency |
| `--addCycle <cycle>` | Hypothetical subscription cycle |
| `-j, --json` | Output as JSON |

```bash
# Basic 12-month forecast
subtrack forecast

# 6-month forecast in JPY
subtrack forecast --months 6 --currency JPY

# What if I cancel Netflix?
subtrack forecast --cancel Netflix

# What if I add a new service?
subtrack forecast --addName "New Service" --addPrice 1500 --addCurrency JPY --addCycle monthly
```

## `history`

View price change history for subscriptions. Supports filtering by ID, showing all changes, or viewing recent activity.

| Option | Description |
|--------|-------------|
| `[id]` | Subscription ID to view history for |
| `--all` | Show all price changes across all subscriptions |
| `--days <n>` | Filter to recent N days (use with `--all`) |
| `-j, --json` | Output as JSON |

```bash
# View history for a specific subscription
subtrack history 3

# View all price changes
subtrack history --all

# Recent changes in the last 30 days
subtrack history --all --days 30

# JSON output
subtrack history --all --json
```

## `timeline`

Shows a monthly spending timeline with an ASCII bar chart. Useful for visualizing spending trends over time.

| Option | Description |
|--------|-------------|
| `--months <n>` | Number of months to display (default: 12) |
| `-c, --categories` | Show breakdown by category (first tag) |
| `--currency <C>` | Convert all prices to target currency |
| `-j, --json` | Output as JSON |

Without `--currency`, each currency gets its own chart and is never summed with another currency. With `--currency`, rows that cannot be converted are excluded and a warning is printed.

```bash
# 12-month spending timeline
subtrack timeline

# 6-month timeline with category breakdown
subtrack timeline --months 6 --categories

# Convert everything to USD
subtrack timeline --currency USD

# JSON output
subtrack timeline --json
```

## `optimize`

Analyzes your subscriptions for cost optimization opportunities. Detects potential savings from cycle changes, duplicate subscriptions, inactive subscriptions, and cancelled subscription waste.

| Option | Description |
|--------|-------------|
| `-j, --json` | Output as JSON |
| `--min-savings <amount>` | Minimum yearly savings to show (default: 0) |
| `-c, --currency <C>` | Convert all prices to target currency |
| `--discount-rate <percent>` | Assumed yearly discount rate for annual plans (default: 15) |
| `--exclude <names>` | Comma-separated subscription names to exclude from analysis |

Without `--currency`, savings are reported per currency and never summed across currencies. `--min-savings` requires `--currency` when subscriptions use more than one currency.

```bash
# Show all optimization suggestions
subtrack optimize

# Only show suggestions saving at least $100/year
subtrack optimize --min-savings 100

# JSON output
subtrack optimize --json

# Show savings in JPY, assuming 10% annual discount
subtrack optimize --currency JPY --discount-rate 10

# Skip subscriptions you can't cancel
subtrack optimize --exclude "GitHub Copilot,Adobe CC"
```

## `compare`

Compares subscription spending between the current period and the previous period. Useful for understanding how your costs have changed over time.

| Argument | Description |
|----------|-------------|
| `[period]` | Period: `monthly` (default), `quarterly`, `yearly` |

| Option | Description |
|--------|-------------|
| `-c, --currency <C>` | Convert all prices to target currency |
| `-a, --api` | Include LLM API usage costs |

```bash
# Compare current month vs previous month
subtrack compare

# Compare current quarter vs previous quarter
subtrack compare quarterly

# With currency conversion and API costs
subtrack compare monthly --currency JPY --api
```

## `calendar`

Displays a monthly calendar with billing days marked. Shows which subscriptions bill on which day.

| Option | Description |
|--------|-------------|
| `--month <n>` | Month (1–12, default: current) |
| `--year <yyyy>` | Year (default: current) |
| `-c, --currency <C>` | Convert all prices to target currency |
| `-j, --json` | Output as JSON |

```bash
# Show current month calendar
subtrack calendar

# Show a specific month
subtrack calendar --month 12 --year 2026

# Convert amounts to JPY
subtrack calendar --currency JPY

# JSON output
subtrack calendar --json
```

## `notify`

Sends a notification for upcoming bills (OS desktop by default). Supports dry-run mode to preview without notifying, and JSON output for scripting.

| Option | Description |
|--------|-------------|
| `--days <n>` | Number of days to look ahead (default: config `notifyDays` or 7) |
| `-c, --channel <name>` | Channel: `os`, `slack`, `webhook` (default: config `notifyChannels` or `os`) |
| `--dry-run` | Show upcoming bills without sending notification |
| `-j, --json` | Output as JSON |

```bash
# Send notification for bills due in 7 days
subtrack notify

# Preview without sending
subtrack notify --dry-run

# Next 30 days
subtrack notify --days 30

# Send to Slack instead of the desktop
subtrack notify --channel slack

# JSON output
subtrack notify --json
```

## `profile`

Save and switch between subscription filter profiles. Each profile stores a set of filters (tags, status, payment method) that can be activated to temporarily filter all views.

### `profile save <name>`

| Option | Description |
|--------|-------------|
| `--tag <name>` | Filter by tag (repeatable or comma-separated) |
| `--status <status>` | Filter by status |
| `--payment-method <method>` | Filter by payment method |

### `profile switch <name>`

Activates a saved profile. All list, payment, and summary commands will only show matching subscriptions.

### `profile list`

Lists all saved profiles.

### `profile show [name]`

Shows the details of a saved profile (or the active one if no name given).

### `profile delete <name>`

Deletes a saved profile.

```bash
# Save a profile for work subscriptions
subtrack profile save work --tag work --status active

# Switch to the work profile
subtrack profile switch work

# List all profiles
subtrack profile list

# Show active profile details
subtrack profile show

# Delete a profile
subtrack profile delete work
```

## `audit`

View and manage the audit log. All mutating operations (add, edit, delete, clone, archive, import, etc.) are recorded in an audit trail.

### `audit list`

| Option | Description |
|--------|-------------|
| `--action <action>` | Filter by action type |
| `--limit <n>` | Max entries (default: 50) |
| `--from <YYYY-MM-DD>` | Start date |
| `--to <YYYY-MM-DD>` | End date |
| `-j, --json` | Output as JSON |

```bash
# Show last 50 audit entries
subtrack audit list

# Filter by action type
subtrack audit list --action subscription.delete

# Filter by date range
subtrack audit list --from 2026-01-01 --to 2026-06-30

# JSON output
subtrack audit list --json
```

### `audit prune`

Removes old audit log entries to save space.

| Option | Description |
|--------|-------------|
| `--days <n>` | Delete entries older than N days (default: 90) |
| `-f, --force` | Skip confirmation |
| `-j, --json` | Output as JSON |

```bash
# Prune entries older than 90 days
subtrack audit prune

# Prune entries older than 30 days
subtrack audit prune --days 30
```

## `maintenance`

Run database maintenance operations: integrity check and VACUUM.

| Option | Description |
|--------|-------------|
| `--vacuum` | Run VACUUM to reclaim disk space (not run by default) |
| `--check` | Run integrity check (default: true) |
| `-j, --json` | Output as JSON |

```bash
# Default: integrity check only
subtrack maintenance

# Run VACUUM to reclaim space
subtrack maintenance --vacuum

# JSON output
subtrack maintenance --json
```

## `cleanup`

One-command database cleanup: runs integrity check, VACUUM, prunes old audit entries and orphaned tags.

| Option | Description |
|--------|-------------|
| `--vacuum` | Run VACUUM (default: `true`) |
| `--audit-days <n>` | Prune audit entries older than N days (default: 90) |
| `-j, --json` | Output as JSON |

VACUUM runs by default and cannot be turned off — use `subtrack maintenance` if you only want an integrity check.

```bash
# Full cleanup with defaults
subtrack cleanup

# Keep 30 days of audit history
subtrack cleanup --audit-days 30

# JSON output
subtrack cleanup --json
```

## `stats`

Shows database statistics: subscription counts by status, tag count, trial count, usage entry count, database file size, and per-currency price ranges.

| Option | Description |
|--------|-------------|
| `-j, --json` | Output as JSON |

Price ranges are reported separately for each currency; `--json` exposes them as `priceRanges`.

```bash
subtrack stats

# JSON output
subtrack stats --json
```

## `currency`

Lists all 37 supported currencies with their display names.

| Option | Description |
|--------|-------------|
| `-j, --json` | Output as JSON |

```bash
subtrack currency

# JSON output — useful for scripting
subtrack currency --json
```

## `mcp`

Starts a Model Context Protocol (MCP) server over stdio, enabling AI assistants (Claude Desktop, Cursor, etc.) to interact with your subscription data programmatically.

```bash
subtrack mcp
```

The MCP server exposes 20 tools for subscription management. See the [MCP page](/mcp) for full details, tool reference, and integration examples.

## `budget`

Shows spending vs a configured budget and detects budget overruns. Supports a single monthly/yearly budget, or multiple named budgets from config.

| Option | Description |
|--------|-------------|
| `--check` | Exit with code 1 when over budget (for cron/scripts) |
| `--period <monthly\|yearly>` | Comparison period (default: `monthly`) |
| `-c, --currency <code>` | Convert all prices to target currency |
| `--name <name>` | Compare against a named budget from `config budgets` |
| `-j, --json` | Output as JSON |

Budgets are configured via `config set`:

```bash
# Monthly budget
subtrack config set monthlyBudget 5000
subtrack budget

# Yearly budget
subtrack config set yearlyBudget 60000
subtrack budget --period yearly --check

# Named budget (JSON array — amount, currency, optional period/categories)
subtrack config set budgets '[{"name":"streaming","amount":3000,"currency":"JPY","categories":["video","music"]}]'
subtrack budget --name streaming

# Convert everything for a fair comparison
subtrack budget --currency USD
```

Named budgets filter subscriptions by their tags when `categories` is set, and can use their own period (`monthly` or `yearly`). `--check` is useful in cron scripts: it sets the exit code to 1 when spending exceeds the budget.

## `dedupe`

Detects duplicate subscriptions by name similarity (Levenshtein distance on normalized names, default threshold 0.8). Pairs sharing the same vendor URL are boosted to a 0.9 score.

| Option | Description |
|--------|-------------|
| `--threshold <0-1>` | Similarity threshold (default: `0.8`) |
| `-j, --json` | Output as JSON |

```bash
subtrack dedupe
subtrack dedupe --threshold 0.7
subtrack dedupe --json

# Merge a duplicate into the keeper (prices, tags, history preserved)
subtrack dedupe merge <keepId> <removeId>
```

`dedupe merge` moves the removed subscription's price history and tags onto the kept one and deletes the duplicate. Cancelled subscriptions are excluded from detection.

## `cancel <id>`

Cancels a subscription. Without `--force`, the command prints the cancellation preview — the guided checklist steps (optional data export, alternative-service check, cancellation-date note, final confirmation) — and aborts without changes. With `--force` it skips the checklist, marks the subscription `cancelled` (not deleted), sets `contractEnd` to today when unset, and writes the audit log.

| Option | Description |
|--------|-------------|
| `-f, --force` | Skip the checklist and cancel immediately |
| `-j, --json` | Output subscription info as JSON (no changes made) |

```bash
# Preview the cancellation without changing anything
subtrack cancel 3

# Cancel immediately (cron / scripting)
subtrack cancel 3 --force

# Inspect the target subscription as JSON (no changes made)
subtrack cancel 3 --json
```

When the checklist's "note the cancellation date" step is confirmed, `Cancelled: <date>` is appended to the subscription notes. Use `subtrack delete <id>` to remove a cancelled subscription permanently.

## `report`

Shows a yearly subscription report: total spending, monthly bar chart (single currency), top subscriptions by yearly cost, subscriptions added/cancelled during the year, price changes, and budget comparison.

| Option | Description |
|--------|-------------|
| `--year <year>` | Target year (default: current year) |
| `-c, --currency <code>` | Convert all prices to target currency |
| `-j, --json` | Output as JSON |

```bash
subtrack report
subtrack report --year 2025
subtrack report --currency USD
subtrack report --json
```

The monthly chart requires a single currency — use `--currency` to convert. Without conversion, totals are reported per currency in `byCurrency` and the top-level JSON `total` is `null` rather than a meaningless cross-currency sum. Cancelled subscriptions are counted only until their contract end date. The report compares yearly spending against `yearlyBudget` when configured.

## `pause [ids...]`

Pauses one or more subscriptions by ID, setting their status to `paused`. Paused subscriptions are excluded from payment totals, analytics, and upcoming bills.

| Option | Description |
|--------|-------------|
| `-f, --force` | Skip the confirmation prompt |

```bash
subtrack pause 3
subtrack pause 3 5 --force
```

## `resume [ids...]`

Resumes one or more paused or cancelled subscriptions by ID, restoring their status to `active`. `resume` accepts paused or cancelled subscriptions.

| Option | Description |
|--------|-------------|
| `-f, --force` | Skip the confirmation prompt |

```bash
subtrack resume 3 --force
```

## `renew <id>`

Renews a subscription and records the change in its price history. Existing values are kept when an option is omitted.

| Option | Description |
|--------|-------------|
| `--price <n>` | New price |
| `--currency <code>` | New currency |
| `--cycle <cycle>` | New billing cycle |
| `--contractEnd <date>` | New contract end date (`YYYY-MM-DD`) |
| `--planTier <tier>` | New plan tier |
| `--autoRenewal` | Enable automatic renewal |

```bash
subtrack renew 3 --price 1980 --contractEnd 2027-08-31
```

## `review`

Displays upcoming bills, contract expirations, and expiring trials. Use the day-range options to customize the review window.

| Option | Description |
|--------|-------------|
| `--billDays <n>` | Days ahead for bills |
| `--contractDays <n>` | Days ahead for contract expirations |
| `--trialDays <n>` | Days ahead for trials |
| `-j, --json` | Output as JSON |

```bash
subtrack review
subtrack review --billDays 30 --contractDays 60 --json
```

## `yearly`

Shows annual spending for a year, optionally converted to one currency. A thin alias of `subtrack report`.

| Option | Description |
|--------|-------------|
| `--year <yyyy>` | Target year (default: current year) |
| `-c, --currency <C>` | Convert all prices to target currency |
| `-j, --json` | Output as JSON |

```bash
subtrack yearly
subtrack yearly --year 2026
subtrack yearly --year 2026 --currency JPY --json
```

## `check`

Checks subscription data integrity (duplicate IDs, orphaned tags, invalid prices/cycles, missing links). `--strict` exits with code 1 when problems are found.

| Option | Description |
|--------|-------------|
| `--strict` | Exit with code 1 when problems are found |
| `-j, --json` | Output as JSON |

```bash
subtrack check
subtrack check --strict
subtrack check --json
```

## `changes`

Shows audit-log changes to subscriptions. Filter by subscription ID or date range.

| Option | Description |
|--------|-------------|
| `--id <n>` | Filter by subscription ID |
| `--from <YYYY-MM-DD>` | Start date (inclusive) |
| `--to <YYYY-MM-DD>` | End date (inclusive) |
| `--limit <n>` | Max entries to show |
| `-j, --json` | Output as JSON |

```bash
subtrack changes --from 2026-01-01 --to 2026-12-31
subtrack changes --id 3 --json
```

## `receipt <file>`

Imports subscription candidates from a receipt file. Use `--dryRun` to inspect without writing, or `--review` to print the review listing.

| Argument | Description |
|----------|-------------|
| `<file>` | Receipt file path (required) |

| Option | Description |
|--------|-------------|
| `--dryRun` | Parse without writing candidates |
| `--review` | Print the review listing before writing |
| `-j, --json` | Output as JSON |

```bash
subtrack receipt receipt.txt --dryRun
subtrack receipt receipt.txt --review
```

## `template`

Manages reusable subscription templates. Templates are stored in `config.json` and can be listed, added, edited, deleted, or used to create a subscription.

| Subcommand | Description |
|------------|-------------|
| `list` | List all templates |
| `add <name>` | Create a new template |
| `edit <name>` | Update an existing template |
| `delete <name>` | Delete a template |
| `use <name>` | Create a subscription from a template |

### `template add <name>` / `template edit <name>`

| Option | Description |
|--------|-------------|
| `<name>` | Template name (required) |
| `--price <n>` | Price in smallest currency unit |
| `--currency <C>` | Currency code |
| `--cycle <cycle>` | Billing cycle |
| `--tags <tags>` | Comma-separated tags |
| `--billingDay <n>` | Billing day of month (1–31) |
| `--notes <text>` | Notes |
| `--paymentMethod <method>` | Payment method |
| `--vendorName <name>` | Vendor name |
| `--vendorUrl <url>` | Vendor URL |
| `--planTier <tier>` | Plan tier |
| `--autoRenewal` | Enable automatic renewal |

### `template use <name>`

| Option | Description |
|--------|-------------|
| `<name>` | Template name (required) |
| `--price <n>` | Override price |
| `--currency <C>` | Override currency |
| `--cycle <cycle>` | Override billing cycle |
| `--tags <tags>` | Override tags (comma-separated) |

```bash
subtrack template add spotify --price 980 --currency JPY --cycle monthly --tags music
subtrack template list
subtrack template use spotify
subtrack template use spotify --price 1080
subtrack template edit spotify --price 1080
subtrack template delete spotify
```

## `suggest`

Manages subscription suggestions — candidates detected by the scanner (for example AI tool usage that looks like a paid subscription) or imported from a receipt.

| Subcommand | Description |
|------------|-------------|
| `list` | List pending suggestions |
| `view [id]` | Show full details of a suggestion (id prompted on a terminal) |
| `add [id]` | Add a suggestion as a subscription (id prompted on a terminal) |
| `dismiss [id]` | Dismiss one suggestion (prompted on a terminal), or all with `--all` |

### `suggest list`

| Option | Description |
|--------|-------------|
| `--all` | Show all suggestions (including dismissed/added) |
| `-j, --json` | Output as JSON |

### `suggest dismiss [id]`

| Option | Description |
|--------|-------------|
| `[id]` | Suggestion ID (required, or use `--all`) |
| `--all` | Dismiss all pending suggestions |

```bash
# List pending suggestions
subtrack suggest list

# Inspect one suggestion
subtrack suggest view 3

# Accept a suggestion
subtrack suggest add 3

# Dismiss one, or all
subtrack suggest dismiss 3
subtrack suggest dismiss --all
```
