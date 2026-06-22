---
name: bug-fixes
description: When diagnosing and fixing bugs in the subtrack project
---

# Subtrack Bug Fix Guidelines

## When to Use

- When investigating test failures
- When debugging runtime errors in the CLI
- When fixing logic errors in commands, database queries, or display rendering
- When addressing type errors or compilation failures

## Basic Rules

- **Reproduce first**: Run the failing command or test locally before making changes
- **Write a failing test first**: Add or update a test that captures the bug before fixing
- **One fix per commit**: Keep bug fixes isolated from feature work
- **Run `pnpm test` after every fix**: Verify the fix doesn't break existing behavior
- **Run `pnpm build`**: Confirm the fix compiles cleanly

## Debugging Workflow

1. **Identify the symptom**: What actually breaks? (test failure, runtime crash, wrong output, type error)
2. **Isolate the layer**: Is the bug in commands (`commands.ts`), database (`db.ts`), display (`display.ts`), or prompts (`prompts.ts`)?
3. **Inspect the data path**: Trace the input → DB query → output pipeline
4. **Add a test that reproduces the bug**: Use `__setDb()` for DB-dependent tests, mock `consola` for output assertions
5. **Apply the fix**: Minimal change — don't refactor unrelated code
6. **Verify with `pnpm test`**: All tests must pass

## Common Bug Patterns in This Codebase

| Symptom | Likely Location | Check |
|---|---|---|
| Wrong displayed amount | `display.ts` | FX rate conversion, currency formatting, integer/cent handling |
| Command crashes | `commands.ts` | Missing null checks, unhandled promise rejections |
| Data not persisted | `db.ts` | Missing `saveDb()` call, missing `COMMIT` in transaction |
| Tag not linked | `db.ts` | Missing `subscription_tags` insert, wrong parameter order |
| Test timeout | `*.test.ts` | Unmocked `fetch`, unclosed DB handle |
| Type error on build | Any `.ts` file | `verbatimModuleSyntax` — use `import type` for type-only imports |

## Testing After a Fix

- Always run the full test suite: `pnpm test`
- If the bug is in DB logic, verify with `__setDb()` in isolation
- If the bug is in display logic, verify output with `consola.mockTypes()` assertions
- If the bug involves FX rates, mock `globalThis.fetch`

## Notes

- Don't silence errors with `try/catch` unless you handle them meaningfully
- Don't introduce new dependencies to fix a bug — prefer minimal in-line fixes
- If a bug requires a schema migration, flag it for discussion — schema changes need care
- Test coverage should increase with bug fixes, not decrease
