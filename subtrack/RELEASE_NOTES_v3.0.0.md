## v3.0.0 (2026-06-18)

### 💥 Breaking Changes

- **CLI framework migration**: Replaced `commander` with `gunshi` as the CLI engine. Argument and option parsing behavior may differ for programmatic usage ([`033ba4a`](https://github.com/nazozokc/subtrack/commit/033ba4a))
- **Currency type relaxation**: `Currency` type changed from a union of specific codes to `string`. Any currency code is now accepted ([`033ba4a`](https://github.com/nazozokc/subtrack/commit/033ba4a))
- **Cascading tag delete**: Foreign key on `subscription_tags.tag_id` now cascades on delete, automatically cleaning up orphaned associations ([`033ba4a`](https://github.com/nazozokc/subtrack/commit/033ba4a))
- **Export format validation**: `export` command now accepts `csv`, `json`, or `md` — previously only `csv` and `md` were supported ([`033ba4a`](https://github.com/nazozokc/subtrack/commit/033ba4a))

### ✨ Features

- **Edit command** (`subtrack edit`): Edit existing subscription fields interactively or via flags (`--name`, `--price`, `--currency`, `--cycle`, `--tags`) ([`033ba4a`](https://github.com/nazozokc/subtrack/commit/033ba4a))
- **Import command** (`subtrack import <file>`): Import subscriptions from a CSV file. Supports `--dry-run` for validation without writing ([`033ba4a`](https://github.com/nazozokc/subtrack/commit/033ba4a))
- **Summary command** (`subtrack summary`): Show subscription summary statistics (totals, counts, breakdowns) ([`033ba4a`](https://github.com/nazozokc/subtrack/commit/033ba4a))
- **Tag management** (`subtrack tag`): Subcommands for `list` (usage counts), `rename` (rename a tag), `delete` (delete a tag and its associations), and `prune` (remove orphaned tags) ([`033ba4a`](https://github.com/nazozokc/subtrack/commit/033ba4a))
- **List sorting** (`subtrack list --sort <field> --desc`): Sort subscriptions by `name`, `price`, `currency`, or `cycle`, with optional descending order ([`033ba4a`](https://github.com/nazozokc/subtrack/commit/033ba4a))
- **Export enhancements**: Added `json` export format and `--tags` filter option to export filtered subscription data ([`033ba4a`](https://github.com/nazozokc/subtrack/commit/033ba4a))
- **Colored output**: Integrated `picocolors` for colored table headers and totals in the terminal ([`033ba4a`](https://github.com/nazozokc/subtrack/commit/033ba4a))
- **Tag query optimization**: Replaced N+1 tag queries with a single batched query for all subscriptions ([`033ba4a`](https://github.com/nazozokc/subtrack/commit/033ba4a))

### 📦 Full Changelog

**Full Changelog**: https://github.com/nazozokc/subtrack/compare/2.3.0...3.0.0
