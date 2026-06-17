<svelte:head>
  <title>FAQ — subtrack</title>
  <meta name="description" content="Frequently asked questions and troubleshooting for subtrack." />
</svelte:head>

<div class="docs">
  <h1>FAQ &amp; Troubleshooting</h1>

  <!-- ── Q1 ──────────────────────────────────────────── -->

  <h2>I lost my database. Can I recover it?</h2>
  <p>
    If you have a backup (created with <code>subtrack backup</code>), copy it
    back to <code>~/.config/subtrack/subtrack.db</code>. Without a backup, the
    data cannot be recovered — the database is stored locally only.
  </p>
  <p>
    <strong>Recommendation:</strong> Set up regular automated backups via cron
    or Task Scheduler. See the <a href="/data">Data &amp; Storage</a> page
    for details.
  </p>

  <!-- ── Q2 ──────────────────────────────────────────── -->

  <h2>Can I add support for more currencies?</h2>
  <p>
    The supported currencies are defined in <code>src/prompts.ts</code> as
    <code>CURRENCY_CHOICES</code>. To add a new currency:
  </p>
  <ol>
    <li>Add it to the <code>CURRENCY_CHOICES</code> array</li>
    <li>Add it to the <code>Currency</code> type in <code>src/db.ts</code></li>
    <li>Verify that <a href="https://open.er-api.com">open.er-api.com</a> supports the currency</li>
  </ol>
  <p>
    Pull requests for additional currencies are welcome!
  </p>

  <!-- ── Q3 ──────────────────────────────────────────── -->

  <h2>Does subtrack work offline?</h2>
  <p>
    Yes. Listing, adding, deleting, and filtering all work fully offline. The
    only feature that requires internet is <code>--currency</code> conversion,
    which fetches live exchange rates from <a href="https://open.er-api.com">open.er-api.com</a>.
  </p>
  <p>
    When offline, <code>--currency</code> falls back to per-currency display
    without conversion.
  </p>

  <!-- ── Q4 ──────────────────────────────────────────── -->

  <h2>How do I restore from a backup?</h2>
  <p>
    Copy the backup file to the database location:
  </p>
  <pre><code>cp ~/backups/subtrack_20260617_143000.db ~/.config/subtrack/subtrack.db</code></pre>
  <p>
    Ensure no subtrack processes are running during the restore. The database
    is flushed to disk after each command.
  </p>

  <!-- ── Q5 ──────────────────────────────────────────── -->

  <h2>Is my data sent anywhere?</h2>
  <p>
    <strong>No.</strong> All data is stored locally in a SQLite file on your
    machine. There are no accounts, no telemetry, and no cloud sync. The only
    external request subtrack makes is to <a href="https://open.er-api.com">
    open.er-api.com</a> for currency exchange rates — and only when you use
    the <code>--currency</code> flag.
  </p>

  <!-- ── Q6 ──────────────────────────────────────────── -->

  <h2>Can I use subtrack in Docker or CI?</h2>
  <p>
    Yes. subtrack is a standard Node.js CLI tool and works in any environment
    with Node.js 18+. For Docker:
  </p>
  <pre><code>FROM node:22-alpine
RUN npm install -g subtrack
CMD ["subtrack", "list"]</code></pre>
  <p>
    Use <code>SUBSC_CLI_DB_DIR</code> to control where the database is stored
    in containerized environments.
  </p>

  <!-- ── Q7 ──────────────────────────────────────────── -->

  <h2>How do I update subtrack?</h2>
  <pre><code>npm update -g subtrack</code></pre>
  <p>
    Or, if installed via pnpm:
  </p>
  <pre><code>pnpm update -g subtrack</code></pre>
  <p>
    Your database will be preserved — updates only affect the CLI code, not
    your data.
  </p>
</div>
