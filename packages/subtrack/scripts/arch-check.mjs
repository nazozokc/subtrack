#!/usr/bin/env node
/**
 * Layer boundary checker.
 *
 * subtrack keeps its domain logic pure and pushes persistence behind the
 * `src/application/` ports. That layering is easy to state and easy to erode:
 * a single `import { getSubscriptions } from "./db.ts"` in a handler quietly
 * reintroduces the coupling the ports exist to prevent. There is no ESLint in
 * this project, so this script enforces the rule instead.
 *
 * Design: a *ratchet*. Known violations are recorded in the baseline file and
 * are reported but tolerated, so the checker has teeth from day one (any NEW
 * violation fails) while existing debt is retired one module at a time.
 *
 *   node scripts/arch-check.mjs           # check (CI); exits 1 on new violations
 *   node scripts/arch-check.mjs --report  # list every violation, still exit 0
 *   node scripts/arch-check.mjs --update  # rewrite the baseline file
 *
 * Uses Node built-ins only, matching the project's zero-dependency stance.
 */

import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs"
import { dirname, relative, resolve, sep } from "node:path"
import { fileURLToPath } from "node:url"

const here = dirname(fileURLToPath(import.meta.url))
// Overridable so the checker can be pointed at a fixture tree by its tests;
// production runs scan the real `src`.
const srcRoot = process.env.ARCH_CHECK_SRC ?? resolve(here, "..", "src")
const baselineFile = resolve(here, "arch-check-baseline.txt")

/**
 * Files that may reach into `src/db/` without going through a port. Every entry
 * is justified by an actual import of `db.ts` / `db/*`; keep the list free of
 * speculative entries so it cannot silently widen the rule.
 *
 * The `*-scanner.ts` modules are deliberately absent: they open *other* tools'
 * SQLite files read-only via their own `DatabaseSync` handle and never touch
 * this app's database layer.
 */
const DB_ALLOWED = new Set([
  "db.ts", // the re-export barrel
  "application/repositories.ts", // the adapter that is supposed to import db
  "index.ts", // entry point: owns the process and flushes on SIGINT/SIGTERM
  "backup.ts", // operates on the database file itself
  "cleanup.ts",
  "diagnostics.ts",
  "maintenance.ts",
  "optimize.ts",
  "mcp/handlers.ts", // alternate transport, shares the handlers
])

/** Layer prefixes whose members must not depend on the listed targets. */
const FORBIDDEN_BY_LAYER = [
  {
    layer: "domain",
    matches: (p) => p === "domain" || p.startsWith("domain/"),
    forbid: ["db", "prompts", "display", "menu", "commands", "presentation", "mcp"],
    reason: "domain logic must stay pure — no persistence, prompts, or rendering",
  },
  {
    layer: "application",
    matches: (p) => p === "application" || p.startsWith("application/"),
    forbid: ["display", "prompts", "commands", "menu", "mcp"],
    reason: "application services orchestrate ports; they must not reach the UI",
  },
  {
    layer: "presentation",
    matches: (p) => p === "presentation" || p.startsWith("presentation/"),
    forbid: ["db"],
    reason: "presentation formats data; it must not query the database",
  },
]

// ── scanner ──────────────────────────────────────────────────────────────

/**
 * Extract relative module specifiers with their line numbers.
 *
 * A plain regex over the source is not good enough: a doc comment or a template
 * literal can mention `from "./db.ts"` and produce a phantom violation, so the
 * source is tokenised first and only real string literals are considered.
 */
