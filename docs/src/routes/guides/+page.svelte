<script lang="ts">
  import { base } from "$app/paths"
</script>

<svelte:head>
  <title>Usage Guides — subtrack</title>
  <meta name="description" content="Practical usage examples and workflows for subtrack." />
</svelte:head>

<div class="docs">
  <h1>Usage Guides</h1>
  <p>
    Practical workflows for managing your subscriptions with subtrack.
  </p>

  <!-- ── Guide 1 ────────────────────────────────────────── -->

  <h2>First-time setup &amp; adding subscriptions</h2>
  <p>
    After installation, start by adding your subscriptions. The interactive mode
    guides you through each field:
  </p>
  <pre><code>subtrack add</code></pre>
  <p>
    You'll be prompted for: name, price, currency, billing cycle, and tags.
    Once added, verify with:
  </p>
  <pre><code>subtrack list</code></pre>

  <h3>Example: Adding a Netflix subscription</h3>
  <pre><code>subtrack add \
  --name "Netflix Premium" \
  --price 1980 \
  --currency JPY \
  --cycle monthly \
  --tags "video,entertainment"</code></pre>

  <!-- ── Guide 2 ────────────────────────────────────────── -->

  <h2>Tag-based organization</h2>
  <p>
    Tags are a powerful way to categorize subscriptions. Examples:
  </p>
  <ul>
    <li><strong>By category:</strong> <code>music</code>, <code>video</code>, <code>cloud</code>, <code>productivity</code></li>
    <li><strong>By priority:</strong> <code>essential</code>, <code>nice-to-have</code></li>
    <li><strong>By payment method:</strong> <code>credit-card</code>, <code>paypal</code></li>
    <li><strong>By usage:</strong> <code>personal</code>, <code>work</code>, <code>family</code></li>
  </ul>

  <p>Filter by tags:</p>
  <pre><code># Find all music-related subscriptions
subtrack tags music

# Find subscriptions used for both work and personal
subtrack tags work personal</code></pre>

  <p>
    Tags use AND logic — only subscriptions matching <strong>all</strong> specified
    tags are shown.
  </p>

  <!-- ── Guide 3 ────────────────────────────────────────── -->

  <h2>Understanding your spending</h2>
  <p>
    Use <code>subtrack payment</code> to see your total spending across different
    periods:
  </p>
  <pre><code># How much per month?
subtrack payment

# How much per year?
subtrack payment yearly

# Weekly cost in USD
subtrack payment weekly --currency USD</code></pre>
  <p>
    <code>payment</code> automatically converts all billing cycles to the target
    period. A yearly subscription will be divided into monthly cost, and a weekly
    subscription will be multiplied accordingly.
  </p>

  <!-- ── Guide 4 ────────────────────────────────────────── -->

  <h2>Managing multi-currency subscriptions</h2>
  <p>
    If you have subscriptions in different currencies (e.g., JPY for local services
    and USD for international ones), subtrack handles this natively:
  </p>
  <pre><code># See everything in your local currency
subtrack list --currency JPY

# Compare spending across currencies
subtrack list</code></pre>
  <p>
    Without <code>--currency</code>, subscriptions are grouped by their original
    currency with per-group subtotals.
  </p>

  <!-- ── Guide 5 ────────────────────────────────────────── -->

  <h2>Regular backups</h2>
  <p>
    Set up a cron job (or Task Scheduler on Windows) for automatic backups:
  </p>
  <pre><code># Example cron: daily backup at 3 AM
0 3 * * * subtrack backup ~/subtrack-backups</code></pre>
  <p>
    Backups are timestamped and will never overwrite previous files. See
    <a href={base + "/data"}>Data &amp; Storage</a> for restore instructions.
  </p>
</div>
