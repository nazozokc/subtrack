# Subtrack for Raycast

Browse your [subtrack](https://github.com/nazozokc/subtrack) subscriptions, upcoming
renewals, and monthly spend without leaving Raycast.

## Requirements

The subtrack CLI must be installed and on your `PATH`. See
[subtrack installation](https://github.com/nazozokc/subtrack#installation) — the
quickest route is `npm install -g subtrack`.

## Setup

No required preferences. Open the extension in Raycast and it works against your
default database at `~/.config/subtrack/subtrack.db`.

Two optional preferences:

- **subtrack binary** — only needed if subtrack is not on your `PATH`.
- **Database directory** — point at an alternate database if you use
  `SUBSC_CLI_DB_DIR` for a separate one.
- **Display currency** — convert all amounts to a single currency.

## Commands

| Command           | What it does                                      |
| ----------------- | ------------------------------------------------- |
| Subscriptions     | Browse everything, filter by status and sort      |
| Upcoming Renewals | What is about to be charged, over a chosen window |
| Monthly Spend     | Total monthly cost by currency and tag            |
| Budget            | Spending against your configured budget           |
| Add Subscription  | Record a new subscription                         |
| Edit Subscription | Change an existing subscription                   |

In **Subscriptions**, the dropdown next to the search field filters by status.
Sort order, direction, archived rows, and a single tag live in the **Filters** and
**Tag** submenus of the action panel. Search matches name, notes, and tags, so
`#streaming` finds anything tagged with it.

## Development

This is a standalone npm project and is deliberately **not** a member of the
pnpm workspace, so `npm` is used here while the rest of the repo uses `pnpm`.

```bash
npm install
npm run verify   # typecheck + lint + manifest validation + build
npm run dev      # load into Raycast (macOS only)
```

`npm run lint` covers ESLint and Prettier. `npm run lint:manifest` runs `ray lint`,
which validates the manifest schema, the icons, and the `author` handle against
raycast.com. Both are part of `npm run verify`.

## Ask Subtrack

The extension also registers subtrack's MCP server, so Raycast AI can answer
questions about your subscriptions directly. Try `Ask Subtrack` in the root
search, or `@subtrack` inside a Quick AI prompt.

This uses whatever `subtrack` resolves to on your `PATH`, so it does not pick up
the **Database directory** preference — it reads your default database.

## Privacy

Everything runs locally. The extension shells out to the subtrack CLI on your
machine and talks to no network service. The one exception is currency
conversion, which uses the `open.er-api.com` exchange rate API — only when you
set a **Display currency**.
