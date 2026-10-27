---
layout: home

hero:
  name: subtrack
  text: Manage your subscription services from the terminal.
  tagline: Local-first subscription manager for the CLI
  image:
    src: /subtrack-cli-logo.png
    alt: subtrack
  actions:
    - theme: brand
      text: Get started
      link: /installation
    - theme: alt
      text: Browse commands
      link: /commands

features:
  - title: CLI-first
    details: Every command runs from flags and safe defaults. Script it, pipe it, or use the interactive prompts — no menus required.
  - title: Local & private
    details: All data stays on your machine in a single SQLite file. No servers, no accounts, no telemetry.
  - title: Multi-currency
    details: Track subscriptions in any ISO 4217 currency and view totals in 37 common currencies with live exchange rates.
  - title: Automation ready
    details: JSON output, predictable exit codes, and a built-in MCP server for AI assistants and other tools.
---

## Quick start

Install the CLI, then add your first subscription:

```bash
# Install globally
npm install -g @subtrack-org/subtrack

# Add a subscription
subtrack add --name Netflix --price 14.99 --currency USD --cycle monthly --status active

# See what you pay each month
subtrack payment
```

New to subtrack? Follow the [installation guide](./installation), then explore the
[usage guides](./guides) for common workflows.

## Explore the docs

<div class="doc-grid">
  <a class="doc-card" href="./installation">
    <strong>Installation</strong>
    <span>npm, pnpm, bun, npx, or build the Rust binary from source.</span>
  </a>
  <a class="doc-card" href="./guides">
    <strong>Usage guides</strong>
    <span>Step-by-step workflows for subscriptions, budgets, tags, backups, and more.</span>
  </a>
  <a class="doc-card" href="./commands">
    <strong>Command reference</strong>
    <span>All 50 commands grouped by task, with every flag and example.</span>
  </a>
  <a class="doc-card" href="./configuration">
    <strong>Configuration</strong>
    <span>Environment variables, encryption, themes, and runtime settings.</span>
  </a>
  <a class="doc-card" href="./data">
    <strong>Data &amp; storage</strong>
    <span>Where the database lives, how backups work, and how to restore.</span>
  </a>
  <a class="doc-card" href="./mcp">
    <strong>MCP server</strong>
    <span>Connect subtrack to Claude Desktop, Cursor, Windsurf, and other AI tools.</span>
  </a>
</div>
