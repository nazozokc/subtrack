import { defineConfig } from "tsdown"

export default defineConfig({
  entry: ["src/launcher.ts", "src/index.ts"],
  // Force-bundle the workspace lib into dist so the published CLI stays
  // self-contained (nothing outside gunshi/node built-ins).
  deps: {
    // Subpath imports (@subtrack/lib/date ...) are only force-bundled when
    // matched with a glob — a bare package name matches exact ids only.
    alwaysBundle: ["@subtrack/lib/*"],
  },
  minify: true,
})
