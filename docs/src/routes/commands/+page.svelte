<script lang="ts">
  import { base } from "$app/paths"
</script>

<svelte:head>
  <title>Commands — subtrack</title>
  <meta name="description" content="Full reference for all subtrack CLI commands." />
</svelte:head>

<div class="docs">
  <h1>Commands</h1>
  <p>
    subtrack provides six commands. Most support both interactive and non-interactive modes.
  </p>

  <!-- ── list ──────────────────────────────────────────── -->

  <h2><code>list</code></h2>
  <p>
    Lists all subscriptions in a formatted table. Subscriptions are grouped by
    currency by default, with a subtotal row per group.
  </p>

  <table>
    <thead>
      <tr>
        <th>Option</th>
        <th>Description</th>
      </tr>
    </thead>
    <tbody>
      <tr>
        <td><code>-c, --currency &lt;C&gt;</code></td>
        <td>Convert all prices to the given currency using live exchange rates</td>
      </tr>
    </tbody>
  </table>

  <h3>Examples</h3>
  <pre><code># List all subscriptions (grouped by currency)
subtrack list

# Convert all prices to JPY
subtrack list --currency JPY</code></pre>

  <p>
    When <code>--currency</code> is used, all prices are converted to the target
    currency (fetched from <a href="https://open.er-api.com">open.er-api.com</a>)
    and displayed as a single group with a grand total.
  </p>

  <!-- ── add ─────────────────────────────────────────────── -->

  <h2><code>add</code></h2>
  <p>
    Adds a new subscription. Without flags, prompts for all fields interactively.
    Providing all flags skips prompts entirely (useful for scripts).
  </p>

  <table>
    <thead>
      <tr>
        <th>Option</th>
        <th>Description</th>
      </tr>
    </thead>
    <tbody>
      <tr>
        <td><code>--name &lt;name&gt;</code></td>
        <td>Subscription name (max 100 characters)</td>
      </tr>
      <tr>
        <td><code>--price &lt;price&gt;</code></td>
        <td>Payment amount — integer, non-negative, max 99,999,999</td>
      </tr>
      <tr>
        <td><code>--currency &lt;C&gt;</code></td>
        <td>Currency code. Supported: JPY, USD, EUR, GBP, AUD, CAD, KRW, CNY, SGD, HKD</td>
      </tr>
      <tr>
        <td><code>--cycle &lt;cycle&gt;</code></td>
        <td>Billing cycle. One of: weekly, bi-weekly, monthly, quarterly, semi-annual, yearly</td>
      </tr>
      <tr>
        <td><code>--tags &lt;tags&gt;</code></td>
        <td>Comma-separated tags (max 10 tags, each max 50 characters)</td>
      </tr>
    </tbody>
  </table>

  <h3>Examples</h3>
  <pre><code># Interactive mode
subtrack add

# Fully non-interactive (skips confirmation)
subtrack add \
  --name Spotify \
  --price 980 \
  --currency JPY \
  --cycle monthly \
  --tags music

# Partial flags — missing fields are prompted
subtrack add --name Netflix

# Tags with existing tag autocomplete
subtrack add --name "AWS" --price 50 --currency USD --cycle monthly</code></pre>

  <!-- ── delete ──────────────────────────────────────────── -->

  <h2><code>delete</code></h2>
  <p>
    Shows an interactive checkbox list of all subscriptions. Select one or more
    to delete. Confirmation is required before deletion.
  </p>

  <div class="callout warning">
    <strong>⚠ Note:</strong> The <code>delete</code> command is always interactive.
    There is no non-interactive mode.
  </div>

  <h3>Example</h3>
  <pre><code>subtrack delete</code></pre>

  <!-- ── payment ─────────────────────────────────────────── -->

  <h2><code>payment [period]</code></h2>
  <p>
    Calculates and displays how much you pay over a given billing period. All
    subscriptions are automatically converted to the target period based on their
    billing cycle.
  </p>

  <p>The <code>period</code> argument defaults to <code>monthly</code>. Valid values:</p>
  <table>
    <thead>
      <tr>
        <th>Period</th>
        <th>Alias</th>
      </tr>
    </thead>
    <tbody>
      <tr><td><code>weekly</code></td><td>per week</td></tr>
      <tr><td><code>bi-weekly</code></td><td>per two weeks</td></tr>
      <tr><td><code>monthly</code></td><td>per month (default)</td></tr>
      <tr><td><code>quarterly</code></td><td>per 3 months</td></tr>
      <tr><td><code>semi-annual</code></td><td>per 6 months</td></tr>
      <tr><td><code>yearly</code></td><td>per year</td></tr>
    </tbody>
  </table>

  <table>
    <thead>
      <tr>
        <th>Option</th>
        <th>Description</th>
      </tr>
    </thead>
    <tbody>
      <tr>
        <td><code>-c, --currency &lt;C&gt;</code></td>
        <td>Convert all prices to the given currency using live exchange rates</td>
      </tr>
    </tbody>
  </table>

  <h3>Examples</h3>
  <pre><code># Monthly total (default)
subtrack payment

# Yearly total
subtrack payment yearly

# Weekly total in JPY
subtrack payment weekly --currency JPY</code></pre>

  <p>
    When <code>--currency</code> is used, the total is displayed as a single amount
    in the target currency. Without it, totals are grouped by currency.
  </p>
  <p>
    If exchange rates cannot be fetched (e.g. offline), the command falls back to
    per-currency display without conversion.
  </p>

  <!-- ── tags ────────────────────────────────────────────── -->

  <h2><code>tags &lt;taglist...&gt;</code></h2>
  <p>
    Filters and displays subscriptions that have <strong>all</strong> specified tags
    (AND logic).
  </p>

  <h3>Examples</h3>
  <pre><code># Subscriptions tagged with "music"
subtrack tags music

# Subscriptions tagged with both "music" AND "video"
subtrack tags music video

# Subscriptions tagged with "entertainment", "video", and "kids"
subtrack tags entertainment video kids</code></pre>

  <!-- ── backup ──────────────────────────────────────────── -->

  <h2><code>backup &lt;destination&gt;</code></h2>
  <p>
    Creates a timestamped backup of the SQLite database in the specified directory.
    The backup filename follows the format <code>subtrack_YYYYMMDD_HHmmss.db</code>.
  </p>

  <p>
    If the destination does not exist or is not a directory, the command exits with
    an error. The backup will not overwrite existing files (exclusive create).
  </p>

  <h3>Examples</h3>
  <pre><code># Backup to current directory
subtrack backup .

# Backup to ~/backups
subtrack backup ~/backups</code></pre>
</div>
