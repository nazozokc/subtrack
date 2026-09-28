#!/usr/bin/env node

// Enable Node's module compile cache before loading the CLI entry point.
// Older supported Node versions do not expose this API, so the launcher
// deliberately falls back to the normal import path.
const nodeModule = await import("node:module")
nodeModule.enableCompileCache?.()
await import("./index.ts")
