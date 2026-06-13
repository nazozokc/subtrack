
Use pnpm for package management and Node.js for runtime.

## Package Management

- Use `pnpm add <package>` instead of `npm install <package>` or `bun add <package>`
- Use `pnpm remove <package>` instead of `npm uninstall <package>` or `bun remove <package>`
- Use `pnpm update` instead of `npm update` or `bun update`
- Use `pnpm run <script>` or `pnpm <script>` instead of `npm run` or `bun run`
- Use `pnpmx <package>` instead of `npx` or `bunx`

## Running

- Use `tsx src/index.ts` for development (TypeScript execution)
- Use `tsdown` for building (already configured in scripts.build)
- Don't use `bun` or `node --loader` for running TypeScript directly

## Testing

- Use `vitest` for running tests (`pnpm test` or `vitest run`)
- Test files are co-located next to source files as `*.test.ts`
- Use `pnpm test:watch` for watch mode

## SQLite

- Use `better-sqlite3` for SQLite
- Don't use `sqlite` (the npm package) or `bun:sqlite`
- Import: `import Database from "better-sqlite3"`

## APIs

- Prefer `node:fs` for file system operations
- Prefer `node:path` for path operations
- Prefer `node:os` for OS-level operations
- Prefer native `fetch` for HTTP requests
- Prefer native `WebSocket` for WebSocket connections

## Code Style

- TypeScript with strict mode enabled
- ESM modules (`"type": "module"` in package.json)
- No semicolons in imports/exports
