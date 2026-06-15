<script lang="ts">
  import { dev } from "$app/environment"
</script>

<svelte:head>
  <title>subtrack — CLI subscription manager</title>
  <meta name="description" content="Manage your subscription services from the terminal." />
</svelte:head>

<section class="hero">
  <h1>subtrack</h1>
  <p class="tagline">Manage your subscription services from&nbsp;the&nbsp;terminal.</p>
  <div class="install">
    <pre>npm install -g subtrack</pre>
  </div>
</section>

<section class="features">
  <div class="feature">
    <h2>🕹️ TUI &amp; CLI</h2>
    <p>Interactive terminal UI for browsing subscriptions, plus a full CLI for scripting and automation.</p>
  </div>
  <div class="feature">
    <h2>💾 Local SQLite</h2>
    <p>All data is stored locally. No servers, no accounts, no telemetry.</p>
  </div>
  <div class="feature">
    <h2>🔔 Notifications</h2>
    <p>Get reminded when subscriptions are about to renew. Never get surprised by a bill again.</p>
  </div>
</section>

<section class="usage">
  <h2>Quick Start</h2>
  <pre># List all subscriptions
subtrack list

# Add a subscription
subtrack add --name Netflix --price 19.99 --period monthly

# Check upcoming renewals
subtrack upcoming

# Open interactive TUI
subtrack tui</pre>
</section>

{#if !dev}
  <p class="outdated-note">
    This page may be outdated. Check the <a href="https://github.com/nazozokc/subtrack">repository</a> for the latest documentation.
  </p>
{/if}

<style>
  .hero {
    text-align: center;
    padding: 4rem 0 3rem;
  }

  h1 {
    font-family: var(--font-mono);
    font-size: 3rem;
    font-weight: 800;
    letter-spacing: -0.03em;
    background: linear-gradient(135deg, var(--accent), #c084fc);
    background-clip: text;
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
  }

  .tagline {
    margin-top: 0.75rem;
    font-size: 1.15rem;
    color: var(--muted);
  }

  .install {
    margin-top: 1.5rem;
  }

  .install pre {
    display: inline-block;
    padding: 0.6rem 1.2rem;
    font-size: 0.9rem;
  }

  .features {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: 1.5rem;
    padding: 2rem 0;
  }

  .feature {
    background: #141414;
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 1.5rem;
  }

  .feature h2 {
    font-size: 1.1rem;
    margin-bottom: 0.5rem;
  }

  .feature p {
    font-size: 0.9rem;
    color: var(--muted);
    line-height: 1.5;
  }

  .usage {
    padding: 2rem 0;
  }

  .usage h2 {
    font-size: 1.25rem;
    margin-bottom: 1rem;
  }

  .outdated-note {
    text-align: center;
    font-size: 0.8rem;
    color: var(--muted);
    margin-top: 2rem;
  }
</style>
