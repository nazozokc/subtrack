---
title: Commands
description: Full reference for all subtrack CLI commands.
---

subtrack provides six commands. Most support both interactive and non-interactive modes.

## `list`

Lists all subscriptions in a formatted table. Subscriptions are grouped by currency by default, with a subtotal row per group.

| Option | Description |
|--------|-------------|
| `-c, --currency <C>` | Convert all prices to the given currency using live exchange rates |

### Examples

```bash
# List all subscriptions (grouped by currency)
subtrack list

# Convert all prices to JPY
subtrack list --currency JPY
```

When `--currency` is used, all prices are converted to the target currency (fetched from [open.er-api.com](https://open.er-api.com)) and displayed as a single group with a grand total.

## `add`

Adds a new subscription. Without flags, prompts for all fields interactively. Providing all flags skips prompts entirely (useful for scripts).

| Option | Description |
|--------|-------------|
| `--name <name>` | Subscription name (max 100 characters) |
| `--price <price>` | Payment amount — integer, non-negative, max 99,999,999 |
| `--currency <C>` | Currency code. Supported: JPY, USD, EUR, GBP, AUD, CAD, KRW, CNY, SGD, HKD |
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

## `delete`

Shows an interactive checkbox list of all subscriptions. Select one or more to delete. Confirmation is required before deletion.

<div class="callout warning">
  <strong>⚠ Note:</strong> The <code>delete</code> command is always interactive. There is no non-interactive mode.
</div>

### Example

```bash
subtrack delete
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
