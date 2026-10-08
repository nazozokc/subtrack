#!/usr/bin/env node
// Golden capture driver for the TypeScript reference implementation of subtrack.
//
// Captures stdout/stderr/exit codes of the built CLI (packages/subtrack/dist)
// across a deterministic matrix of cases and freezes them under
// rust/fixtures/goldens/ for the Rust port to replay.
//
// Usage:
//   node rust/fixtures/capture/capture.mjs             # capture → sync into rust/fixtures/
//   node rust/fixtures/capture/capture.mjs --verify    # re-capture into a temp dir and diff
//                                                       # against the committed goldens
//   node rust/fixtures/capture/capture.mjs --out DIR   # sync results into DIR instead
//
// All runs happen under os.tmpdir(): the CLI's path-safety checks only accept
// $HOME and temp bases, and every case runs with a fake HOME inside the temp
// root — the Rust harness must do the same.
//
// Prerequisites:
//   pnpm build                      # packages/subtrack/dist/index.mjs
//   pnpm exec tsx rust/fixtures/capture/make-legacy.mjs
//
// Determinism measures (see manifest.json for the authoritative list):
//   - per-case fake HOME / SUBSC_CLI_DB_DIR under a temp root
//   - TZ=UTC, LC_ALL=C.utf8, minimal inherited environment
//   - all HTTP goes through capture/fxmock.mjs (loaded via `node --import`)
//   - seed dates are computed relative to the capture date; the Rust harness
//     must inject that date via SUBTRACK_TODAY
//   - output normalization: temp paths, version, dates/times, file stamps

import { spawnSync } from "node:child_process"
import { createHash } from "node:crypto"
import {
  cpSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync,
  rmSync, writeFileSync,
} from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { fileURLToPath } from "node:url"

// ── Locations & constants ────────────────────────────────────────

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const fixturesDir = path.resolve(scriptDir, "..")
const repoRoot = path.resolve(fixturesDir, "..", "..")
const cliPath = path.join(repoRoot, "packages", "subtrack", "dist", "index.mjs")
const docsPath = path.join(repoRoot, "docs", "commands.md")
const commandsIndexPath = path.join(repoRoot, "packages", "subtrack", "src", "commands", "index.ts")
const fxMockPath = path.join(scriptDir, "fxmock.mjs")
const inputsDir = path.join(fixturesDir, "inputs")

const REFERENCE_VERSION = JSON.parse(
  readFileSync(path.join(repoRoot, "packages", "subtrack", "package.json"), "utf8"),
).version

const DEFAULT_TIMEOUT_MS = 15_000
const MCP_TIMEOUT_MS = 10_000
const MENU_TIMEOUT_MS = 8_000
const MAX_BUFFER = 64 * 1024 * 1024

// ── CLI execution ────────────────────────────────────────────────

/** Minimal, deterministic environment: nothing inherited but PATH. */
function baseEnv(dbDir, homeDir) {
  return {
    PATH: process.env.PATH ?? "/usr/bin:/bin",
    HOME: homeDir,
    TZ: "UTC",
    LC_ALL: "C.utf8",
    LANG: "C.utf8",
    TERM: "xterm-256color",
    SUBSC_CLI_DB_DIR: dbDir,
  }
}

function spawnCli(args, { cwd, env, input = "", timeout = DEFAULT_TIMEOUT_MS }) {
  const res = spawnSync(
    process.execPath,
    ["--import", fxMockPath, cliPath, ...args],
    { cwd, env, input, timeout, maxBuffer: MAX_BUFFER, encoding: null },
  )
  if (res.error && res.error.code === "ETIMEDOUT") {
    return { stdout: Buffer.alloc(0), stderr: Buffer.alloc(0), status: -1, timedOut: true }
  }
  if (res.error) throw res.error
  return { stdout: res.stdout, stderr: res.stderr, status: res.status ?? -1, timedOut: false }
}

// ── Normalization ────────────────────────────────────────────────

const escapeRe = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")

function makeNormalizer(caseRoot) {
  const literal = [
    [caseRoot, "{WORK}"],
    [fixturesDir, "{FIXTURES}"],
    [repoRoot, "{REPO}"],
  ]
  const versionRe = new RegExp(`(?<![\\d.])${escapeRe(REFERENCE_VERSION)}(?![\\d.])`, "g")
  return (text) => {
    let s = text
    for (const [from, to] of literal) s = s.split(from).join(to)
    s = s.replace(versionRe, "{VERSION}")
    s = s.replace(/\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?/g, "{DATETIME}")
    s = s.replace(/\d{4}-\d{2}-\d{2}/g, "{DATE}")
    s = s.replace(/\d{8}_\d{6}/g, "{STAMP}")
    return s
  }
}

const MONTHS = "Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec"

/**
 * Verify-only canonicalization: maps every date-ish rendering to a token so a
 * capture re-run on a later day can be compared against the committed golden.
 * Applied to BOTH sides of a --verify diff; never stored.
 */
const canonDates = (s) =>
  s
    .replace(/\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?/g, "{DATETIME}")
    .replace(/\d{4}-\d{2}-\d{2}/g, "{DATE}")
    .replace(new RegExp(`\\b(?:${MONTHS}) \\d{1,2}\\b`, "g"), "{SDATE}")
    .replace(new RegExp(`\\b\\d{1,2} (?:${MONTHS}) \\d{4}\\b`, "g"), "{DATE}")
    .replace(/\d{8}_\d{6}/g, "{STAMP}")

