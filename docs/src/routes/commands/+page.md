---
title: Commands
description: Full reference for all subtrack CLI commands.
---

subtrack provides the following commands. Most support both interactive and non-interactive modes.

## `list`

Lists all subscriptions in a formatted table. Subscriptions are grouped by currency by default, with a subtotal row per group.

| Option | Description |
|--------|-------------|
| `-c, --currency <C>` | Convert all prices to the given currency using live exchange rates |
| `--sort <field>` | Sort by field: `name`, `price`, `currency`, `cycle`, `id` (default) |
| `-d, --desc` | Sort in descending order (use with `--sort`) |

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
```

When `--currency` is used, all prices are converted to the target currency (fetched from [open.er-api.com](https://open.er-api.com)) and displayed as a single group with a grand total.

## `add`

Adds a new subscription. Without flags, prompts for all fields interactively. Providing all flags skips prompts entirely (useful for scripts).

| Option | Description |
|--------|-------------|
| `--name <name>` | Subscription name (max 100 characters) |
| `--price <price>` | Payment amount — integer, non-negative, max 99,999,999 |
| `--currency <C>` | Currency code (ISO 4217). Accepts any 3-letter code; interactive mode provides a curated list |
| `--cycle <cycle>` | Billing cycle. One of: weekly, bi-weekly, monthly, quarterly, semi-annual, yearly |
| `--tags <tags>` | Comma-separated tags (max 10 tags, each max 50 characters) |

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
```

Without flags, `edit` shows a multi-select of fields to change. Each selected field is prompted with the current value as default.

## `delete`

Shows an interactive checkbox list of all subscriptions. Select one or more to delete. Confirmation is required before deletion.

<div class="callout warning">
  <strong>⚠ Note:</strong> The <code>delete</code> command is always interactive. There is no non-interactive mode.
</div>

### Example

```bash
subtrack delete
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
```

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

### Examples

```bash
# Monthly total (default)
subtrack payment

# Yearly total
subtrack payment yearly

# Weekly total in JPY
subtrack payment weekly --currency JPY
```

When `--currency` is used, the total is displayed as a single amount in the target currency. Without it, totals are grouped by currency.

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

## `backup <destination>`

Creates a timestamped backup of the SQLite database in the specified directory. The backup filename follows the format `subtrack_YYYYMMDD_HHmmss.db`.

If the destination does not exist or is not a directory, the command exits with an error. The backup will not overwrite existing files (exclusive create).

### Examples

```bash
# Backup to current directory
subtrack backup .

# Backup to ~/backups
subtrack backup ~/backups
```
