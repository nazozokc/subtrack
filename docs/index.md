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
      text: Get Started
      link: /installation
    - theme: alt
      text: Commands
      link: /commands

features:
  - title: CLI-first
    details: Full CLI for scripting and automation, plus an interactive menu for exploring your subscriptions.
  - title: Local SQLite
    details: All data is stored locally. No servers, no accounts, no telemetry.
  - title: Currency Conversion
    details: Live exchange rates from open.er-api.com. View totals in any of 37 commonly used currencies, or pass any valid ISO 4217 code.
---

## Quick Start

```bash
# List all subscriptions
subtrack list

# Add a subscription
subtrack add --name Netflix --price 1999 --currency USD --cycle monthly

# Show monthly payment total
subtrack payment

# Show upcoming bills
subtrack upcoming

# Run `subtrack` alone to open the interactive menu
subtrack
```

## Where to go next

- [Install subtrack](/installation) with npm, pnpm, or from source.
- Browse the [complete command reference](/commands), including scripting and JSON output examples.
- Learn about [configuration, encryption, and backups](/configuration) and [data storage](/data).
- If you use an AI assistant, see the [MCP integration guide](/mcp).