const isUtf8 = (buf) => Buffer.from(buf.toString("utf8"), "utf8").equals(buf)

// ── docs harvest ─────────────────────────────────────────────────

/** Extract `subtrack …` argv arrays from fenced ```bash blocks in docs/commands.md. */
function harvestDocs(md) {
  const lines = md.split("\n")
  const commands = []
  let inBash = false
  let cmd = ""

  const flush = () => {
    if (cmd) commands.push(cmd.trim())
    cmd = ""
  }

  for (const line of lines) {
    if (!inBash) {
      if (/^```bash/.test(line)) inBash = true
      continue
    }
    if (/^```/.test(line)) {
      flush()
      inBash = false
      continue
    }
    const t = line.trim()
    if (!t || t.startsWith("#")) continue

    if (!cmd) {
      if (/^(\$ )?subtrack(\s|$)/.test(t)) cmd = t.replace(/^\$ /, "")
    } else if (cmd.endsWith("\\")) {
      cmd = `${cmd.slice(0, -1).trimEnd()} ${t}`
    } else {
      flush()
      if (/^(\$ )?subtrack(\s|$)/.test(t)) cmd = t.replace(/^\$ /, "")
    }
    if (cmd && !cmd.endsWith("\\")) flush()
  }
  return commands
}

function tokenize(cmdline) {
  const out = []
  const re = /"([^"]*)"|'([^']*)'|(\S+)/g
  let m
  while ((m = re.exec(cmdline))) out.push(m[1] ?? m[2] ?? m[3])
  return out
}

const MUTATE_COMMANDS = new Set([
  "add", "edit", "delete", "clone", "archive", "unarchive", "cancel",
  "pause", "resume", "renew", "bulk", "import", "restore", "backup",
  "receipt", "review", "cleanup", "maintenance",
])

function classify(args) {
  const [cmd, sub] = args
  if (cmd === "dedupe") return args.includes("merge") ? "mutate" : "reads"
  if (cmd === "config") return ["set", "reset"].includes(sub) ? "mutate" : "reads"
  if (cmd === "usage") return ["add", "edit", "delete", "import"].includes(sub) ? "mutate" : "reads"
  if (cmd === "trial") return ["add", "delete"].includes(sub) ? "mutate" : "reads"
  if (cmd === "tag") return ["rename", "delete", "prune", "merge"].includes(sub) ? "mutate" : "reads"
  if (cmd === "template") return !sub || ["list", "show"].includes(sub) ? "reads" : "mutate"
  if (cmd === "profile") return !sub || ["list", "show"].includes(sub) ? "reads" : "mutate"
  if (cmd === "suggest") return ["add", "dismiss"].includes(sub) ? "mutate" : "reads"
  if (cmd === "audit") return sub === "prune" ? "mutate" : "reads"
  return MUTATE_COMMANDS.has(cmd) ? "mutate" : "reads"
}

function notifyChannel(args) {
  const i = args.findIndex((a) => a === "--channel" || a.startsWith("--channel="))
  if (i === -1) return null
  if (args[i].includes("=")) return args[i].split("=")[1]
  return args[i + 1] ?? null
}

