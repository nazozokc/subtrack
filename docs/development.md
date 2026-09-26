---
title: Development
description: How to set up, build, test, and contribute to subtrack.
---

## Repository setup

```bash
git clone https://github.com/nazozokc/subtrack.git
cd subtrack
pnpm install
```

## Nix devShell

If you use Nix, a devShell is available with all required tools:

```bash
nix develop
```

This provides `node`, `pnpm`, `typescript`, `typos`, and `nixfmt`.

## Available commands

| Command | Description |
|---------|-------------|
| `pnpm start` | Run subtrack in dev mode (tsx) |
| `pnpm build` | Build all packages |
| `pnpm test` | Test all packages |
| `pnpm lint:types` | Type check (tsc --noEmit) |
| `pnpm format` | Format Nix files (nixfmt) |

## Tech stack

| Category | Choice |
|----------|--------|
| Runtime | Node.js |
| Language | TypeScript (strict mode, ESM) |
| CLI framework | Self-contained (`src/cli/`) |
| Interactive prompts | Self-contained (`src/prompts/`, `node:readline`) |
| Terminal output | `@subtrack/lib` (`ansi`, `logger`, `table`) |
| Spreadsheets | Self-contained `@subtrack/lib/xlsx` (hand-written XLSX) |
| OS notifications | Native commands via `node:child_process` (`osascript` / `notify-send`) |
| Database | `node:sqlite` (`DatabaseSync`) |
| Exchange rates | [open.er-api.com](https://open.er-api.com) |
| Build tool | `tsdown` |
| Test framework | `vitest` |
| Package manager | `pnpm` |
| Documentation | VitePress (this site) |
| Encryption | AES-256-GCM via Node.js `node:crypto` |

## Contributing

See [CONTRIBUTING.md](https://github.com/nazozokc/subtrack/blob/main/CONTRIBUTING.md) for branch policy, AI agent guidelines, and PR requirements.

### Before opening a PR

```bash
pnpm build
pnpm test
```

Make sure both pass and CI is green.

## Project structure

```
subtrack/
├── lib/                        # Shared library (@subtrack/lib, source package)
│   └── src/
│       ├── ansi.ts             # ANSI color helpers (picocolors-compatible)
│       ├── logger.ts           # consola-compatible logger
│       ├── table.ts            # cli-table3-compatible table renderer
│       ├── xlsx.ts             # Hand-written XLSX generator
│       ├── json.ts             # Safe JSON parsing helper
│       ├── path.ts             # Safe path resolution helpers
│       ├── format.ts           # File size formatting, byte helpers
│       ├── date.ts             # Date formatting & billing cycle helpers
│       └── crypto.ts           # AES-256-GCM encryption helpers
├── apps/
│   └── subtrack/              # CLI tool (TypeScript/ESM)
│       ├── src/
│       │   ├── index.ts            # Entry point, CLI bootstrap
│       │   ├── launcher.ts         # Bare `subtrack` → interactive menu
│       │   ├── pre-command.ts      # Pre-command hooks (notification banner)
│       │   ├── cli/                # Self-contained CLI framework
│       │   │   ├── types.ts           # `define()` and command types
│       │   │   ├── parser.ts          # Tokenizer + argument resolution
│       │   │   ├── router.ts          # Command/subcommand resolution
│       │   │   ├── help.ts            # Usage/help rendering
│       │   │   └── index.ts           # `cli()` entry, banner, error handling
│       │   ├── menu/               # Interactive main menu (bare `subtrack`)
│       │   │   ├── index.ts           # Menu router
│       │   │   ├── header.ts          # Header/banner rendering
│       │   │   ├── views.ts           # Read-only views
│       │   │   ├── edits.ts           # Edit flows
│       │   │   ├── manage.ts          # Lifecycle actions
│       │   │   ├── data.ts            # Import/export/backup flows
│       │   │   ├── config.ts          # Config menu
│       │   │   └── shared.ts          # Shared menu helpers
│       │   ├── commands/           # Command definitions (`define` from `src/cli/types.ts`)
│       │   │   ├── index.ts            # Barrel, subCommands map
│       │   │   ├── lazy.ts             # Lazy-loaded command groups (startup perf)
│       │   │   ├── core.ts             # list, add, edit, delete, cancel, clone,
│       │   │   │                       # archive, unarchive, search
│       │   │   ├── tag.ts              # tags, tag subcommands (list/rename/delete/prune/merge)
│       │   │   ├── trial.ts            # trial subcommands (add/list/expiring/delete)
│       │   │   ├── bulk.ts             # bulk subcommands (status/delete/tag)
│       │   │   ├── io.ts               # export, import
│       │   │   ├── backup.ts           # backup, restore
│       │   │   ├── config.ts           # config subcommands (list/get/set/reset)
│       │   │   ├── usage.ts            # usage subcommands (add/list/edit/delete/import/refresh/total)
│       │   │   ├── report.ts           # summary, payment, upcoming, analytics, compare,
│       │   │   │                       # calendar, forecast, history, notify, timeline,
│       │   │   │                       # optimize, stats, budget, report
│       │   │   ├── features.ts         # pause, resume, renew, review, yearly, check,
│       │   │   │                       # changes, receipt, template
│       │   │   ├── suggest.ts          # suggest subcommands (list/view/add/dismiss)
│       │   │   └── misc.ts             # mcp, profile, audit, maintenance, cleanup,
│       │   │                           # currency, dedupe
│       │   ├── subscription/      # Subscription command handlers
│       │   │   ├── core.ts            # handleList, handleDelete, handleClone, handleArchive,
│       │   │   │                       # handleUnarchive, handleTags
│       │   │   ├── add.ts             # handleAdd
│       │   │   ├── edit.ts            # handleEdit
│       │   │   └── delete.ts          # handleDelete
│       │   ├── db/                # Database layer (SQLite CRUD via node:sqlite)
│       │   │   ├── connection.ts      # DB connection, save, restore, backup helpers
│       │   │   ├── schema.ts          # Table creation & migrations
│       │   │   ├── integrity.ts       # Integrity checks
│       │   │   ├── subscriptions.ts   # Subscription CRUD
│       │   │   ├── tags.ts            # Tag CRUD
│       │   │   ├── usage.ts           # LLM usage CRUD
│       │   │   ├── trials.ts          # Trial CRUD
│       │   │   ├── suggestions.ts     # Suggestion CRUD
│       │   │   ├── price-history.ts   # Price change history
│       │   │   └── audit.ts           # Audit log CRUD
│       │   ├── domain/            # Framework-free business logic
│       │   │   └── billing.ts         # Billing cycle / next-billing-date math
│       │   ├── application/       # Ports & repository interfaces
│       │   │   ├── index.ts
│       │   │   ├── ports.ts
│       │   │   └── repositories.ts
│       │   ├── presentation/      # Output formatting adapters
│       │   │   └── output.ts
│       │   ├── prompts/           # Interactive prompt primitives
│       │   │   ├── index.ts           # Barrel
│       │   │   ├── core.ts            # Shared readline helpers
│       │   │   ├── input.ts           # Text input + validation
│       │   │   ├── select.ts          # Single/multi select
│       │   │   ├── confirm.ts         # Yes/no confirmation
│       │   │   ├── choices.ts         # Choice lists
│       │   │   ├── keys.ts            # Keypress handling
│       │   │   ├── search.ts          # Incremental search prompt
│       │   │   ├── renderer.ts        # Prompt rendering
│       │   │   └── ansi.ts            # Prompt-local ANSI helpers
│       │   ├── notifications/     # Notification banner
│       │   │   └── banner.ts
│       │   ├── suggest/           # Suggestion detection
│       │   │   ├── suggest.ts
│       │   │   ├── matcher.ts
│       │   │   ├── interactor.ts
│       │   │   ├── types.ts
│       │   │   └── parser/
│       │   ├── mcp/               # MCP server implementation
│       │   │   ├── index.ts           # Barrel
│       │   │   ├── server.ts          # MCP server setup & transport
│       │   │   ├── tools.ts           # Tool definitions & schemas
│       │   │   ├── handlers.ts        # Tool call handlers
│       │   │   ├── security.ts        # Input validation & sanitization
│       │   │   └── types.ts           # MCP type definitions
│       │   ├── display.ts         # Table rendering, price formatting
│       │   ├── display-constants.ts # Table styling constants
│       │   ├── prompts.ts         # Validation helpers re-exported from `src/prompts/`
│       │   ├── types.ts           # TypeScript type definitions
│       │   ├── payment.ts         # Payment totals & summary statistics
│       │   ├── upcoming.ts        # Upcoming bills calculator
│       │   ├── analytics.ts       # Subscription analytics & budget tracking
│       │   ├── budget.ts          # Budget vs actual comparison
│       │   ├── compare.ts         # Period-over-period spending comparison
│       │   ├── compare-totals.ts  # Shared period comparison totals
│       │   ├── forecast.ts        # Spending forecast with what-if scenarios
│       │   ├── timeline.ts        # Monthly spending timeline & bar chart
│       │   ├── optimize.ts        # Cost optimization suggestions
│       │   ├── history.ts         # Price change history view
│       │   ├── calendar.ts        # Monthly calendar with billing days
│       │   ├── notify.ts          # Desktop notification for upcoming bills
│       │   ├── profile.ts         # Filter profile management
│       │   ├── tag.ts             # Tag management handlers
│       │   ├── search.ts          # Subscription search
│       │   ├── trial.ts           # Trial management handlers
│       │   ├── bulk.ts            # Bulk operation handlers
│       │   ├── config.ts          # Configuration management (JSON file)
│       │   ├── export.ts          # CSV / JSON / Markdown / Excel / ICS export formatters
│       │   ├── import-csv.ts      # CSV parser & import handler
│       │   ├── backup.ts          # Backup & restore handlers
│       │   ├── fx.ts              # FX rate API & price conversion
│       │   ├── pricing.ts         # LiteLLM pricing cache & cost calculation
│       │   ├── price.ts           # Price formatting helpers
│       │   ├── report.ts          # Yearly report
│       │   ├── review.ts          # Upcoming bills / contracts / trials review
│       │   ├── yearly.ts          # Annual spending view
│       │   ├── cancel.ts          # Guided cancellation
│       │   ├── dedupe.ts          # Duplicate detection & merge
│       │   ├── receipt.ts         # Receipt parsing
│       │   ├── templates.ts       # Subscription templates
│       │   ├── features.ts        # Feature command handlers
│       │   ├── subscription-actions.ts # Pause / resume / renew
│       │   ├── validation.ts      # Shared input validation
│       │   ├── error.ts           # Error helpers
│       │   ├── diagnostics.ts     # Diagnostic reporting
│       │   ├── startup-profile.ts # Optional startup profiling
│       │   ├── usage-add.ts       # LLM usage add (interactive & flags)
│       │   ├── usage-edit.ts      # LLM usage field updates (flags)
│       │   ├── usage-import.ts    # LLM usage import from JSONL/JSON logs
│       │   ├── usage-refresh.ts   # Auto-scanner for AI tool usage data
│       │   ├── usage-total.ts     # Aggregated usage cost/token summary
│       │   ├── usage.ts           # LLM API usage list & delete
│       │   ├── scanner.ts         # Scanner framework for AI tool log parsing
│       │   ├── scanner-support.ts # Shared scanner helpers
│       │   ├── scanner-types.ts   # Scanner type definitions
│       │   ├── claude-scanner.ts  # Claude Code log scanner
│       │   ├── codex-scanner.ts   # Codex CLI log scanner
│       │   ├── copilot-scanner.ts # GitHub Copilot scanner
│       │   ├── cursor-scanner.ts  # Cursor editor scanner
│       │   ├── opencode-scanner.ts # OpenCode DB scanner
│       │   ├── windsurf-scanner.ts # Windsurf editor scanner
│       │   ├── audit.ts           # Audit log command handler
│       │   ├── audit-log.ts       # Audit log writer
│       │   ├── maintenance.ts     # Database maintenance (VACUUM, integrity check)
│       │   ├── cleanup.ts         # One-command database cleanup
│       │   ├── stats.ts           # Database statistics
│       │   ├── currency.ts        # List supported currencies
│       │   ├── mcp.ts             # MCP server entry (lazy-loads mcp/)
│       │   └── __tests__/         # Test files (vitest)
│       └── dist/                  # Built output (dist/index.mjs)
├── docs/                      # Documentation site (VitePress)
├── images/                    # Logo & branding assets
├── flake.nix                  # Nix devShell
└── pnpm-workspace.yaml
```