function scanImports(source) {
  const found = []
  let line = 1
  let i = 0
  let prevWord = "" // last identifier, for `from` detection
  let prev = "" // last significant character

  const keywordBefore = () => {
    const m = /([A-Za-z_$][\w$]*)\s*$/.exec(source.slice(0, i))
    return m ? m[1] : ""
  }

  while (i < source.length) {
    const c = source[i]
    const next = source[i + 1]

    if (c === "\n") {
      line++
      i++
      continue
    }

    // Comments
    if (c === "/" && next === "/") {
      while (i < source.length && source[i] !== "\n") i++
      continue
    }
    if (c === "/" && next === "*") {
      i += 2
      while (i < source.length && !(source[i] === "*" && source[i + 1] === "/")) {
        if (source[i] === "\n") line++
        i++
      }
      i += 2
      continue
    }

    // Strings
    if (c === '"' || c === "'") {
      // Context must be sampled *before* consuming the literal, otherwise the
      // slice includes its contents and the keyword is misidentified.
      const kw = keywordBefore()
      const wasImport = prevWord === "import"
      const startLine = line
      let value = ""
      i++
      while (i < source.length && source[i] !== c) {
        if (source[i] === "\\") {
          value += source[i + 1]
          i += 2
          continue
        }
        if (source[i] === "\n") line++
        value += source[i]
        i++
      }
      i++
      // A string is a module specifier when `from` precedes it, or when it is
      // a bare side-effect import / dynamic import().
      if ((kw === "from" || wasImport) && value.startsWith(".")) {
        found.push({ spec: value, line: startLine })
      }
      prev = c
      prevWord = ""
      continue
    }

    // Template literals — skipped whole, so a `from "./x"` inside one is ignored.
    if (c === "`") {
      i++
      let depth = 0
      while (i < source.length) {
        if (source[i] === "\\") {
          i += 2
          continue
        }
        if (source[i] === "\n") line++
        if (depth === 0 && source[i] === "`") break
        if (source[i] === "$" && source[i + 1] === "{") {
          depth++
          i += 2
          continue
        }
        if (depth > 0 && source[i] === "}") {
          depth--
          i++
          continue
        }
        i++
      }
      i++
      prev = "`"
      prevWord = ""
      continue
    }

    // Regex literal — only where a value is expected.
    if (c === "/" && !/[\w)\]}]/.test(prev)) {
      i++
      let inClass = false
      while (i < source.length) {
        if (source[i] === "\\") {
          i += 2
          continue
        }
        if (source[i] === "\n") break
        if (source[i] === "[") inClass = true
        else if (source[i] === "]") inClass = false
        else if (source[i] === "/" && !inClass) break
        i++
      }
      i++
      prev = "/"
      prevWord = ""
      continue
    }

    if (/\s/.test(c)) {
      i++
      continue
    }
    if (/[A-Za-z_$]/.test(c)) {
      const m = /^[A-Za-z_$][\w$]*/.exec(source.slice(i))
      prevWord = m[0]
      prev = source[i + m[0].length - 1]
      i += m[0].length
      continue
    }
    prev = c
    // `import("./db.ts")` is a module specifier too. The paren must not erase
    // the keyword, otherwise a dynamic import is an invisible way to reach
    // past the layer rules. Any other punctuation does clear it.
    if (c !== "(" || prevWord !== "import") prevWord = ""
    i++
  }

  return found
}

// ── module graph ──────────────────────────────────────────────────────────

function listTsFiles(dir, acc = []) {
  for (const entry of readdirSync(dir)) {
    const p = resolve(dir, entry)
    if (statSync(p).isDirectory()) {
      if (entry !== "__tests__") listTsFiles(p, acc)
    } else if (entry.endsWith(".ts")) {
      acc.push(p)
    }
  }
  return acc
}

function resolveModule(spec, fromDir) {
  const base = resolve(fromDir, spec)
  for (const cand of [base, `${base}.ts`, resolve(base, "index.ts")]) {
    if (cand.startsWith(srcRoot) && statSyncSafe(cand)) return cand
  }
  return null
}

/** Report paths with forward slashes so rules and the baseline are portable. */
const toPosix = (p) => (sep === "/" ? p : p.split(sep).join("/"))

function statSyncSafe(p) {
  try {
    return statSync(p).isFile()
  } catch {
    return false
  }
}

