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

## Managing multi-currency subscriptions

If you have subscriptions in different currencies (e.g., JPY for local services and USD for international ones), subtrack handles this natively:

```bash
# See everything in your local currency
subtrack list --currency JPY

# Compare spending across currencies
subtrack list
```

Without `--currency`, subscriptions are grouped by their original currency with per-group subtotals.

## Regular backups

Set up a cron job (or Task Scheduler on Windows) for automatic backups:

```bash
# Example cron: daily backup at 3 AM
0 3 * * * subtrack backup ~/subtrack-backups
```

Backups are timestamped and will never overwrite previous files. See [Data & Storage](/data) for restore instructions.
