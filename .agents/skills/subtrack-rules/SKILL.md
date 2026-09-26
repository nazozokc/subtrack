---
name: subtrack-rules
description: When working on the subtrack project, editing source code, running tests, or managing dependencies
---

# Subtrack Project Rules

## Project Overview

subtrack is a Node.js CLI tool for managing subscription services from the terminal. Written in TypeScript, it stores data in SQLite and provides interactive prompts for CRUD operations.

## Package Management

- Use `pnpm add <package>` instead of `npm install <package>` or `bun add <package>`
- Use `pnpm remove <package>` instead of `npm uninstall <package>` or `bun remove <package>`
- Use `pnpm update` instead of `npm update` or `bun update`
- Use `pnpm run <script>` or `pnpm <script>` instead of `npm run` or `bun run`
- Use `pnpmx <package>` instead of `npx` or `bunx`
- All packages go under `apps/subtrack/` directory (monorepo root has no deps)

## Build & Run

- Development execution: `tsx src/index.ts`
- Build: `tsdown` (configured as `pnpm build`)
- Type check: `pnpm lint:types` (`tsc --noEmit`)
- Don't use `bun` or `node --loader` for running TypeScript directly
- Build output is `apps/subtrack/dist/index.mjs`

## Testing

- Framework: `vitest` (`pnpm test` or `vitest run`)
- Coverage: `pnpm test:coverage` (`vitest run --coverage`, v8 provider)
- Test files live under `src/__tests__/` as `*.test.ts` (not co-located)
- Use `pnpm test:watch` for watch mode
- Use `__setDb()` from `db.ts` to inject an in-memory SQLite database for tests
- Mock `consola` via `consola.mockTypes()` for output assertions (see `display.test.ts`)
- Mock `globalThis.fetch` for exchange rate API tests

## Database

- Use `node:sqlite` (`DatabaseSync`) — **not** `better-sqlite3`, `sqlite3`, or `bun:sqlite`
- Import: `import { DatabaseSync } from "node:sqlite"` and `import type { DatabaseSync, SQLInputValue } from "node:sqlite"`
- Requires Node >= 22.5 (`engines` in package.json)
- Database file location: `$SUBSC_CLI_DB_DIR` env var or `~/.config/subtrack/subtrack.db`
- State is held in memory (`_db`) and persisted to disk via synchronous `saveDb()` (VACUUM INTO) on writes
- API mapping vs old sql.js style: `db.prepare(sql).run(...params)` returns `{ changes, lastInsertRowid }`; result-returning `exec` is replaced by `prepare().all()` / `.get()`; `db.export()` is replaced by `exportDbBytes()`
- Schema has 8 tables: `subscriptions`, `tags`, `subscription_tags`, `llm_usage`, `trials`, `price_history`, `suggestions`, `audit_log`
- Schema migrations are versioned via `PRAGMA user_version` (`SCHEMA_VERSION` in `db/schema.ts`). When adding tables/columns, add a new `migrateToV{N}` step and bump `SCHEMA_VERSION` — never edit an existing migration
- Always use transactions for multi-step writes (`BEGIN TRANSACTION` / `COMMIT` / `ROLLBACK`)
- Use `PRAGMA foreign_keys = ON` at connection time

## Architecture

The source code (`subtrack/src/`) follows a layered separation:

