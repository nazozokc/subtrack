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
| CLI framework | `commander` |
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
├── subtrack/         # CLI tool (TypeScript/ESM)
│   ├── src/
│   │   ├── index.ts      # Entry point, commander setup
│   │   ├── commands.ts   # Command handlers
│   │   ├── db.ts         # SQLite database layer
│   │   ├── display.ts    # Table rendering, currency conversion
│   │   └── prompts.ts    # Input validation, prompt helpers
│   └── dist/             # Built output
├── docs/             # Documentation site (SvelteKit)
├── flake.nix         # Nix devShell
└── pnpm-workspace.yaml
```
