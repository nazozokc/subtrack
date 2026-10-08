# rust/fixtures — TypeScript reference goldens

Frozen behavior of the **TypeScript implementation** of subtrack, replayed by the
Rust port. The Rust harness runs the same cases and byte-compares stdout/stderr,
exit codes, and produced files against these goldens.

**The reference implementation is preserved at tag `last-typescript`.**

## Layout

```
rust/fixtures/
├── goldens/<group>/<id>.stdout|.stderr   # normalized outputs (ANSI kept as-is)
├── goldens/<group>/<id>.files/<path>     # new/changed files in the case cwd
├── dbs/populated/                        # shared starting DB (config.json, subtrack.db, .key)
├── dbs/empty                             # marker: case starts with a fresh DB
├── inputs/                               # fixture files (csv, jsonl, backups, fx-rates.json, receipt.txt)
├── cases.jsonl                           # per-case argv, db, stdin, exits, files (365 cases)
├── manifest.json                         # env contract, normalizations, exclusions, counts
└── capture/                              # capture driver (needs the TS build)
```

Groups: `seed` (30, build the populated DB), `help` (52), `reads` (122),
`mutate` (103), `errors` (48), `interactive` (1), `mcp` (1), `legacy` (6),
`legacy-plain` (2).

## Re-capture / verify

Requires the TypeScript reference build:

```bash
pnpm build
pnpm exec tsx rust/fixtures/capture/make-legacy.mjs   # once: legacy fixtures in inputs/
node rust/fixtures/capture/capture.mjs                # capture → sync into rust/fixtures/
node rust/fixtures/capture/capture.mjs --verify       # fresh capture in tmp, diff vs committed
```

After `last-typescript`, the TS sources are gone from the branch — check out the
tag (or keep `dist/`) to regenerate. Prefer fixing the Rust side over
re-capturing; re-capture only when the reference itself was wrong.

## Harness contract (what the Rust test runner must reproduce)

- **cwd**: a `cwd/` dir inside the per-case temp root; snapshot compares
  `<id>.files/` against new/changed files there.
- **HOME**: fake `home/` under the same root; **DB dir**: fake
  `SUBSC_CLI_DB_DIR` under `db/`.
- **Run root**: under the OS temp dir — the CLI's path-safety checks only
  accept `$HOME` and temp bases, so the case root must be inside both.
- **Env**: `TZ=UTC`, `LC_ALL=C.utf8`, `TERM=xterm-256color`, inherit `PATH`
  only. Inject `SUBTRACK_TODAY=2026-10-08` (capture date) — seed dates and
  today-relative outputs depend on it.
- **HTTP**: no network. Replay `inputs/fx-rates.json` for the FX endpoint
  (capture used `capture/fxmock.mjs`: the open.er-api.com URL → 200, others → 404).
- **stdin**: closed unless the case sets `stdin: true`.
- **Multi-step cases**: output prefixed with `<<<step N>>>` (1-based) per step.
- **Exit codes**: from the case's `exits[]` in `cases.jsonl` (one per step).

## Normalization (applied to goldens before storage)

| Token | Meaning |
|---|---|
| `{WORK}` | per-case temp root |
| `{FIXTURES}` | this directory |
| `{REPO}` | repository root |
| `{VERSION}` | package version (boundary-safe) |
| `{DATETIME}` | `YYYY-MM-DD[ T]HH:MM:SS[.sss][Z\|±HH:MM]` |
| `{DATE}` | `YYYY-MM-DD` (applied after DATETIME) |
| `{STAMP}` | `YYYYMMDD_HHMMSS` in backup filenames/paths |

Year-less short dates (`Nov 8`) are stored literally — reproduced via
`SUBTRACK_TODAY`. ANSI escape codes are **not** stripped; compare bytes after
the same normalization.

## File outputs

`<id>.files/` holds new/changed files relative to the case cwd:

- **text** — stored normalized (same tokens as stdout)
- **gzip** (`.db.gz`) — stored raw; gunzip before comparing the DB
- **skip** — `.sha256` sidecars, `.xlsx`, raw `.db` (random IVs /
  implementation-defined bytes): not captured, not compared

## Excluded cases

- `notify` (desktop notification side effects)
- `mcp` bare (covered by the dedicated JSON-RPC probe case)
- `dedupe merge` (shell pipeline — not a single argv)
