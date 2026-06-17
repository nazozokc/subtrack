<script lang="ts">
  import { base } from "$app/paths"
</script>

<svelte:head>
  <title>Installation — subtrack</title>
  <meta name="description" content="Install subtrack via npm, pnpm, or build from source." />
</svelte:head>

<div class="docs">
  <h1>Installation</h1>

  <h2>Requirements</h2>
  <p>
    <strong>subtrack</strong> requires <strong>Node.js 18 or later</strong>.
    It is a pure Node.js package and does not require any system-level dependencies.
  </p>

  <h2>Install via npm (recommended)</h2>
  <pre><code>npm install -g subtrack</code></pre>

  <h2>Install via pnpm</h2>
  <pre><code>pnpm add -g subtrack</code></pre>

  <h2>Install via bun</h2>
  <pre><code>bun add -g subtrack</code></pre>

  <h2>Run with npx (no install)</h2>
  <p>If you prefer not to install globally, use <code>npx</code>:</p>
  <pre><code>npx subtrack list</code></pre>
  <p>
    Note that <code>npx</code> will download the package on every first run,
    so global install is recommended for regular use.
  </p>

  <h2>Build from source</h2>
  <p>Clone the repository and build locally:</p>
  <pre><code>git clone https://github.com/nazozokc/subtrack.git
cd subtrack
pnpm install
pnpm build
pnpm link --global</code></pre>
  <p>
    This links the <code>subtrack</code> command to your global <code>node_modules</code>.
    Re-run <code>pnpm link --global</code> after pulling updates.
  </p>

  <div class="callout info">
    <strong>💡 Tip:</strong> Run <code>subtrack --help</code> after installation to verify everything works.
  </div>

  <h2>Verify installation</h2>
  <pre><code>subtrack --help</code></pre>
  <p>You should see the list of available commands.</p>
</div>
