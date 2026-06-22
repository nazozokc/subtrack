---
title: FAQ
description: Frequently asked questions and troubleshooting for subtrack.
---

## I lost my database. Can I recover it?

Use `subtrack restore` to restore from a backup. If you have a backup file, you can also restore it manually:

```bash
cp ~/backups/subtrack_20260617_143000.db.gz ~/.config/subtrack/subtrack.db.gz
gunzip -k ~/.config/subtrack/subtrack.db.gz
```

Without a backup, the data cannot be recovered — the database is stored locally only.

**Recommendation:** Set up regular automated backups via cron or Task Scheduler. See the [Data & Storage](/data) page for details.

## Can I add support for more currencies?

The `Currency` type now accepts any ISO 4217 3-letter code, so all currencies supported by [open.er-api.com](https://open.er-api.com) work out of the box. The interactive prompt provides a curated list of 36 commonly used currencies. If you need a currency not in the list, use the `--currency` flag directly with any valid code.

## Does subtrack work offline?

Yes. Listing, adding, editing, deleting, importing, exporting, and filtering all work fully offline. The only feature that requires internet is `--currency` conversion, which fetches live exchange rates from [open.er-api.com](https://open.er-api.com).

When offline, `--currency` falls back to per-currency display without conversion.

## How do I restore from a backup?

Use the `restore` command for interactive or direct restore:

```bash
# Interactive: select from available backups
subtrack restore

# Direct restore from a specific file
subtrack restore ~/backups/subtrack_20260617_143000.db.gz
```

Alternatively, copy the backup file manually:

```bash
cp ~/backups/subtrack_20260617_143000.db.gz ~/.config/subtrack/subtrack.db.gz
gunzip -k ~/.config/subtrack/subtrack.db.gz
```

The `restore` command automatically backs up your current data before restoring. Ensure no subtrack processes are running during the restore — the database is flushed to disk after each command.

## Is my data sent anywhere?

**No.** All data is stored locally in a SQLite file on your machine. There are no accounts, no telemetry, and no cloud sync. The only external request subtrack makes is to [open.er-api.com](https://open.er-api.com) for currency exchange rates — and only when you use the `--currency` flag.

## Is the database encrypted?

Yes. subtrack automatically encrypts the SQLite database file on disk using **AES-256-GCM**. On first run, a random 256-bit key is generated and stored at `~/.config/subtrack/.key`. Alternatively, you can set `SUBSC_CLI_DB_PASSPHRASE` to derive the key from a passphrase.

> ⚠️ **If you lose the key file or forget the passphrase, the database cannot be recovered.** Always back up the `.key` file or keep your passphrase in a secure location (e.g. password manager).

## Can I encrypt backups?

Yes. Use `subtrack backup --encrypt` to create an encrypted backup. Encrypted backups use the `.db.enc` extension and can only be restored on a machine with the same encryption key or passphrase.

## Can I use subtrack in Docker or CI?

Yes. subtrack is a standard Node.js CLI tool and works in any environment with Node.js 22+. For Docker:

```dockerfile
FROM node:22-alpine
RUN npm install -g subtrack
CMD ["subtrack", "list"]
```

Use `SUBSC_CLI_DB_DIR` to control where the database is stored in containerized environments.

## How does LLM API cost tracking work?

The `subtrack usage` command uses [LiteLLM](https://github.com/BerriAI/litellm) pricing data to automatically calculate costs based on model name and token counts. Pricing is cached locally for 24 hours and can be refreshed with `subtrack usage refresh`. If a model is not found in the cache, it falls back to the LiteLLM Model Catalog API, then prompts for manual cost input.

API costs are stored in USD cents (as a real number, allowing fractional cents for precise billing) and can be included in payment totals with `subtrack payment --api`.

## Where does the pricing data come from?

Model pricing is fetched from the [LiteLLM GitHub repository](https://github.com/BerriAI/litellm) (`model_prices_and_context_window.json`). The data is cached locally for 24 hours to avoid excessive network requests. You can force a refresh with `subtrack usage refresh`.

## How do I update subtrack?

```bash
npm update -g subtrack
```

Or, if installed via pnpm:

```bash
pnpm update -g subtrack
```

Your database will be preserved — updates only affect the CLI code, not your data.