/** @type {Map<string, {spec: string, line: number}[]>} */
const graph = new Map()
for (const file of listTsFiles(srcRoot)) {
  // Every comparison below is against a forward-slash path ("db/connection.ts"),
  // and `relative()` hands back backslashes on Windows. Normalising here keeps
  // the layer rules — and the baseline file — identical on every platform.
  const rel = toPosix(relative(srcRoot, file))
  graph.set(rel, scanImports(readFileSync(file, "utf8")))
}

// ── rules ────────────────────────────────────────────────────────────────

// One violation is identified by where it is and what it reaches, so a
// baseline entry accepts that edge and not every edge in the same file.
const violationKey = (file, rule, target) => `${file}|${rule}|${target}`

/** @type {{file: string, line: number, rule: string, target: string, reason: string}[]} */
const violations = []
const seen = new Set()

for (const [file, specs] of graph) {
  const targets = []
  for (const { spec, line } of specs) {
    const resolved = resolveModule(spec, dirname(resolve(srcRoot, file)))
    if (resolved) targets.push({ target: toPosix(relative(srcRoot, resolved)), line })
  }

  const push = (t, rule, reason) => {
    const key = violationKey(file, rule, t.target)
    if (seen.has(key)) return
    seen.add(key)
    violations.push({ file, line: t.line, rule, target: t.target, reason: `${reason} (imports ${t.target})` })
  }

  // Rule 1 — persistence is reached only through the application ports.
  if (!DB_ALLOWED.has(file) && !file.startsWith("db/")) {
    for (const t of targets) {
      if (t.target === "db.ts" || t.target.startsWith("db/")) {
        push(t, "no-direct-db", "use a port from src/application/ports.ts")
      }
    }
  }

  // Rules 2-4 — layer purity.
  for (const rule of FORBIDDEN_BY_LAYER) {
    if (!rule.matches(file)) continue
    for (const t of targets) {
      if (rule.forbid.some((f) => t.target === f || t.target === `${f}.ts` || t.target.startsWith(`${f}/`))) {
        push(t, `no-${rule.layer}-dep`, rule.reason)
      }
    }
  }
}

violations.sort((a, b) => a.file.localeCompare(b.file) || a.line - b.line)

// ── report ───────────────────────────────────────────────────────────────

const mode = process.argv[2]
const baseline = new Set(
  statSyncSafe(baselineFile)
    ? readFileSync(baselineFile, "utf8")
        .split("\n")
        .map((l) => l.trim())
        .filter((l) => l && !l.startsWith("#"))
    : [],
)

// A baseline keyed on file path alone would let a *new* violation appear inside
// an already-tracked file without failing. Key on file|rule|target, which is
// the same identity `push()` uses to dedupe, so accepting one edge does not
// silently accept another in the same file.
const keyOf = (v) => violationKey(v.file, v.rule, v.target)
const fresh = violations.filter((v) => !baseline.has(keyOf(v)))
const tracked = violations.filter((v) => baseline.has(keyOf(v)))

if (mode === "--update") {
  const keys = [...new Set(violations.map((v) => violationKey(v.file, v.rule, v.target)))].sort()
  const body = [
    "# Tracked layer-boundary violations, one `file|rule|target` per line.",
    "# Regenerate with `node scripts/arch-check.mjs --update`.",
    "# Remove a line once that import goes through the application ports.",
    ...keys,
    "",
  ].join("\n")
  writeFileSync(baselineFile, body)
  console.log(`arch-check: baseline written — ${keys.length} violation(s)`)
  process.exit(0)
}

if (mode === "--report") {
  console.log(`arch-check: ${violations.length} violation(s) across ${new Set(violations.map((v) => v.file)).size} file(s)\n`)
  for (const v of violations) console.log(`  ${v.file}:${v.line}  [${v.rule}] ${v.reason}`)
  process.exit(0)
}

for (const v of fresh) console.error(`  ${v.file}:${v.line}  [${v.rule}] ${v.reason}`)
if (fresh.length > 0) {
  console.error(`\narch-check: ${fresh.length} new layer violation(s). Route persistence through src/application/ports.ts.`)
  process.exit(1)
}
console.log(`arch-check: ok — ${tracked.length} tracked violation(s) remain in the baseline`)
