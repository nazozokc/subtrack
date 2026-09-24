# subtrack

CLI tool to manage subscription services from the terminal. Node.js + TypeScript.

## Tech Stack

- **Runtime**: Node.js (>=22.5, not Bun or Deno)
- **Language**: TypeScript (strict mode, ESM, `verbatimModuleSyntax`)
- **Database**: `node:sqlite` (`DatabaseSync`, built-in SQLite)
- **CLI**: self-contained (`src/cli/` — `define`, tokenizer/parser, router, help renderer)
- **Prompts**: self-contained (`src/prompts/`, `node:readline`)
- **Logging**: `@subtrack/lib/logger` (self-contained, `consola`-compatible logger)
- **Tables**: `@subtrack/lib/table` (self-contained, `cli-table3`-compatible renderer)
- **Colors**: `@subtrack/lib/ansi` (self-contained ANSI helpers, `picocolors`-compatible)
- **Spreadsheets**: `@subtrack/lib/xlsx` (self-contained XLSX generator)
- **Notifications**: native OS commands via `node:child_process` (`osascript` / `notify-send`)
- **Build**: `tsdown`
- **Test**: `vitest`

## Package Management

- Use `pnpm add <package>`
- Use `pnpm remove <package>`
- Use `pnpm update`
- Use `pnpm run <script>` or `pnpm <script>`
- Use `pnpmx <package>` instead of `npx` or `bunx`

## Running

- Development: `pnpm start` (tsx src/index.ts)
- Build: `pnpm build` (tsdown)
- Output: `dist/index.mjs`

## Testing

- `pnpm test` (vitest)
- Test files under `src/__tests__/` as `*.test.ts`
- Use `__setDb()` from `db.ts` to inject in-memory SQLite for tests
- Mock `consola` via `consola.mockTypes()`
- Mock `globalThis.fetch` for FX rate API

## Architecture

| File | Responsibility |
|---|---|
| `src/index.ts` | CLI entry point, routing |
| `src/cli/` | Self-contained CLI framework (`define`, parser, router, help, `cli()`) |
| `src/menu/` | Interactive main menu (launched by bare `subtrack`) — entry in `index.ts`, category sub-menus in `views.ts` / `edits.ts` / `manage.ts` / `data.ts` / `config.ts` |
| `src/commands/` | Command definitions (`define()` from `src/cli/types.ts` + `.run()`) |
| `src/subscription/` | Core subscription handlers (list/add/edit/delete/clone/archive/tags) |
| `src/db.ts`, `src/db/` | SQLite CRUD, schema, persistence |
| `src/display.ts` | Table rendering, formatting |
| `src/prompts.ts` | Input validation, interactive prompts |
| `src/payment.ts` | Payment/summary calculations |
| `src/fx.ts` | Exchange rate fetching & conversion |
| `src/pre-command.ts` | Pre-command hooks (notification banner) |
| `src/usage.ts` | LLM API usage tracking |
| `src/export.ts` | CSV/JSON/MD export |
| `src/import-csv.ts` | CSV import |
| `src/pricing.ts` | Pricing/litellm integration |
| `src/types.ts` | Shared types (incl. `Cycle` re-exported from `@subtrack/lib/date`) |
| `../../lib/src/` | Shared utilities (`@subtrack/lib/ansi|logger|table|xlsx|json|path|format|date|crypto`) |

## Key Conventions

- **Local imports**: `.ts` extension (`import { x } from "./foo.ts"`)
- **Node built-ins**: `node:` prefix (`node:fs`, `node:path`, `node:os`)
- **Type imports**: `type` prefix (`import type { X } from "./foo.ts"`)
- **No semicolons** in imports/exports
- **Prices**: integers (smallest unit — JPY no decimal, USD cents)
- **DB**: `node:sqlite` — `db.prepare(sql).run(...params)` for writes, `prepare().all()/.get()` for reads, `PRAGMA foreign_keys = ON`, transactions for multi-step writes

## Environment Variables

| Variable | Description |
|---|---|
| `SUBSC_CLI_DB_DIR` | Override database directory (default: `~/.config/subtrack`) |

## Commands

| Command | Description |
|---|---|
| `subtrack list` | List all subscriptions |
| `subtrack add` | Add a subscription |
| `subtrack edit [id]` | Edit a subscription |
| `subtrack delete [ids...]` | Delete subscriptions |
| `subtrack tags <names...>` | Filter by tags (AND logic) |
| `subtrack tag list\|rename\|delete\|prune` | Manage tags |
| `subtrack export csv\|json\|md\|ics` | Export subscriptions |
| `subtrack import <file>` | Import from CSV |
| `subtrack summary` | Show subscription summary |
| `subtrack backup [destination]` | Backup database |
| `subtrack restore [file]` | Restore database |
| `subtrack payment [period]` | Show payment totals |
| `subtrack budget` | Show spending vs budget |
| `subtrack dedupe [merge]` | Detect/merge duplicate subscriptions |
| `subtrack cancel [id]` | Cancel with guided checklist |
| `subtrack report` | Yearly subscription report |
| `subtrack usage add\|list\|delete\|refresh` | Track LLM API usage |
