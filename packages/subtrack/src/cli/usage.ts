/**
 * Header/banner suppression decision.
 *
 * The header is human-facing decoration. Two output modes need stdout to stay
 * machine-parseable, so it must not be printed:
 *
 * - `mcp` speaks JSON-RPC on stdout, so any banner corrupts the frame stream.
 * - `--json` (or its `-j` short form) emits a single JSON document, so a banner
 *   before it breaks `JSON.parse`. Every JSON-capable command relies on this,
 *   which is what makes `subtrack <cmd> --json` safe to consume from other
 *   programs.
 *
 * Tokenizing rather than scanning raw argv is deliberate: it collapses short
 * groups (`-nj`) and `--flag=value` forms, so `list --json=true` is recognized
 * instead of slipping past a literal `--json` check. `j` is the short form of
 * `json` on every command that offers it, and is unused by any other flag.
 */
import { tokenize } from "./parser.ts"

export function shouldSuppressUsage(argv: readonly string[]): boolean {
  if (argv[0] === "mcp") return true
  return tokenize([...argv]).some(
    (t) => t.kind === "option" && (t.name === "json" || t.name === "j"),
  )
}
