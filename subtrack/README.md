# subsc-cli

A CLI tool to manage your subscription services from the terminal.

## Features

- List all subscriptions in a formatted table
- Add a subscription interactively (name, price, currency, cycle, tags)
- Delete a subscription by ID

## Usage

```bash
# List all subscriptions
bunx subsc-cli list

# Add a new subscription (interactive prompts)
bunx subsc-cli add

# Delete a subscription by ID
bunx subsc-cli delete <id>

# List tags subscriptions
bunx subsc-cli tags <tags>
```

> [!TIP]
> The binary name is `subsc-cli`. Run via `bunx subsc-cli <command>` after `bun install`.

## Data

Subscriptions are stored in `~/.config/subscription-cli/subscription.json`.

## Development

```bash
# Install dependencies
bun install

# Run directly
bun run src/index.ts
```

## Install

```bash
bun install
```

Then link globally if you want:

```bash
bun link
```

Now you can use `sb` directly from anywhere.
