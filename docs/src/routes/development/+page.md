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
| `pnpm start` | Run subtrack directly from TypeScript source via `tsx` |
| `pnpm build` | Build the CLI bundle to `dist/index.mjs` via `tsdown` |
| `pnpm test` | Run tests with `vitest` |
| `pnpm test:watch` | Run tests in watch mode |
| `pnpm lint:typos` | Check for spelling errors with `typos` |

## Tech stack

| Category | Choice |
|----------|--------|
| Runtime | Node.js |
| Language | TypeScript (strict mode, ESM) |
| CLI framework | `gunshi` |
| Interactive prompts | `@inquirer/prompts` |
| Terminal output | `consola`, `picocolors`, `cli-table3` |
| Database | `sql.js` (SQLite via WASM) |
| Exchange rates | [open.er-api.com](https://open.er-api.com) |
| Build tool | `tsdown` |
| Test framework | `vitest` |
| Package manager | `pnpm` |
| Documentation | SvelteKit (this site) |

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
├── subtrack/              # CLI tool (TypeScript/ESM)
│   ├── src/
│   │   ├── index.ts           # Entry point, CLI definitions (gunshi)
│   │   ├── commands.ts        # Re-exports & thin CLI wrappers
│   │   ├── subscription.ts    # Add/edit/delete/list/tags handlers
│   │   ├── backup.ts          # Backup & restore handlers
│   │   ├── tag.ts             # Tag management handlers
│   │   ├── upcoming.ts        # Upcoming bills calculator
│   │   ├── analytics.ts       # Subscription analytics & budget tracking
│   │   ├── config.ts          # Configuration management (JSON file)
│   │   ├── db.ts              # SQLite database layer (CRUD, schema, crypto)
│   │   ├── display.ts         # Table rendering, price formatting
│   │   ├── prompts.ts         # Input validation, prompt helpers
│   │   ├── types.ts           # TypeScript type definitions
│   │   ├── payment.ts         # Payment totals & summary statistics
│   │   ├── usage.ts           # LLM API usage list & delete
│   │   ├── usage-add.ts       # LLM usage add (interactive & flags)
│   │   ├── usage-import.ts    # LLM usage import from JSONL/JSON logs
│   │   ├── usage-refresh.ts   # Auto-scanner for AI tool usage data
│   │   ├── pricing.ts         # LiteLLM pricing cache & cost calculation
│   │   ├── export.ts          # CSV / JSON / Markdown export formatters
│   │   ├── import-csv.ts      # CSV parser & import handler
│   │   ├── fx.ts              # FX rate API & price conversion
│   │   ├── crypto.ts          # AES-256-GCM encryption helpers
│   │   ├── path-utils.ts      # Safe path resolution helpers
│   │   ├── date-utils.ts      # Date formatting utilities
│   │   ├── safe-json.ts       # Safe JSON parsing helper
│   │   ├── scanner.ts         # Scanner framework for AI tool log parsing
│   │   ├── scanner-types.ts   # Scanner type definitions
│   │   ├── claude-scanner.ts  # Claude Code log scanner
│   │   ├── codex-scanner.ts   # Codex CLI log scanner
│   │   ├── copilot-scanner.ts # GitHub Copilot scanner
│   │   ├── cursor-scanner.ts  # Cursor editor scanner
│   │   ├── opencode-scanner.ts # OpenCode DB scanner
│   │   ├── windsurf-scanner.ts # Windsurf editor scanner
│   │   └── __tests__/         # Test files (vitest)
│   └── dist/                  # Built output (dist/index.mjs)
├── docs/                  # Documentation site (SvelteKit)
├── images/                # Logo & branding assets
├── flake.nix              # Nix devShell
└── pnpm-workspace.yaml
```
