# subtrack — GitHub Copilot Instructions

## Project

A CLI tool to manage subscription services from the terminal. Node.js + TypeScript, SQLite via
sql.js (WASM).

## Tech Stack

- **Runtime**: Node.js (not Bun or Deno)
- **Language**: TypeScript (strict mode, ESM, `verbatimModuleSyntax`)
- **Database**: `sql.js` (SQLite via WASM)
- **CLI**: `commander`
- **Prompts**: `@inquirer/prompts`
- **Logging**: `consola`
- **Tables**: `cli-table3`
- **Build**: `tsdown`
- **Test**: `vitest`

## Architecture (4-layer separation)

| Layer       | File              | Responsibility                                |
|-------------|-------------------|---------------------------------------------|
| Entry       | `src/index.ts`    | CLI definition (commander), command routing |
| Commands    | `src/commands.ts` | Command handlers, workflow logic            |
| Database    | `src/db.ts`       | SQLite CRUD, schema, persistence            |
| Display     | `src/display.ts`  | Table rendering, FX rate conversion         |
| Prompts     | `src/prompts.ts`  | Input validation, interactive prompts       |

## Key Conventions

- **Local imports**: use `.ts` extension (`import { x } from "./foo.ts"`)
- **Node built-ins**: use `node:` prefix (`node:fs`, `node:path`)
- **Type imports**: use `type` prefix (`import type { X } from "./foo.ts"`)
- **No semicolons** in imports/exports
- **Prices**: stored as integers (smallest unit: JPY has no decimal, USD stored as cents)
- **Cycles**: weekly / bi-weekly / monthly / quarterly / semi-annual / yearly
- **DB transactions**: always use `BEGIN TRANSACTION` / `COMMIT` / `ROLLBACK`
- **PRAGMA**: `PRAGMA foreign_keys = ON` at connection time

## Environment Variables

- `SUBSC_CLI_DB_DIR`: override database directory (default: `~/.config/subtrack/`)
- Data file: `subtrack.db` (SQLite)

## Commands

| Command                            | Description                        |
|------------------------------------|------------------------------------|
| `subtrack list`                    | List all subscriptions             |
| `subtrack add`                     | Add a subscription                 |
| `subtrack delete`                  | Delete subscriptions (interactive) |
| `subtrack tags <taglist...>`       | Filter by tags                     |
| `subtrack backup <destination>`    | Backup database                    |
| `subtrack payment [period]`        | Show payment totals                |
| `subtrack export csv`              | Export subscriptions as CSV        |
| `subtrack export md`               | Export subscriptions as Markdown   |
