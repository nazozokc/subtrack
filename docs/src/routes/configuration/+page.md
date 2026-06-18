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

### Supported currencies (36)

The interactive prompt provides a curated list of 36 currencies. The `--currency` flag accepts any valid ISO 4217 3-letter code supported by [open.er-api.com](https://open.er-api.com).

```
AED  ARS  AUD  BRL  CAD  CHF  CLP  CNY  COP  CZK
DKK  EGP  EUR  GBP  HKD  HUF  IDR  ILS  INR  JPY
KRW  MXN  MYR  NGN  NOK  NZD  PHP  PLN  SAR  SEK
SGD  THB  TRY  TWD  USD  VND  ZAR
```

### Supported billing cycles (6)

```
weekly  bi-weekly  monthly  quarterly  semi-annual  yearly
```