function exclusionReason(raw, args) {
  if (/[|<>`$]/.test(raw)) {
    return "shell pipeline, redirection, or substitution — not capturable as a single argv"
  }
  if (args[0] === "mcp") return "covered by the dedicated mcp probe case"
  if (args[0] === "notify") {
    const ch = notifyChannel(args)
    const safe =
      args.includes("--dry-run") ||
      args.includes("--json") ||
      (args.includes("--channel") && ["slack", "webhook"].includes(ch))
    if (!safe) {
      return "OS/desktop notification side effects (notify-send/osascript) — environment dependent"
    }
  }
  return null
}

// ── Case definitions ─────────────────────────────────────────────

function listCommands() {
  const src = readFileSync(commandsIndexPath, "utf8")
  const m = /export const subCommands = \{([\s\S]*?)\n\}/.exec(src)
  if (!m) throw new Error("cannot parse subCommands from commands/index.ts")
  return [...m[1].matchAll(/^\s{2}([a-z][a-zA-Z0-9]*):/gm)].map((x) => x[1])
}

const isoDate = (d) => d.toISOString().slice(0, 10)

function buildSeedSteps(now) {
  const offset = (n) => {
    const d = new Date(now)
    d.setUTCDate(d.getUTCDate() + n)
    return isoDate(d)
  }
  const today = isoDate(now)
  return [
    ["add", "--name", "Netflix", "--price", "14.99", "--currency", "USD", "--cycle", "monthly", "--billingDay", "15", "--status", "active", "--tags", "video,4k", "--paymentMethod", "credit_card", "--vendorName", "Netflix Inc.", "--vendorUrl", "https://www.netflix.com", "--planTier", "Standard"],
    ["add", "--name", "Spotify", "--price", "5.99", "--currency", "USD", "--cycle", "monthly", "--billingDay", "1", "--status", "active", "--tags", "music", "--paymentMethod", "paypal"],
    ["add", "--name", "Adobe Creative Cloud", "--price", "5999", "--currency", "JPY", "--cycle", "yearly", "--billingDay", "20", "--status", "active", "--tags", "design,work", "--contractStart", "2025-03-01", "--contractEnd", "2027-03-01", "--autoRenewal", "false", "--planTier", "All Apps"],
    ["add", "--name", "GitHub Pro", "--price", "4.00", "--currency", "USD", "--cycle", "monthly", "--billingDay", "12", "--status", "active", "--tags", "dev", "--paymentMethod", "paypal"],
    ["add", "--name", "Local Gym", "--price", "12000", "--currency", "JPY", "--cycle", "monthly", "--billingDay", "5", "--status", "paused", "--tags", "health"],
    ["add", "--name", "Old Magazine", "--price", "9.99", "--currency", "EUR", "--cycle", "quarterly", "--billingDay", "3", "--status", "cancelled", "--tags", "reading"],
    ["add", "--name", "iCloud+", "--price", "0.99", "--currency", "USD", "--cycle", "monthly", "--billingDay", "25", "--status", "active", "--tags", "cloud"],
    ["add", "--name", "AWS", "--price", "50.00", "--currency", "USD", "--cycle", "monthly", "--billingDay", "1", "--status", "active", "--tags", "work", "--paymentMethod", "credit_card", "--vendorName", "Amazon", "--vendorUrl", "https://aws.amazon.com", "--planTier", "Business"],
    ["add", "--name", "Netflix", "--price", "19.99", "--currency", "USD", "--cycle", "monthly", "--billingDay", "20", "--status", "active", "--tags", "video"],
    ["add", "--name", "Weekly Paper", "--price", "2.50", "--currency", "USD", "--cycle", "weekly", "--billingDay", "1", "--status", "active", "--tags", "news"],
    ["add", "--name", "Bus Pass", "--price", "20.00", "--currency", "USD", "--cycle", "bi-weekly", "--billingDay", "2", "--status", "active", "--tags", "transport"],
    ["add", "--name", "Domain Renewal", "--price", "1500", "--currency", "JPY", "--cycle", "semi-annual", "--billingDay", "12", "--status", "active", "--tags", "dev"],
    ["add", "--name", "Discord Nitro", "--price", "9.99", "--currency", "USD", "--cycle", "yearly", "--billingDay", "8", "--status", "active", "--tags", "video", "--discountAmount", "10", "--discountType", "percentage"],
    ["archive", "7"],
    ["edit", "1", "--price", "15.99"],
    ["trial", "add", "--name", "Spotify Premium Trial", "--expires-at", offset(-3), "--price", "980", "--currency", "JPY", "--cycle", "monthly", "--notes", "expired trial"],
    ["trial", "add", "--name", "Netflix Trial", "--expires-at", offset(6), "--price", "14.99", "--currency", "USD", "--cycle", "monthly"],
    ["trial", "add", "--name", "Adobe Trial", "--expires-at", offset(60), "--price", "5999", "--currency", "JPY", "--cycle", "yearly"],
    ["usage", "add", "--provider", "openai", "--model", "gpt-4o", "--input-tokens", "500", "--output-tokens", "200", "--date", today, "--cost", "0.15", "--description", "Chat completion"],
    ["usage", "add", "--provider", "anthropic", "--model", "claude-3-opus-20240229", "--input-tokens", "1200", "--output-tokens", "800", "--date", today, "--cost", "0.42"],
    ["usage", "add", "--provider", "openai", "--model", "gpt-4o-mini", "--input-tokens", "3000", "--output-tokens", "700", "--date", offset(-111), "--cost", "0.07", "--description", "June batch"],
    ["config", "set", "monthlyBudget", "500"],
    ["config", "set", "yearlyBudget", "6000"],
    ["config", "set", "notifyDays", "14"],
    ["config", "set", "listShowNotes", "on"],
    ["config", "set", "listShowMethod", "on"],
    ["config", "set", "slackWebhook", "https://hooks.slack.com/redacted"],
    ["config", "set", "webhookUrl", "https://example.com/hook"],
    ["template", "add", "spotify", "--price", "980", "--currency", "JPY", "--cycle", "monthly", "--tags", "music"],
    ["profile", "save", "work", "--tag", "work", "--status", "active"],
  ]
}

const CURATED_READS = [
  ["check", "--strict"],
  ["dedupe"],
  ["history", "1"],
  ["audit", "list", "--from", "2026-01-01", "--to", "2026-06-30"],
  ["export", "excel"],
  ["export", "excel", "-o", "out.xlsx"],
  ["receipt", "receipt.txt", "--dryRun"],
  ["receipt", "receipt.txt", "--json"],
  ["usage", "import", "./responses.jsonl", "--dry-run"],
]

const CURATED_MUTATE = [
  ["restore", "golden-backup.db.gz", "--force"],
  ["restore", "legacy-backup.db", "--force"],
  ["backup", "."],
  ["backup", "./backups"],
  ["import", "./openai-responses.jsonl"],
  ["usage", "import", "./openai-responses.jsonl"],
  ["template", "use", "spotify"],
  ["config", "set", "defaultCurrency", "JPY"],
]

/** Multi-step chains: write, then read back — parity for read-after-write paths. */
const CURATED_CHAINS = [
  [
    ["config", "set", "defaultCurrency", "JPY"],
    ["budget"],
    ["list", "--currency", "JPY"],
    ["summary"],
  ],
  [
    ["edit", "3", "--price", "1500"],
    ["list"],
    ["history", "3"],
  ],
  [
    ["delete", "2", "--force"],
    ["list"],
    ["tags", "video"],
  ],
  [
    ["usage", "import", "./openai-responses.jsonl"],
    ["usage", "list"],
    ["usage", "total"],
  ],
  [
    ["template", "use", "spotify", "--price", "1080"],
    ["list"],
  ],
]

const CURATED_ERRORS = [
  ["bogus-command"],
  ["list", "--bogus-flag"],
  ["list", "--min-price", "abc"],
  ["list", "--max-price", "5", "--min-price", "10"],
  ["list", "--status", "bogus"],
  ["list", "--limit", "0"],
  ["list", "--limit", "abc"],
  ["list", "--sort", "bogus"],
  ["edit", "999", "--price", "10"],
  ["delete", "999", "--force"],
  ["clone", "999"],
  ["archive", "999"],
  ["pause", "999"],
  ["resume", "999"],
  ["renew", "999"],
  ["cancel", "999"],
  ["history", "999"],
  ["add", "--name", "X", "--price", "abc", "--currency", "USD", "--cycle", "monthly", "--status", "active"],
  ["add", "--name", "X", "--price", "1", "--currency", "USD", "--cycle", "monthly", "--status", "active", "--billingDay", "99"],
  ["add", "--name", "X", "--price", "1", "--currency", "USD", "--cycle", "bogus", "--status", "active"],
  ["add", "--name", "X", "--price", "1", "--currency", "ZZZ", "--cycle", "monthly", "--status", "active"],
  ["add", "--name", "X", "--price", "1", "--currency", "USD", "--cycle", "monthly", "--status", "bogus"],
  ["import", "missing-file.csv"],
  ["restore", "missing-file.db.gz", "--force"],
  ["export", "bogus-format"],
  ["config", "get", "notAKey"],
  ["config", "set", "notAKey", "value"],
  ["config", "set", "monthlyBudget", "abc"],
  ["tag", "rename", "nope", "other"],
  ["trial", "add", "--name", "X", "--expires-at", "not-a-date"],
  ["usage", "add", "--provider", "openai", "--model", "m", "--input-tokens", "x", "--output-tokens", "1", "--cost", "0.1"],
  ["payment", "bogus-period"],
  ["yearly", "--year", "abc"],
  ["upcoming", "abc"],
  ["bulk", "status", "--set", "bogus"],
  ["usage", "edit", "999", "--cost", "1"],
  ["usage", "delete", "999"],
  ["template", "delete", "nope"],
  ["profile", "switch", "nope"],
  ["profile", "delete", "nope"],
  ["suggest", "view", "999"],
  ["suggest", "add", "999"],
  ["suggest", "dismiss", "999"],
  ["trial", "delete", "999"],
  ["changes", "--from", "not-a-date", "--to", "2026-12-31"],
  ["audit", "list", "--from", "not-a-date"],
  ["receipt", "missing.txt"],
  ["usage", "import", "missing.jsonl"],
]

const MCP_STDIN = [
  { jsonrpc: "2.0", id: 1, method: "initialize", params: { protocolVersion: "2024-11-05", capabilities: {}, clientInfo: { name: "capture", version: "1" } } },
  { jsonrpc: "2.0", method: "notifications/initialized" },
  { jsonrpc: "2.0", id: 2, method: "tools/list" },
  { jsonrpc: "2.0", id: 3, method: "tools/call", params: { name: "list_subscriptions", arguments: {} } },
  { jsonrpc: "2.0", id: 4, method: "tools/call", params: { name: "get_subscription", arguments: { id: 1 } } },
  { jsonrpc: "2.0", id: 5, method: "shutdown" },
].map((m) => JSON.stringify(m)).join("\n") + "\n"

function buildCaseList() {
  const excluded = []
  const cases = []
  const add = (group, steps, opts = {}) =>
    cases.push({ group, steps, db: opts.db ?? "populated", stdin: opts.stdin, timeoutMs: opts.timeoutMs })

  const commands = listCommands()

  // help
  add("help", [["--help"]], { db: "empty" })
  add("help", [["--version"]], { db: "empty" })
  for (const cmd of commands) add("help", [[cmd, "--help"]], { db: "empty" })

  // docs-harvested
  const seen = new Set()
  for (const raw of harvestDocs(readFileSync(docsPath, "utf8"))) {
    const args = tokenize(raw)
    if (args[0] !== "subtrack") continue
    const argv = args.slice(1)
    if (argv.length === 0) continue // bare `subtrack` handled by interactive group
    const key = JSON.stringify(argv)
    if (seen.has(key)) continue
    seen.add(key)
    const reason = exclusionReason(raw, argv)
    if (reason) {
      excluded.push({ command: raw, reason })
      continue
    }
    add(classify(argv), [argv])
  }

  // curated single-step cases (skip ones already harvested from docs)
  for (const argv of CURATED_READS) {
    const key = JSON.stringify(argv)
    if (seen.has(key)) continue
    seen.add(key)
    add("reads", [argv])
  }
  for (const argv of CURATED_MUTATE) {
    const key = JSON.stringify(argv)
    if (seen.has(key)) continue
    seen.add(key)
    add("mutate", [argv])
  }
  for (const steps of CURATED_CHAINS) add("mutate", steps)
  for (const argv of CURATED_ERRORS) add("errors", [argv])

  // interactive
  add("interactive", [[]], { db: "populated", timeoutMs: MENU_TIMEOUT_MS })

  // mcp
  add("mcp", [["mcp"]], { db: "populated", stdin: MCP_STDIN, timeoutMs: MCP_TIMEOUT_MS })

  // legacy
  add("legacy", [["list"]], { db: "legacy" })
  add("legacy", [["list", "--json"]], { db: "legacy" })
  add("legacy", [["check"]], { db: "legacy" })
  add("legacy", [["summary"]], { db: "legacy" })
  add("legacy", [["stats"]], { db: "legacy" })
  add("legacy", [["export", "json"]], { db: "legacy" })
  add("legacy-plain", [["list"]], { db: "legacy-plain" })
  add("legacy-plain", [["check"]], { db: "legacy-plain" })

  return { cases, excluded }
}

// ── Case execution ───────────────────────────────────────────────

const slug = (argv) =>
  argv.join(" ").replace(/[^a-zA-Z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 60) || "root"

function listFilesRecursive(dir, prefix = "") {
  const out = []
  if (!existsSync(dir)) return out
  for (const entry of readdirSync(dir, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
    const rel = prefix ? `${prefix}/${entry.name}` : entry.name
    if (entry.isDirectory()) out.push(...listFilesRecursive(path.join(dir, entry.name), rel))
    else out.push(rel)
  }
  return out
}

function fileMode(name) {
  if (name.endsWith(".gz")) return "gzip"
  if (name.endsWith(".xlsx")) return "skip"
  if (name.endsWith(".sha256")) return "skip"
  if (name === "subtrack.db" || name.endsWith(".db")) return "skip"
  return "bytes"
}

function runCase(def, id, outRoot, populatedDir, legacyDirs) {
  const caseRoot = path.join(outRoot, "work", def.group, id)
  const dbDir = path.join(caseRoot, "db")
  const cwdDir = path.join(caseRoot, "cwd")
  const homeDir = path.join(caseRoot, "home")
  for (const d of [dbDir, cwdDir, homeDir]) mkdirSync(d, { recursive: true, mode: 0o700 })

  if (def.db === "populated") cpSync(populatedDir, dbDir, { recursive: true })
  else if (legacyDirs[def.db]) cpSync(legacyDirs[def.db], dbDir, { recursive: true })
  cpSync(inputsDir, cwdDir, { recursive: true })

  const norm = makeNormalizer(caseRoot)
  const env = baseEnv(dbDir, homeDir)
  const baseline = new Map(
    listFilesRecursive(inputsDir).map((rel) => [rel, readFileSync(path.join(inputsDir, rel))]),
  )

  const stdoutChunks = []
  const stderrChunks = []
  const exits = []
  const timeouts = []

  def.steps.forEach((argv, i) => {
    const separator = def.steps.length > 1 ? Buffer.from(`<<<step ${i + 1}>>>\n`) : Buffer.alloc(0)
    const r = spawnCli(argv, { cwd: cwdDir, env, input: def.stdin ?? "", timeout: def.timeoutMs ?? DEFAULT_TIMEOUT_MS })
    stdoutChunks.push(separator, r.stdout)
    stderrChunks.push(separator, r.stderr)
    exits.push(r.status)
    timeouts.push(r.timedOut)
  })

  const rawStdout = Buffer.concat(stdoutChunks)
  const rawStderr = Buffer.concat(stderrChunks)
  const binary = !isUtf8(rawStdout) || !isUtf8(rawStderr)

  const goldensDir = path.join(outRoot, "goldens", def.group)
  mkdirSync(goldensDir, { recursive: true })
  if (binary) {
    writeFileSync(path.join(goldensDir, `${id}.stdout`), rawStdout)
    writeFileSync(path.join(goldensDir, `${id}.stderr`), rawStderr)
  } else {
    writeFileSync(path.join(goldensDir, `${id}.stdout`), norm(rawStdout.toString("utf8")))
    writeFileSync(path.join(goldensDir, `${id}.stderr`), norm(rawStderr.toString("utf8")))
  }

  // New/changed files written into the case cwd (relative to the inputs baseline).
  // Generated filenames embed a timestamp (backup stamps) — normalize STAMP in
  // stored paths so the Rust harness can pair files across runs.
  const files = []
  for (const relRaw of listFilesRecursive(cwdDir)) {
    const after = readFileSync(path.join(cwdDir, relRaw))
    const before = baseline.get(relRaw)
    if (before && before.equals(after)) continue
    const rel = relRaw.replace(/\d{8}_\d{6}/g, "{STAMP}")
    const mode = fileMode(relRaw)
    files.push({
      path: rel,
      mode,
      sha256: createHash("sha256").update(after).digest("hex"),
    })
    if (mode === "skip") continue
    const dest = path.join(goldensDir, `${id}.files`, rel)
    mkdirSync(path.dirname(dest), { recursive: true })
    if (mode === "gzip" || !isUtf8(after)) writeFileSync(dest, after)
    else writeFileSync(dest, norm(after.toString("utf8")))
  }

  rmSync(caseRoot, { recursive: true, force: true })

  return {
    id,
    group: def.group,
    steps: def.steps,
    db: def.db,
    stdin: def.stdin ? true : false,
    exits,
    timeouts: timeouts.some(Boolean),
    binary,
    files,
  }
}

// ── Seed / populated fixture ─────────────────────────────────────

/**
 * Runs the seed sequence, writes goldens/seed/*, and (unless verify)
 * rebuilds fixtures/dbs/populated + inputs/golden-backup.db.gz.
 */
function buildPopulated(outRoot, now, updateFixtures) {
  const seedRoot = path.join(outRoot, ".seed-work")
  rmSync(seedRoot, { recursive: true, force: true })
  const dbDir = path.join(seedRoot, "db")
  const cwdDir = path.join(seedRoot, "cwd")
  const homeDir = path.join(seedRoot, "home")
  for (const d of [dbDir, cwdDir, homeDir]) mkdirSync(d, { recursive: true, mode: 0o700 })
  cpSync(inputsDir, cwdDir, { recursive: true })

  const norm = makeNormalizer(seedRoot)
  const env = baseEnv(dbDir, homeDir)
  const steps = buildSeedSteps(now)
  const entries = []
  let failed = false

  const goldensDir = path.join(outRoot, "goldens", "seed")
  mkdirSync(goldensDir, { recursive: true })

  steps.forEach((argv, i) => {
    const separator = Buffer.from(`<<<step ${i + 1}>>>\n`)
    const r = spawnCli(argv, { cwd: cwdDir, env, input: "", timeout: DEFAULT_TIMEOUT_MS })
    const out = Buffer.concat([separator, r.stdout])
    const err = Buffer.concat([separator, r.stderr])
    if (r.status !== 0 || r.timedOut) {
      failed = true
      console.error(`seed step ${i + 1} failed (exit ${r.status}${r.timedOut ? ", timeout" : ""}):`)
      console.error(`  argv: ${JSON.stringify(argv)}`)
      console.error(`  stdout:\n${out.toString("utf8")}`)
      console.error(`  stderr:\n${err.toString("utf8")}`)
      return
    }
    const id = `${String(i + 1).padStart(3, "0")}-${slug(argv)}`
    writeFileSync(path.join(goldensDir, `${id}.stdout`), norm(out.toString("utf8")))
    writeFileSync(path.join(goldensDir, `${id}.stderr`), norm(err.toString("utf8")))
    entries.push({ id, group: "seed", steps: [argv], db: "empty", stdin: false, exits: [r.status], timeouts: false, binary: false, files: [] })
  })

  if (failed) {
    rmSync(seedRoot, { recursive: true, force: true })
    throw new Error("seed failed — fixture not updated")
  }

  if (updateFixtures) {
    // Populate fixtures/dbs/populated atomically (only on full success).
    // cpSync, not renameSync: the temp root is usually on another device.
    const populatedDir = path.join(fixturesDir, "dbs", "populated")
    const stagingDir = path.join(outRoot, ".populated-staging")
    rmSync(stagingDir, { recursive: true, force: true })
    cpSync(dbDir, stagingDir, { recursive: true })
    rmSync(populatedDir, { recursive: true, force: true })
    mkdirSync(path.dirname(populatedDir), { recursive: true })
    cpSync(stagingDir, populatedDir, { recursive: true })
    rmSync(stagingDir, { recursive: true, force: true })

    // Regenerate the backup fixture used by the restore goldens.
    // backup destinations must live within $HOME (safePath), which is the
    // seed's fake home — the result is moved into inputs/ afterwards.
    const backupDest = path.join(homeDir, "backups")
    mkdirSync(backupDest, { recursive: true, mode: 0o700 })
    for (const f of readdirSync(backupDest)) rmSync(path.join(backupDest, f))
    const bk = spawnCli(["backup", backupDest], { cwd: cwdDir, env, input: "", timeout: DEFAULT_TIMEOUT_MS })
    if (bk.status !== 0) {
      console.error(`seed backup failed (exit ${bk.status}):\n${bk.stdout}\n${bk.stderr}`)
      throw new Error("seed backup failed")
    }
    const produced = readdirSync(backupDest).filter((f) => /\.db\.gz(\.sha256)?$/.test(f))
    for (const f of produced) {
      const to = f.replace(/^subtrack_\d{8}_\d{6}\.db\.gz/, "golden-backup.db.gz")
      // cross-device safe: copy + unlink (temp root is usually not on the repo's fs)
      const dest = path.join(inputsDir, to)
      cpSync(path.join(backupDest, f), dest)
      rmSync(path.join(backupDest, f))
    }
  }

  rmSync(seedRoot, { recursive: true, force: true })
  return entries
}

// ── Verify mode: compare two capture outputs ─────────────────────

async function gzipEqualCanon(a, b) {
  const { gunzipSync } = await import("node:zlib")
  try {
    return canonDates(gunzipSync(a).toString("latin1")) === canonDates(gunzipSync(b).toString("latin1"))
  } catch {
    return false
  }
}

async function compareOutputs(committedRoot, freshRoot) {
  const diffs = []
  const push = (file, why) => diffs.push(`${file}: ${why}`)

  // cases.jsonl — date-canonicalized (seed argv embeds capture-relative dates;
  // skip/gzip file shas are intentionally nondeterministic across re-seeds).
  const loadJsonl = (root) =>
    readFileSync(path.join(root, "cases.jsonl"), "utf8").split("\n").filter(Boolean).map((l) => JSON.parse(l))
  const canonEntry = (e) =>
    canonDates(JSON.stringify({ ...e, files: e.files.filter((f) => f.mode === "bytes") }))
  const a = loadJsonl(committedRoot)
  const b = loadJsonl(freshRoot)
  const bByKey = new Map(b.map((e) => [`${e.group}/${e.id}`, e]))
  for (const entry of a) {
    const other = bByKey.get(`${entry.group}/${entry.id}`)
    if (!other) {
      push(`cases.jsonl ${entry.group}/${entry.id}`, "missing in fresh capture")
      continue
    }
    if (canonEntry(entry) !== canonEntry(other)) push(`cases.jsonl ${entry.group}/${entry.id}`, "entry differs")
  }
  for (const entry of b) {
    if (!a.some((e) => e.group === entry.group && e.id === entry.id)) {
      push(`cases.jsonl ${entry.group}/${entry.id}`, "unexpected extra entry")
    }
  }

  // manifest.json — date-canonicalized (capturedAt / capturedOnDate / injection differ).
  const manA = canonDates(readFileSync(path.join(committedRoot, "manifest.json"), "utf8"))
  const manB = canonDates(readFileSync(path.join(freshRoot, "manifest.json"), "utf8"))
  if (manA !== manB) push("manifest.json", "differs (besides capture timestamps)")

  // goldens/ — text compared date-canonically; gzip/skip content is
  // re-seed-dependent and covered by entries instead.
  const walk = (root, prefix = "") => {
    const out = []
    for (const e of readdirSync(path.join(root, prefix), { withFileTypes: true })) {
      const rel = prefix ? `${prefix}/${e.name}` : e.name
      if (e.isDirectory()) out.push(...walk(root, rel))
      else out.push(rel)
    }
    return out
  }
  const goldenRel = (root) => {
    const g = path.join(root, "goldens")
    return existsSync(g) ? walk(g).map((f) => `goldens/${f}`) : []
  }
  const filesA = new Set(goldenRel(committedRoot))
  const filesB = new Set(goldenRel(freshRoot))
  for (const f of filesA) {
    if (!filesB.has(f)) {
      push(f, "missing in fresh capture")
      continue
    }
    const ba = readFileSync(path.join(committedRoot, f))
    const bb = readFileSync(path.join(freshRoot, f))
    if (ba.equals(bb)) continue
    if (f.includes(".files/")) {
      if (f.endsWith(".gz")) {
        if (!(await gzipEqualCanon(ba, bb))) push(f, "gzip content differs")
      } else if (isUtf8(ba) && isUtf8(bb)) {
        if (canonDates(ba.toString("utf8")) !== canonDates(bb.toString("utf8"))) push(f, "content differs")
      } else if (!ba.equals(bb)) {
        push(f, "binary content differs")
      }
    } else if (isUtf8(ba) && isUtf8(bb)) {
      if (canonDates(ba.toString("utf8")) !== canonDates(bb.toString("utf8"))) push(f, "content differs")
    } else {
      push(f, "binary content differs")
    }
  }
  for (const f of filesB) {
    if (!filesA.has(f)) push(f, "unexpected extra golden")
  }

  return diffs
}

// ── Main ─────────────────────────────────────────────────────────

async function main() {
  const argv = process.argv.slice(2)
  const verify = argv.includes("--verify")
  const outIdx = argv.indexOf("--out")
  const outArg = outIdx !== -1 ? argv[outIdx + 1] : null
  const destRoot = outArg ? path.resolve(outArg) : fixturesDir

  if (!existsSync(cliPath)) {
    console.error(`missing build: ${cliPath}\nrun: pnpm build`)
    process.exit(1)
  }
  for (const d of ["legacy", "legacy-plain"]) {
    if (!existsSync(path.join(fixturesDir, "dbs", d, "subtrack.db"))) {
      console.error(`missing legacy fixture: dbs/${d}/subtrack.db\nrun: pnpm exec tsx rust/fixtures/capture/make-legacy.mjs`)
      process.exit(1)
    }
  }

  // IMPORTANT: every run happens under os.tmpdir(). path-safety checks in the
  // CLI (safePath/safeOutputPath) allow only $HOME and temp bases; the fake
  // HOME lives inside the temp root, so a repo-directory run root would make
  // legitimate file operations fail differently than the Rust harness (which
  // will also run under a temp root).
  const workRoot = mkdtempSync(path.join(tmpdir(), "subtrack-capture-"))
  const outRoot = workRoot

  const now = new Date()
  const updateFixtures = !verify

  // 1. Seed → goldens/seed (+ fixtures/dbs/populated + inputs/golden-backup when capturing).
  const seedEntries = buildPopulated(outRoot, now, updateFixtures)
  const populatedDir = path.join(fixturesDir, "dbs", "populated")

  // 2. Build the case list.
  const { cases, excluded } = buildCaseList()

  // 3. Run cases with group-scoped sequence numbers (stable across modes).
  const legacyDirs = {
    legacy: path.join(fixturesDir, "dbs", "legacy"),
    "legacy-plain": path.join(fixturesDir, "dbs", "legacy-plain"),
  }
  const groupSeq = new Map()
  const entries = [...seedEntries]
  for (const def of cases) {
    const n = (groupSeq.get(def.group) ?? 0) + 1
    groupSeq.set(def.group, n)
    const id = `${String(n).padStart(3, "0")}-${slug(def.steps[0] ?? [])}`
    entries.push(runCase(def, id, outRoot, populatedDir, legacyDirs))
    process.stderr.write(`\r${def.group}: ${id}          `)
  }
  process.stderr.write("\n")

  // 4. cases.jsonl
  writeFileSync(
    path.join(outRoot, "cases.jsonl"),
    entries.map((e) => JSON.stringify(e)).join("\n") + "\n",
  )

  // 5. manifest.json
  const counts = {}
  for (const e of entries) counts[e.group] = (counts[e.group] ?? 0) + 1
  const manifest = {
    purpose:
      "Golden outputs of the TypeScript reference implementation of subtrack, replayed by the Rust port.",
    reference: {
      package: "subtrack",
      version: REFERENCE_VERSION,
      source: "packages/subtrack (TypeScript)",
      commandCount: listCommands().length,
    },
    capturedAt: now.toISOString(),
    capturedOnDate: isoDate(now),
    env: {
      tz: "UTC",
      locale: "C.utf8",
      term: "xterm-256color",
      home: "per-case fake HOME under the case temp root",
      dbDir: "per-case fake SUBSC_CLI_DB_DIR under the case temp root",
      inherited: "PATH only",
      runRoot: "os.tmpdir() — path-safety bases (home+tmp) must include the case root",
    },
    dateInjection: {
      env: "SUBTRACK_TODAY",
      value: isoDate(now),
      note:
        "Seed dates and today-relative outputs were computed on capturedOnDate (UTC). " +
        "The Rust test harness sets SUBTRACK_TODAY so its 'today' matches.",
    },
    fxMock: {
      preload: "capture/fxmock.mjs",
      rates: "inputs/fx-rates.json",
      intercepted: "https://open.er-api.com/v6/latest/USD → 200; all other URLs → 404",
    },
    normalizations: [
      "{WORK}   — per-case temp root (db/, cwd/, home/ live beneath it)",
      "{FIXTURES} — rust/fixtures directory",
      "{REPO}   — repository root",
      `{VERSION} — package version (${REFERENCE_VERSION}), boundary-safe`,
      "{DATETIME} — YYYY-MM-DD[ T]HH:MM:SS[.sss][Z|±HH:MM] (sqlite datetime etc.)",
      "{DATE}   — YYYY-MM-DD (applied after DATETIME)",
      "{STAMP}  — YYYYMMDD_HHMMSS in generated filenames and file paths (backups)",
      "Note: year-less short dates (\"Nov 8\") are stored literally — the Rust harness " +
        "reproduces them via SUBTRACK_TODAY injection.",
    ],
    stepSeparator: "<<<step N>>> (1-based) precedes each step's output in multi-step cases",
    fileOutputs:
      "New/changed files in the case cwd are stored as <group>/<id>.files/<path> " +
      "with mode bytes (text normalized, binary raw), gzip (stored raw; compare after " +
      "gunzip), or skip (.sha256 sidecars, .xlsx, raw .db — random IVs / implementation-defined bytes).",
    excluded,
    counts,
  }
  writeFileSync(path.join(outRoot, "manifest.json"), JSON.stringify(manifest, null, 2) + "\n")

  let diffs = []
  if (verify) {
    // 6. Verify against the committed goldens (still inside workRoot).
    diffs = await compareOutputs(fixturesDir, outRoot)
  } else {
    // 6. Sync results into the fixtures directory.
    rmSync(path.join(destRoot, "goldens"), { recursive: true, force: true })
    cpSync(path.join(outRoot, "goldens"), path.join(destRoot, "goldens"), { recursive: true })
    cpSync(path.join(outRoot, "cases.jsonl"), path.join(destRoot, "cases.jsonl"))
    cpSync(path.join(outRoot, "manifest.json"), path.join(destRoot, "manifest.json"))
  }

  rmSync(workRoot, { recursive: true, force: true })

  console.log(`captured ${entries.length} cases${verify ? " (temp)" : ` into ${destRoot}`}`)
  console.log(`groups: ${Object.entries(counts).map(([k, v]) => `${k}=${v}`).join(", ")}`)

  if (verify) {
    if (diffs.length) {
      console.error(`\nVERIFY FAILED — ${diffs.length} difference(s):`)
      for (const d of diffs) console.error(`  ${d}`)
      process.exit(1)
    }
    console.log("verify OK — fresh capture matches committed goldens")
  }
}

main().catch((err) => {
  console.error(err)
  process.exit(1)
})
