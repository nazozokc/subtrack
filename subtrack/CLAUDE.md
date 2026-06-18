# subtrack

CLI tool to manage subscription services from the terminal. Node.js + TypeScript.

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
- Test files co-located as `*.test.ts`
- Use `__setDb()` from `db.ts` to inject in-memory SQLite for tests
- Mock `consola` via `consola.mockTypes()`
- Mock `globalThis.fetch` for FX rate API

## Architecture (4 layers)

| Layer | File | Responsibility |
|---|---|---|
| Entry | `src/index.ts` | CLI definition (commander), command routing |
| Commands | `src/commands.ts` | Command handlers, workflow logic |
| Database | `src/db.ts` | SQLite CRUD, schema, persistence |
| Display | `src/display.ts` | Table rendering, FX rate conversion |
| Prompts | `src/prompts.ts` | Input validation, interactive prompts |

## Key Conventions

- **Local imports**: `.ts` extension (`import { x } from "./foo.ts"`)
- **Node built-ins**: `node:` prefix (`node:fs`, `node:path`, `node:os`)
- **Type imports**: `type` prefix (`import type { X } from "./foo.ts"`)
- **No semicolons** in imports/exports
- **Prices**: integers (smallest unit — JPY no decimal, USD cents)
- **Cycles**: weekly / bi-weekly / monthly / quarterly / semi-annual / yearly
- **DB**: `sql.js` with `PRAGMA foreign_keys = ON`, use transactions for multi-step writes

## Environment Variables

| Variable | Description |
|---|---|
| `SUBSC_CLI_DB_DIR` | Override database directory (default: `~/.config/subtrack`) |

## Commands

| Command | Description |
|---|---|
| `subtrack list` | List all subscriptions |
| `subtrack add` | Add a subscription |
| `subtrack delete` | Delete subscriptions (interactive) |
| `subtrack tags <taglist...>` | Filter by tags |
| `subtrack backup <destination>` | Backup database |
| `subtrack payment [period]` | Show payment totals |
| `subtrack export csv` | Export subscriptions as CSV |
