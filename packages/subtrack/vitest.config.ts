import { defineConfig } from "vitest/config"

export default defineConfig({
  test: {
    // Scanner tests read real editor DBs (file I/O) and restore tests do backups
    testTimeout: 15_000,
    // Force ANSI colors so display helpers can be tested for styling
    env: {
      FORCE_COLOR: "1",
      // Isolate config.json so tests never read the user's real config
      SUBSC_CLI_DB_DIR: "/tmp/subtrack-vitest-config",
    },
    coverage: {
      provider: "v8",
      reporter: ["text", "lcov"],
      include: ["src/**/*.ts"],
      exclude: ["src/__tests__/**"],
    },
  },
})
