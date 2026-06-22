---
title: Usage Guides
description: Practical usage examples and workflows for subtrack.
---

Practical workflows for managing your subscriptions with subtrack.

## First-time setup & adding subscriptions

After installation, start by adding your subscriptions. The interactive mode guides you through each field:

```bash
subtrack add
```

You'll be prompted for: name, price, currency, billing cycle, and tags. Once added, verify with:

```bash
subtrack list
```

### Example: Adding a Netflix subscription

```bash
subtrack add \
  --name "Netflix Premium" \
  --price 1980 \
  --currency JPY \
  --cycle monthly \
  --tags "video,entertainment"
```

## Editing subscriptions

Update existing subscriptions with `subtrack edit`. Run it interactively to pick a subscription and choose which fields to change:

```bash
subtrack edit
```

For non-interactive use, specify the subscription ID and the fields to update:

```bash
subtrack edit 3 --price 2500 --tags "video,entertainment,4k"
```

To see subscription IDs, run `subtrack list`.

## Importing from CSV

Bulk-import subscriptions from a CSV file:

```bash
subtrack import subscriptions.csv
```

The CSV must have the header `name,cycle,tags,price,currency`. Tags are separated by semicolons:

```csv
name,cycle,tags,price,currency
Netflix,monthly,video;entertainment,1980,JPY
GitHub Copilot,monthly,development,10,USD
```

Use `--dry-run` to validate without importing:

```bash
subtrack import subscriptions.csv --dry-run
```

## Tag-based organization

Tags are a powerful way to categorize subscriptions. Examples:

- **By category:** `music`, `video`, `cloud`, `productivity`
- **By priority:** `essential`, `nice-to-have`
- **By payment method:** `credit-card`, `paypal`
- **By usage:** `personal`, `work`, `family`

Filter by tags:

```bash
# Find all music-related subscriptions
subtrack tags music

# Find subscriptions used for both work and personal
subtrack tags work personal
```

Tags use AND logic — only subscriptions matching **all** specified tags are shown.

### Managing tags

View all tags and their usage count:

```bash
subtrack tag list
```

Rename a tag (merges if the new name already exists):

```bash
subtrack tag rename entertainment fun
```

Delete a tag and its associations:

```bash
subtrack tag delete fun
```

Clean up orphaned tags (tags no longer attached to any subscription):

```bash
subtrack tag prune
```

## Understanding your spending

Use `subtrack payment` to see your total spending across different periods:

```bash
# How much per month?
subtrack payment

# How much per year?
subtrack payment yearly

# Weekly cost in USD
subtrack payment weekly --currency USD
```

`payment` automatically converts all billing cycles to the target period. A yearly subscription will be divided into monthly cost, and a weekly subscription will be multiplied accordingly.

You can also include LLM API usage costs in the payment calculation:

```bash
# Monthly subscription + API costs
subtrack payment monthly --api

# API costs converted to JPY
subtrack payment monthly --api --currency JPY
```

### Summary statistics

Get a quick overview of all your subscriptions:

```bash
subtrack summary
```

This shows total count, the most expensive subscription, monthly spending broken down by currency, and monthly spending broken down by tag.

## Managing multi-currency subscriptions

If you have subscriptions in different currencies (e.g., JPY for local services and USD for international ones), subtrack handles this natively:

```bash
# See everything in your local currency
subtrack list --currency JPY

# Compare spending across currencies
subtrack list
```

Without `--currency`, subscriptions are grouped by their original currency with per-group subtotals.

## Exporting data

Export subscriptions for use in spreadsheets or other tools:

```bash
# CSV (for Excel / Google Sheets)
subtrack export csv

# JSON (for programmatic use)
subtrack export json

# Markdown (for documentation)
subtrack export md

# Filter by tags and convert currency
subtrack export csv --tags video --currency JPY
```

## Regular backups

Set up a cron job (or Task Scheduler on Windows) for automatic backups:

```bash
# Example cron: daily backup at 3 AM
0 3 * * * subtrack backup ~/subtrack-backups
```

Backups are timestamped and will never overwrite previous files. See [Data & Storage](/data) for restore instructions.

## Tracking LLM API costs

subtrack can track LLM API usage costs alongside subscription costs. This is useful if you use paid LLM APIs (OpenAI, Anthropic, etc.) and want to see total AI spending.

### Adding usage entries

```bash
# Interactive mode
subtrack usage add

# Non-interactive
subtrack usage add \
  --provider openai \
  --model gpt-4o \
  --inputTokens 5000 \
  --outputTokens 1500 \
  --date 2026-06-19 \
  --description "Code review"
```

Cost is automatically calculated using LiteLLM pricing data. If a model is not recognized, you'll be prompted to enter the cost manually.

### Listing and filtering

```bash
# All entries
subtrack usage list

# Filter by provider and date range
subtrack usage list --provider anthropic --from 2026-06-01
```

### Including API costs in payment totals

```bash
# See subscription + API costs together
subtrack payment monthly --api
```

API costs for the current period are fetched and added to the subscription totals. They are stored in USD cents and automatically converted if you use `--currency`.

### Refreshing pricing data

The LiteLLM pricing cache is automatically refreshed every 24 hours. To force a refresh:

```bash
subtrack usage refresh
```
