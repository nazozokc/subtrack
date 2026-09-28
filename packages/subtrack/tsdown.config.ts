import { defineConfig } from "tsdown"

export default defineConfig({
  entry: ["src/launcher.ts", "src/index.ts"],
  // Force-bundle the workspace lib AND gunshi into dist so the published CLI
  // stays self-contained (nothing outside node built-ins), and startup skips
  // node_modules resolution (realpathSync/package_json_reader lookups).
  deps: {
    // Subpath imports (@subtrack/lib/date ...) are only force-bundled when
    // matched with a glob — a bare package name matches exact ids only.
    alwaysBundle: ["@subtrack/lib/*", "gunshi"],
  },
  minify: true,
})
