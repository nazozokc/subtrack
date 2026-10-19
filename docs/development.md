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

This provides `node`, `pnpm`, `typescript`, `typos`, `rustc`, `cargo`, `rustfmt`, `clippy`, and `nixfmt-rfc-style`.

## Available commands

| Command | Description |
|---------|-------------|
| `pnpm build` | Build all workspace packages |
| `pnpm test` | Test all workspace packages |
| `pnpm lint:security` | Security lint across the workspace |
| `pnpm format` | Format Nix files (`nix fmt`) |
| `cargo build --manifest-path rust/Cargo.toml` | Build the Rust CLI (debug) |
| `cargo build --release --manifest-path rust/Cargo.toml` | Build the optimized release binary |
| `cargo run --manifest-path rust/Cargo.toml` | Run the CLI from source |
| `cargo test --test golden --manifest-path rust/Cargo.toml` | Run the golden test suite (`cd rust && cargo test --test golden`, ~30 s) |
| `cargo clippy` / `cargo fmt` | Lint / format the Rust code |

## Tech stack

| Category | Choice |
|----------|--------|
| Language | Rust 2024 edition, workspace pinned to 1.98.0 (`rust-toolchain.toml`) |
| CLI framework | Self-contained (`src/cli.rs`, `src/parser.rs`, `src/router.rs`, `src/help.rs`) |
| Database | SQLite via `rusqlite` (bundled, no system libsqlite) |
| Dates | `chrono` |
| Encryption | AES-256-GCM (`aes-gcm`) + `scrypt` key derivation |
| Compression | `flate2` (zlib, statically linked — gzip output is byte-identical to the TypeScript reference) |
| Search/regex | `regex-lite` |
| Distribution | npm launcher (`packages/subtrack`) + 6 platform binary packages |
| Package manager | `pnpm` (workspace), `cargo` (Rust) |
| Documentation | VitePress (this site) |

## Golden fixtures

The test suite is a goldens harness: `rust/fixtures/` contains frozen reference outputs captured from the TypeScript implementation (tag `last-typescript`). Every case runs the Rust binary and compares stdout, stderr, and exit code byte-for-byte.

- `cd rust && cargo test --test golden` runs the full matrix (365 cases: seed, help, reads, mutations, errors, mcp, legacy).
- Populated databases (`fixtures/dbs/populated/`) are seeded from `config.json` + `subtrack.db` + `.key` per group.
- Re-capturing fixtures requires building the TypeScript implementation: `pnpm build`, then the capture scripts under `rust/fixtures/`.
- For deterministic date-sensitive output, fixtures set `SUBTRACK_TODAY` (a Rust-only environment variable that pins the "today" used by date logic).

## Release packaging

- Linux binaries are built as static musl targets (`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`) — they run on any glibc or musl distro, including Alpine.
- macOS and Windows x64 binaries are cross-compiled from arm64 runners using rustup target installation.
- The release profile is tuned for size/speed: `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `strip = "symbols"`.
- The `subtrack` npm package is a launcher (`packages/subtrack/src/launcher.mjs`) that resolves and spawns the binary from the matching optional dependency (`@subtrack/subtrack-<platform>`); `SUBTRACK_BINARY` overrides the resolved binary path (useful for local dev builds).
- Platform packages declare `os`/`cpu` and carry the prebuilt binary as `bin/subtrack`; their `prepack` runs `scripts/verify-native-package.mjs`.

## Contributing

See [CONTRIBUTING.md](https://github.com/nazozokc/subtrack/blob/main/CONTRIBUTING.md) for branch policy, AI agent guidelines, and PR requirements.

### Before opening a PR

```bash
cargo fmt --manifest-path rust/Cargo.toml
cargo clippy --manifest-path rust/Cargo.toml
cargo test --test golden --manifest-path rust/Cargo.toml
pnpm build
```

Make sure all checks pass and CI is green.

## Project structure

```
subtrack/
├── rust/                          # Rust implementation
│   ├── Cargo.toml                 # Workspace (crates: ["crates/subtrack"], edition 2024)
│   ├── crates/subtrack/           # The CLI crate
│   │   ├── src/
│   │   │   ├── main.rs            # Entry point
│   │   │   ├── cli.rs             # Self-contained CLI framework (define(), routing, header)
│   │   │   ├── parser.rs          # Tokenizer + argument resolution (strict: unknown flags error)
│   │   │   ├── router.rs          # Command/subcommand resolution
│   │   │   ├── help.rs            # Usage/help rendering
│   │   │   ├── commands.rs        # Command registry (50 commands)
│   │   │   ├── mcp.rs             # MCP server (20 tools)
│   │   │   ├── subs.rs            # add/edit/delete/cancel/pause/resume/renew/backup/restore…
│   │   │   ├── list.rs            # list (filters, sorting)
│   │   │   ├── search.rs          # search
│   │   │   ├── tags.rs            # tags & tag subcommands
│   │   │   ├── trial.rs, bulk.rs, dedupe.rs, templates.rs, suggest.rs
│   │   │   ├── io.rs, export.rs, receipt.rs      # export / import / receipts
│   │   │   ├── payment.rs, analytics.rs, summary.rs, forecast.rs, compare.rs
│   │   │   ├── budget.rs, optimize.rs, timeline.rs, calendar.rs, upcoming.rs,
│   │   │   ├── review.rs, report.rs, history.rs, changes.rs, notify.rs
│   │   │   ├── profile.rs         # Filter profiles
│   │   │   ├── usage.rs           # LLM usage tracking (add/list/import/refresh/total)
│   │   │   ├── audit.rs, maintenance.rs, cleanup.rs, stats.rs, check.rs, config.rs
│   │   │   ├── db.rs              # SQLite CRUD, schema, migrations
│   │   │   ├── currency.rs, fx.rs # Currency list & exchange rates
│   │   │   ├── backup.rs, crypto.rs # Backup & AES-256-GCM encryption
│   │   │   ├── display.rs, table.rs, price.rs, xlsx.rs, json.rs
│   │   │   ├── validation.rs, safe_path.rs
│   │   │   ├── date.rs            # Date helpers (SUBTRACK_TODAY override)
│   │   │   ├── ansi.rs, logger.rs # ANSI helpers & consola-compatible logger
│   │   │   └── …
│   │   └── tests/golden.rs        # Golden fixture harness
│   └── fixtures/                  # Frozen TS reference goldens (README.md explains re-capture)
├── packages/
│   ├── subtrack/                  # npm package: launcher + metadata
│   │   ├── src/launcher.mjs       # Spawns the platform binary (SUBTRACK_BINARY override)
│   │   └── package.json           # optionalDependencies → platform packages
│   ├── subtrack-linux-x64/        # Platform binary packages (os/cpu fields, bin/subtrack)
│   ├── subtrack-linux-arm64/
│   ├── subtrack-darwin-x64/
│   ├── subtrack-darwin-arm64/
│   ├── subtrack-win32-x64/
│   └── subtrack-win32-arm64/
├── docs/                          # Documentation site (VitePress)
├── scripts/                       # set-version.mjs, verify-native-package.mjs
├── rust-toolchain.toml            # Pinned Rust toolchain (1.98.0, clippy/rustfmt)
├── flake.nix                      # Nix devShell
├── package.json                   # pnpm build / test / lint:security / format / clean
└── pnpm-workspace.yaml
```