| Layer | File | Responsibility |
|---|---|---|
| Entry | `src/index.ts` | CLI bootstrap, routing (bare `subtrack` opens the interactive menu) |
| CLI | `src/cli/` | Self-contained CLI framework (`define`, tokenizer/parser, router, help renderer, `cli()`) |
| Commands | `src/commands/` | Command definitions (`define()` from `src/cli/types.ts` + `.run()`) |
| Handlers | `src/subscription/`, `src/menu/`, `src/payment.ts`, … | Command handlers, workflow logic, user interaction |
| Audit write | `src/audit-log.ts` | `logAudit` write-side helper (deliberately separate from `audit.ts` to keep the table renderer out of the shared config chunk) |
| Database | `src/db.ts`, `src/db/` | SQLite CRUD, schema, persistence, `__setDb()` for testing |
| Display | `src/display.ts` | Table rendering, formatting (table renderer via `@subtrack/lib/table`) |
| Prompts | `src/prompts.ts` | Input validation, interactive prompts, shared choices |
| FX | `src/fx.ts` | Exchange rate fetching & conversion (`fetchFxRates`, `convertPrice`, `convertSubsWithRates`, `tryConvert`) |
| Dates | `@subtrack/lib/date` | Date helpers (`today`, `formatDate`, `daysUntil`, period ranges, `Cycle`) |
| Path safety | `@subtrack/lib/path` | `resolveSafePath` / `resolveSafeOutputPath` (+ `safePath` / `safeOutputPath` shortcuts) |
| Shared utils | `lib/src/` | `@subtrack/lib/ansi|logger|table|xlsx|json|path|format|date|crypto` (source package, bundled by tsdown) |

Keep concerns separated. Don't put DB queries in display logic or prompt logic in command handlers.

## Import Style

- Use `node:` prefix for Node.js built-ins: `import { readFileSync } from "node:fs"`, `import path from "node:path"`, `import { homedir } from "node:os"`
- Use `.ts` extension in local imports: `import { handleList } from "./subscription/core.ts"`
- Prefer native `fetch` for HTTP requests
- Prefer native `WebSocket` for WebSocket connections (if needed)
- Use `type` prefix for type-only imports: `import type { SharedArgs } from "./db.ts"`

## Code Style

- TypeScript strict mode enabled (see `tsconfig.json`)
- `noUnusedLocals` / `noUnusedParameters`: enabled — unused imports/params are compile errors
- Test files (`*.test.ts`) excluded from `tsconfig.json` — type-checked separately by vitest
- ESM modules (`"type": "module"` in package.json)
- No semicolons in imports/exports
- Target: ESNext, module: Preserve, moduleResolution: bundler
- `verbatimModuleSyntax`: enforce correct import/value separation
- `noUncheckedIndexedAccess`: disabled (codebase not compatible)

## Dependencies

subtrack has **zero runtime dependencies** — everything is self-contained:

| Module | Usage |
|---|---|
| `src/cli/` | Self-contained CLI framework (`define`, tokenizer/parser, router, help renderer) — replaced gunshi |
| `src/prompts/` | Self-contained interactive prompts (`input`, `confirm`, `checkbox`, `select`, `search`) on `node:readline` — no inquirer |
| `@subtrack/lib/logger` | Self-contained `consola`-compatible logger (`consola.info/success/error/warn/fail`) |
| `@subtrack/lib/table` | Self-contained `cli-table3`-compatible table renderer (customizable chars, styles, column widths) |
| `@subtrack/lib/ansi` | Self-contained `picocolors`-compatible terminal colors |
| `node:sqlite` | Built-in SQLite (`DatabaseSync`, `SQLInputValue`) — no WASM, no native deps |
| `@types/node` | Node.js type definitions (dev) |

## Linting

- Spell check: `typos` (`pnpm lint:typos` or `typos`)
- No ESLint or Prettier configured (formatting via editor defaults)
- Nix formatting: `nix fmt` (`nixfmt-rfc-style`)

## Environment

- `SUBSC_CLI_DB_DIR`: override database directory (default: `~/.config/subtrack/`)
- Data file: `subtrack.db` (SQLite)
- No `.env` file or secrets management in this project

## Conventions

- Prices are stored in major currency units (e.g. `14.99` USD); `llm_usage.cost` is stored in USD cents
- Currencies: JPY/USD/EUR/GBP/AUD/CAD/KRW/CNY/SGD/HKD
- Cycles: weekly/bi-weekly/monthly/quarterly/semi-annual/yearly
- Tags are stored in a normalized many-to-many relation
- FX rates are fetched from `open.er-api.com` (USD base)
