import adapter from "@sveltejs/adapter-static"
import { mdsvex } from "mdsvex"
import path from "node:path"
import { fileURLToPath } from "node:url"

const __dirname = path.dirname(fileURLToPath(import.meta.url))

/** @type {import("@sveltejs/kit").Config} */
const config = {
  extensions: [".svelte", ".md"],
  preprocess: mdsvex({
    extensions: [".md"],
    layout: path.resolve(__dirname, "src/lib/layouts/MarkdownLayout.svelte"),
  }),
  kit: {
    adapter: adapter({
      fallback: "index.html",
      precompress: false,
    }),
    paths: {
      base: process.env.GITHUB_PAGES === "true" ? "/subtrack" : "",
    },
  },
}

export default config
