<script lang="ts">
  import "../app.css"
  let { children } = $props()
</script>

<header>
  <nav>
    <a href="/subtrack" class="logo">subtrack</a>
    <div class="links">
      <a href="https://github.com/nazozokc/subtrack">GitHub</a>
      <a href="https://www.npmjs.com/package/subtrack">npm</a>
    </div>
  </nav>
</header>

<main>
  {@render children()}
</main>

<footer>
  <p>MIT &mdash; <a href="https://github.com/nazozokc/subtrack">nazozokc/subtrack</a></p>
</footer>

<style>
  header {
    border-bottom: 1px solid var(--border);
    padding: 1rem 2rem;
  }

  nav {
    max-width: 960px;
    margin: 0 auto;
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .logo {
    font-family: var(--font-mono);
    font-weight: 700;
    font-size: 1.2rem;
    color: var(--fg);
  }

  .logo:hover {
    color: var(--accent);
  }

  .links {
    display: flex;
    gap: 1.5rem;
  }

  .links a {
    font-size: 0.9rem;
  }

  main {
    flex: 1;
    max-width: 960px;
    width: 100%;
    margin: 0 auto;
    padding: 2rem;
  }

  footer {
    border-top: 1px solid var(--border);
    padding: 1.5rem 2rem;
    text-align: center;
    font-size: 0.85rem;
    color: var(--muted);
  }
</style>
