<svelte:head>
  <title>Configuration — subtrack</title>
  <meta name="description" content="Environment variables and configuration options for subtrack." />
</svelte:head>

<div class="docs">
  <h1>Configuration</h1>
  <p>
    subtrack follows a <strong>zero-configuration</strong> philosophy. It works
    out of the box with sensible defaults. The only configurable option is
    the database directory.
  </p>

  <h2>Environment variables</h2>

  <table>
    <thead>
      <tr>
        <th>Variable</th>
        <th>Description</th>
        <th>Default</th>
      </tr>
    </thead>
    <tbody>
      <tr>
        <td><code>SUBSC_CLI_DB_DIR</code></td>
        <td>Override the database directory</td>
        <td><code>~/.config/subtrack</code></td>
      </tr>
    </tbody>
  </table>

  <h3>Example usage</h3>
  <pre><code># Use a custom directory for the database
export SUBSC_CLI_DB_DIR=~/project/subtrack-data
subtrack list

# Or set it per-command
SUBSC_CLI_DB_DIR=/tmp/test-db subtrack add \
  --name Spotify \
  --price 980 \
  --currency JPY \
  --cycle monthly</code></pre>

  <div class="callout info">
    <strong>💡 Tip:</strong> Changing <code>SUBSC_CLI_DB_DIR</code> lets you
    maintain separate databases — useful for testing or multi-profile setups.
  </div>

  <h2>No config file</h2>
  <p>
    subtrack does not use configuration files (<code>.subtrackrc</code>,
    <code>subtrack.json</code>, etc.). All settings are controlled via
    environment variables or CLI flags. This keeps the tool simple and
    predictable.
  </p>

  <h2>Currency &amp; cycle choices</h2>

  <h3>Supported currencies (10)</h3>
  <pre><code>JPY  USD  EUR  GBP  AUD  CAD  KRW  CNY  SGD  HKD</code></pre>

  <h3>Supported billing cycles (6)</h3>
  <pre><code>weekly  bi-weekly  monthly  quarterly  semi-annual  yearly</code></pre>
</div>
