import { defineConfig } from "vitepress"

export default defineConfig({
  lang: "en-US",
  title: "subtrack",
  description:
    "Manage your subscription services from the terminal. Local-first, scriptable, and built for the CLI.",
  base: "/subtrack/",
  head: [
    [
      "link",
      {
        rel: "icon",
        type: "image/png",
        href: "/subtrack/subtrack-cli-logo.png",
      },
    ],
    [
      "meta",
      {
        name: "theme-color",
        content: "#ffffff",
        media: "(prefers-color-scheme: light)",
      },
    ],
    [
      "meta",
      {
        name: "theme-color",
        content: "#1b1b1f",
        media: "(prefers-color-scheme: dark)",
      },
    ],
    ["meta", { property: "og:type", content: "website" }],
    ["meta", { property: "og:site_name", content: "subtrack" }],
    [
      "meta",
      {
        property: "og:description",
        content:
          "Manage your subscription services from the terminal. Local-first, scriptable, and built for the CLI.",
      },
    ],
    ["meta", { name: "twitter:card", content: "summary" }],
  ],
  themeConfig: {
    logo: "/subtrack-cli-logo.png",
    siteTitle: "subtrack",
    search: {
      provider: "local",
    },
    nav: [
      {
        text: "Guide",
        activeMatch: "^/(installation|guides)",
        items: [
          { text: "Overview", link: "/" },
          { text: "Installation", link: "/installation" },
          { text: "Usage guides", link: "/guides" },
        ],
      },
      {
        text: "Reference",
        activeMatch: "^/(commands|configuration|data|mcp)",
        items: [
          { text: "All commands", link: "/commands" },
          { text: "Configuration", link: "/configuration" },
          { text: "Data & storage", link: "/data" },
          { text: "MCP server", link: "/mcp" },
        ],
      },
      {
        text: "Project",
        activeMatch: "^/(development|faq)",
        items: [
          { text: "Development", link: "/development" },
          { text: "FAQ", link: "/faq" },
        ],
      },
      { text: "GitHub", link: "https://github.com/nazozokc/subtrack" },
      {
        text: "npm",
        link: "https://www.npmjs.com/package/@subtrack-org/subtrack",
      },
    ],
    sidebar: [
      {
        text: "Get started",
        items: [
          { text: "Overview", link: "/" },
          { text: "Installation", link: "/installation" },
        ],
      },
      {
        text: "Usage",
        items: [{ text: "Usage guides", link: "/guides" }],
      },
      {
        text: "Reference",
        items: [
          { text: "Commands", link: "/commands" },
          { text: "Configuration", link: "/configuration" },
          { text: "Data & storage", link: "/data" },
          { text: "MCP server", link: "/mcp" },
        ],
      },
      {
        text: "Project",
        items: [
          { text: "Development", link: "/development" },
          { text: "FAQ", link: "/faq" },
        ],
      },
    ],
    outline: {
      level: [2, 3],
      label: "On this page",
    },
    editLink: {
      pattern: "https://github.com/nazozokc/subtrack/edit/main/docs/:path",
      text: "Edit this page on GitHub",
    },
    lastUpdated: {
      text: "Last updated",
      formatOptions: { dateStyle: "medium" },
    },
    docFooter: {
      prev: "Previous page",
      next: "Next page",
    },
    footer: {
      message: "Released under the MIT License.",
      copyright: "Copyright © 2025-present nazozokc",
    },
  },
})
