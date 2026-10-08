// Network mock preloaded via `node --import` during golden capture.
//
// Replaces globalThis.fetch so every HTTP call the CLI makes is
// deterministic and offline:
//   - https://open.er-api.com/*   → 200 with inputs/fx-rates.json
//   - anything else               → 404 (pricing/webhook endpoints)
//
// The same rates file is used by the Rust test harness, so FX-converted
// outputs are reproducible on both sides.

import { readFileSync } from "node:fs"
import path from "node:path"
import { fileURLToPath } from "node:url"

const here = path.dirname(fileURLToPath(import.meta.url))
const rates = JSON.parse(
  readFileSync(path.join(here, "..", "inputs", "fx-rates.json"), "utf8"),
)

globalThis.fetch = async (input) => {
  const url = String(typeof input === "object" && input?.url ? input.url : input)
  if (url.includes("open.er-api.com")) {
    return new Response(JSON.stringify(rates), {
      status: 200,
      statusText: "OK",
      headers: { "content-type": "application/json" },
    })
  }
  return new Response("mock: not found", {
    status: 404,
    statusText: "Not Found",
    headers: { "content-type": "text/plain" },
  })
}
