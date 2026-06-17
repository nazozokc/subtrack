<script lang="ts">
  import { base } from "$app/paths"
</script>

<svelte:head>
  <title>Data &amp; Storage — subtrack</title>
  <meta name="description" content="How subtrack stores data, database location, backup and restore." />
</svelte:head>

<div class="docs">
  <h1>Data &amp; Storage</h1>

  <h2>Database location</h2>
  <p>
    All subscription data is stored in a single SQLite database file at:
  </p>
  <pre><code>~/.config/subtrack/subtrack.db</code></pre>
  <p>
    The database is created automatically on first use. No database server or
    configuration is required.
  </p>

  <h2>Database structure</h2>
  <p>Three tables with a many-to-many relationship:</p>

  <pre><code>subscriptions
├── id        INTEGER PRIMARY KEY AUTOINCREMENT
├── name      TEXT NOT NULL
├── price     INTEGER NOT NULL
├── currency  TEXT NOT NULL
└── cycle     TEXT NOT NULL

tags
├── id   INTEGER PRIMARY KEY AUTOINCREMENT
└── name TEXT NOT NULL UNIQUE

subscription_tags
├── subscription_id  INTEGER NOT NULL (FK → subscriptions.id)
└── tag_id           INTEGER NOT NULL (FK → tags.id)
│                    PRIMARY KEY (subscription_id, tag_id)</code></pre>

  <p>
    Deleting a subscription automatically removes its tag associations via
    <code>ON DELETE CASCADE</code>. Orphaned tags (with no subscriptions) are
    not automatically cleaned up.
  </p>

  <h2>Prices are stored as integers</h2>
  <p>
    Prices are stored as whole numbers (integers) in the database. This avoids
    floating-point precision issues. For display, prices are formatted with the
    appropriate currency symbol and decimal places using
    <code>Intl.NumberFormat</code>.
  </p>

  <h2>Backup</h2>
  <p>
    Use the <code>backup</code> command to create a timestamped copy of your
    database:
  </p>
  <pre><code>subtrack backup ~/backups
# Creates: ~/backups/subtrack_20260617_143000.db</code></pre>
  <p>
    Backups use exclusive file creation, so they will never overwrite an existing
    file. See the <a href={base + "/commands"}>Commands</a> reference for full
    details.
  </p>

  <h2>Restore from backup</h2>
  <p>To restore from a backup, simply copy the backup file to the database location:</p>
  <pre><code># Stop subtrack (close all running instances)
cp ~/backups/subtrack_20260617_143000.db ~/.config/subtrack/subtrack.db</code></pre>
  <p>
    Make sure subtrack is not running when you restore, as changes are written to
    the database file on every command.
  </p>

  <h2>Custom database directory</h2>
  <p>
    Override the default <code>~/.config/subtrack</code> directory with the
    <code>SUBSC_CLI_DB_DIR</code> environment variable:
  </p>
  <pre><code>SUBSC_CLI_DB_DIR=/path/to/custom/dir subtrack list</code></pre>
  <p>
    See <a href={base + "/configuration"}>Configuration</a> for more details.
  </p>
</div>
