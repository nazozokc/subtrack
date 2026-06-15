import adapter from "@sveltejs/adapter-static"

/** @type {import("@sveltejs/kit").Config} */
const config = {
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
