## v2.0.0 (2026-06-15)

### 💥 Breaking Changes

- **Monorepo restructure**: moved to pnpm workspace. The CLI package now lives under `subtrack/`, and a new `docs/` package serves the documentation site. Install dependencies from the root with `pnpm install`. ([`d21c065`](https://github.com/nazozokc/subtrack/commit/d21c065))

### ✨ Features

- Add interactive add/delete mode for managing subscriptions without flags ([`90f1518`](https://github.com/nazozokc/subtrack/commit/90f1518))
- Group subscriptions by currency by default (when no `--currency` flag is given) ([`33e43b6`](https://github.com/nazozokc/subtrack/commit/33e43b6))
- Expand supported currencies and generalize the FX rate API ([`90f1518`](https://github.com/nazozokc/subtrack/commit/90f1518))
- Replace `cli-table3` with a custom inline table renderer with ANSI-aware cell sizing ([`90f1518`](https://github.com/nazozokc/subtrack/commit/90f1518))
- Add SvelteKit documentation site deployed to GitHub Pages ([`d21c065`](https://github.com/nazozokc/subtrack/commit/d21c065))
- Add CLI branding logo to docs and README ([`123315b`](https://github.com/nazozokc/subtrack/commit/123315b), [`8cbb229`](https://github.com/nazozokc/subtrack/commit/8cbb229))

### 🐛 Bug Fixes

- Restrict CI to `subtrack/` directory and rename workflow to `app-ci.yml` ([`38921dc`](https://github.com/nazozokc/subtrack/commit/38921dc))
- Remove explicit pnpm version from `setup-node` action — reads from `package.json` instead ([`da1017f`](https://github.com/nazozokc/subtrack/commit/da1017f))
- Enable GitHub Pages auto-enablement in `configure-pages` action ([`e51ba6b`](https://github.com/nazozokc/subtrack/commit/e51ba6b))

### 📦 Full Changelog

**Full Changelog**: https://github.com/nazozokc/subtrack/compare/v1.1.0...v2.0.0
