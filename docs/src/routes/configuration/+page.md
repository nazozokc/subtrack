---
title: Configuration
description: Environment variables and configuration options for subtrack.
---

subtrack follows a **zero-configuration** philosophy. It works out of the box with sensible defaults. The only configurable option is the database directory.

## Environment variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBSC_CLI_DB_DIR` | Override the database directory | `~/.config/subtrack` |

### Example usage

```bash
# Use a custom directory for the database
export SUBSC_CLI_DB_DIR=~/project/subtrack-data
subtrack list

# Or set it per-command
SUBSC_CLI_DB_DIR=/tmp/test-db subtrack add \
  --name Spotify \
  --price 980 \
  --currency JPY \
  --cycle monthly
```

<div class="callout info">
  <strong>💡 Tip:</strong> Changing <code>SUBSC_CLI_DB_DIR</code> lets you maintain separate databases — useful for testing or multi-profile setups.
</div>

## No config file

subtrack does not use configuration files (`.subtrackrc`, `subtrack.json`, etc.). All settings are controlled via environment variables or CLI flags. This keeps the tool simple and predictable.

## Currency & cycle choices

### Supported currencies (10)

```
JPY  USD  EUR  GBP  AUD  CAD  KRW  CNY  SGD  HKD
```

### Supported billing cycles (6)

```
weekly  bi-weekly  monthly  quarterly  semi-annual  yearly
```
