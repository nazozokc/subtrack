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
| `cargo test --manifest-path rust/Cargo.toml` | Run the Rust test suite (unit tests + CLI smoke suite) |
| `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings` | Lint the Rust code (CI gate, denies warnings) |
| `cargo fmt --manifest-path rust/Cargo.toml --all -- --check` | Verify Rust formatting (CI gate) |

## Tech stack

| Category | Choice |
|----------|--------|
| Language | Rust 2024 edition, workspace pinned to 1.98.0 (`rust-toolchain.toml`) |
| CLI framework | Self-contained (`src/cli.rs`, `src/parser/`, `src/router.rs`, `src/help.rs`) |
| Database | SQLite via `rusqlite` (bundled, no system libsqlite) |
| Dates | `chrono` |
| Encryption | AES-256-GCM (`aes-gcm`) + `scrypt` key derivation |
| Compression | `flate2` (zlib, statically linked — gzip output is byte-identical to the TypeScript reference) |
| Search/regex | `regex-lite` |
| Distribution | npm launcher (`packages/subtrack`) + 6 platform binary packages |
| Package manager | `pnpm` (workspace), `cargo` (Rust) |
| Documentation | VitePress (this site) |

## Testing

The Rust crate carries two layers of tests, plus format/lint gates — all wired into CI (`check.yml` runs `cargo fmt --check`, `cargo clippy -D warnings`, and `cargo test`):

- **Unit tests** live next to their implementation in each module's `tests.rs` (declared with `#[cfg(test)] mod tests;`). Pure logic — date/billing math, price formatting, validators, argument parsing, table layout, safe path resolution, crypto round-trips, FX conversion, and the suggest email parser — is covered directly against the implementation.
- **CLI smoke suite** (`rust/crates/subtrack/tests/cli_smoke.rs`) runs the compiled binary end-to-end in a temporary database directory with `SUBTRACK_TODAY` pinned (a Rust-only environment variable that fixes the "today" used by date logic), asserting exit codes and key output fragments for the main command flows.
- **Formatter/lint gates.** `cargo fmt --check` and `cargo clippy --all-targets -D warnings` keep the code base format- and lint-clean; both come from the pinned `rust-toolchain.toml`.

Add a unit test next to the function it covers; extend `cli_smoke.rs` when a new user-facing command lands.

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
├── rust/                          # Rust implementation (multi-crate workspace)
│   ├── Cargo.toml                 # Workspace (crates: ["crates/*"], edition 2024)
│   ├── rust-toolchain.toml        # Pinned toolchain inherited by all crates
│   ├── crates/core/               # subtrack-core — infrastructure & domain layer
│   │   ├── src/
│   │   │   ├── ansi.rs, logger.rs # ANSI helpers & consola-compatible logger (buffer, flush, exit code)
│   │   │   ├── ui.rs, prompt.rs   # Terminal width, interactive prompt core
│   │   │   ├── parser/            # Tokenizer + argument resolution (tokenize.rs, resolve.rs, tests.rs)
│   │   │   ├── domain.rs, models.rs # Status/Cycle/FilterDomain + row types shared by all layers
│   │   │   ├── validation.rs, safe_path.rs
│   │   │   ├── date/              # Date helpers (period.rs, billing.rs, SUBTRACK_TODAY override)
│   │   │   ├── price.rs, fx.rs    # Price formatting & embedded-rate FX (data/fx-rates.json)
│   │   │   ├── json.rs, xlsx.rs   # JS-compatible JSON emitter & XLSX writer
│   │   │   ├── formats.rs         # Document builders (csv / md / ics / xlsx / json export)
│   │   │   ├── calendar.rs, upcoming.rs # Pure billing-day & upcoming math
│   │   │   └── crypto.rs          # AES-256-GCM storage, key dirs, temp files
│   │   └── data/fx-rates.json     # Embedded FX rates (include_str! in fx.rs)
│   ├── crates/ui/                 # subtrack-ui — table rendering engine
│   │   └── src/table/             # (ansi.rs, text.rs, theme.rs, builder.rs, tests.rs)
│   ├── crates/db/                 # subtrack-db — SQLite persistence layer
│   │   └── src/db/                # CRUD, migrations, encrypted save/backup, history
│   ├── crates/config/             # subtrack-config — settings & display themes
│   ├── crates/commands/           # subtrack-commands — command registry, router, help
│   ├── crates/mcp/                # subtrack-mcp — MCP server (20 tools; handlers/)
│   └── crates/subtrack/           # The `subtrack` CLI binary + command handlers
│       ├── src/
│       │   ├── main.rs            # Entry point
│       │   ├── cli.rs             # CLI bootstrap & dispatch (creates Command objects)
│       │   ├── subs/              # add/edit/delete/clone/status/cancel handlers
│       │   ├── list.rs            # list (filters, sorting)
│       │   ├── search.rs          # search
│       │   ├── tags.rs            # tags & tag subcommands
│       │   ├── trial.rs, bulk.rs, dedupe.rs, templates.rs, suggest.rs
│       │   ├── io.rs, export.rs, receipt.rs      # import / export / receipts commands
│       │   ├── payment.rs, analytics.rs, summary.rs, forecast.rs, compare.rs
│       │   ├── budget.rs, optimize.rs, timeline.rs, calendar.rs, upcoming.rs,
│       │   ├── review.rs, history.rs, changes.rs, notify.rs
│       │   ├── report/            # Yearly report + monthly totals (calc.rs, render.rs)
│       │   ├── profile.rs         # Filter profiles
│       │   ├── usage/             # LLM usage tracking (add/list/import/refresh/total)
│       │   ├── prompts.rs         # Prompt helpers composing the two layers
│       │   ├── suggest_parser/    # Suggest email parsers (bank, card, receipt, wallet, generic)
│       │   ├── currency.rs        # Currency list command
│       │   ├── display.rs, backup.rs # Display CLI recipes & backup command
│       │   └── …
│       └── tests/cli_smoke.rs     # End-to-end CLI smoke tests
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
