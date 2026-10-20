---
title: Installation
description: Install subtrack via npm, pnpm, or build the Rust binary from source.
---

## Requirements

**subtrack** is a native Rust binary distributed through npm. The npm package (`subtrack`) contains a small Node.js launcher that runs the platform binary, so Node.js **22.5+** is required to launch it. The binary itself is self-contained — Linux builds are statically linked (musl) and work on any glibc or musl distribution, including Alpine.

Prebuilt binaries are provided for:

| Platform | Package |
|----------|---------|
| Linux x64 | `@subtrack/subtrack-linux-x64` |
| Linux arm64 | `@subtrack/subtrack-linux-arm64` |
| macOS x64 (Intel) | `@subtrack/subtrack-darwin-x64` |
| macOS arm64 (Apple Silicon) | `@subtrack/subtrack-darwin-arm64` |
| Windows x64 | `@subtrack/subtrack-win32-x64` |
| Windows arm64 | `@subtrack/subtrack-win32-arm64` |

## Install via npm (recommended)

```bash
npm install -g subtrack
```

## Install via pnpm

```bash
pnpm add -g subtrack
```

## Install via bun

```bash
bun add -g subtrack
```

## Run with npx (no install)

If you prefer not to install globally, use `npx`:

```bash
npx subtrack list
```

Note that `npx` will download the package on every first run, so global install is recommended for regular use.

## Build from source

The Rust workspace lives under `rust/` in the monorepo. Building requires a Rust toolchain (the pinned channel comes from `rust-toolchain.toml` at the repository root):

```bash
rustup install     # installs the pinned channel from rust-toolchain.toml
cargo build --release --manifest-path rust/Cargo.toml
```

The binary is written to `rust/target/release/subtrack`. To use it with the npm launcher (or in place of a downloaded platform binary), point the launcher at it with the `SUBTRACK_BINARY` environment variable, or add it to your `PATH` directly:

```bash
export SUBTRACK_BINARY=$PWD/rust/target/release/subtrack
SUBTRACK_BINARY=$PWD/rust/target/release/subtrack subtrack list
```

<div class="callout info">
  <strong>💡 Tip:</strong> Run <code>subtrack --help</code> after installation to verify everything works.
</div>

## Verify installation

```bash
subtrack --help
```

You should see the list of available commands.
