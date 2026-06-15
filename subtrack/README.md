<p align="center">
  <img src="images/subtrack-cli-logo.jpg" alt="subtrack" width="400"/>
</p>

# subtrack

A CLI tool to manage your subscription services from the terminal.

**subtrack** lets you add, list, filter, and delete subscriptions with an interactive CLI. Data is stored locally in SQLite.

## Features

- **List** all subscriptions in a formatted table — auto-width, Unicode box-drawing, currency-grouped
- **Currency conversion** via `list --currency <CUR>` — automatically fetches live exchange rates
- **Add** subscriptions interactively, or fully non-interactive via flags
- **Delete** multiple subscriptions at once with a checkbox selector
- **Filter by tags** — AND-based filtering with multiple tags
- **Tag autocomplete hints** — shows existing tags during input
- **10 currencies** supported: JPY, USD, EUR, GBP, AUD, CAD, KRW, CNY, SGD, HKD
- **6 billing cycles**: weekly, bi-weekly, monthly, quarterly, semi-annual, yearly
- **SQLite** storage — portable, zero-config, lives in `~/.config/subtrack/subtrack.db`
- **Input validation** — name length, price bounds, tag limits

## Installation

```bash
# from npm
npm install -g subtrack

# or with pnpm
pnpm add -g subtrack

# or with bun
bun add -g subtrack

# or from source
pnpm install
pnpm build
pnpm link --global
```

## Usage

```bash
# List all subscriptions (grouped by currency)
subtrack list

# List with currency conversion
subtrack list --currency JPY

# Add a subscription (interactive)
subtrack add

# Add a subscription non-interactively
subtrack add --name Spotify --price 980 --currency JPY --cycle monthly --tags music

# Delete subscriptions (interactive checkbox)
subtrack delete

# Filter subscriptions by tags (AND)
subtrack tags music video
```

## npx usage
```bash
# List all subscriptions (grouped by currency)
npx subtrack list

# List with currency conversion
npx subtrack list --currency JPY

# Add a subscription (interactive)
npx subtrack add

# Add a subscription non-interactively
npx subtrack add --name Spotify --price 980 --currency JPY --cycle monthly --tags music

# Delete subscriptions (interactive checkbox)
npx subtrack delete

# Filter subscriptions by tags (AND)
npx subtrack tags music video
```


### Commands

#### `list`

Lists all subscriptions in a formatted table. Subscriptions are grouped by
currency by default. Each group shows a subtotal row.

| Option              | Description                                       |
| ------------------- | ------------------------------------------------- |
| `-c, --currency <C>` | Convert all prices to the given currency using live exchange rates |

When `--currency` is used, all prices are converted to the target currency
(fetched from [open.er-api.com](https://open.er-api.com)) and displayed as a
single group with a total.

#### `add`

Adds a new subscription. Without flags, prompts for all fields interactively.

| Option                | Description                          |
| --------------------- | ------------------------------------ |
| `--name <name>`       | Subscription name                    |
| `--price <price>`     | Payment amount (integer)             |
| `--currency <currency>` | Currency code (e.g. JPY, USD)      |
| `--cycle <cycle>`      | Billing cycle (e.g. monthly, yearly) |
| `--tags <tags>`       | Comma-separated tags                 |

All flags are optional. Providing all flags skips prompts entirely (useful for
scripts). Partial flags still prompt for missing fields.

#### `delete`

Shows an interactive checkbox list of all subscriptions. Select one or more
to delete. Confirmation is required before deletion.

> **Note:** The `delete` command is always interactive — no non-interactive mode.

#### `tags <taglist...>`

Filters and displays subscriptions that have **all** specified tags (AND logic).

```bash
# Subscriptions tagged with both "music" and "video"
subtrack tags music video
```

### Non-interactive mode

All flags on `add` can be combined for fully automated usage:

```bash
subtrack add --name "Netflix" --price 1490 --currency JPY --cycle monthly --tags "video,entertainment"
```

When all required flags are provided, the confirmation prompt is skipped.
Validation errors still produce error messages.

## Data

Subscriptions are stored in:

```
~/.config/subtrack/subtrack.db
```

The database directory can be overridden with the `SUBSC_CLI_DB_DIR` environment
variable:

```bash
SUBSC_CLI_DB_DIR=/path/to/custom/dir subtrack list
```

The database uses three tables: `subscriptions`, `tags`, and
`subscription_tags` (many-to-many relationship). Deleting a subscription
automatically cleans up associated tag links via `ON DELETE CASCADE`.

## Development

```bash
# Install dependencies
pnpm install

# Run directly (TypeScript)
pnpm start          # runs tsx src/index.ts

# Build
pnpm build          # runs tsdown → dist/index.mjs

# Test
pnpm test           # vitest run
pnpm test:watch     # vitest

# Lint
pnpm lint:typos     # typos check
```

## Tech Stack

| Category        | Choice                        |
| --------------- | ----------------------------- |
| Runtime         | Node.js                       |
| Language        | TypeScript (strict mode, ESM) |
| CLI framework   | commander                     |
| Interactive UI  | @inquirer/prompts             |
| Logging         | consola                       |
| Database        | sql.js (SQLite via WASM)      |
| Exchange rates  | open.er-api.com               |
| Build           | tsdown                        |
| Test            | vitest                        |
| Package manager | pnpm                          |

## Environment Variables

| Variable             | Description                              |
| -------------------- | ---------------------------------------- |
| `SUBSC_CLI_DB_DIR`   | Override the database directory (default: `~/.config/subtrack`) |

## License

MIT
