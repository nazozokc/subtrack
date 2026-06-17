<svelte:head>
  <title>Development — subtrack</title>
  <meta name="description" content="How to set up, build, test, and contribute to subtrack." />
</svelte:head>

<div class="docs">
  <h1>Development</h1>

  <h2>Repository setup</h2>
  <pre><code>git clone https://github.com/nazozokc/subtrack.git
cd subtrack
pnpm install</code></pre>

  <h2>Nix devShell</h2>
  <p>
    If you use Nix, a devShell is available with all required tools:
  </p>
  <pre><code>nix develop</code></pre>
  <p>
    This provides <code>node</code>, <code>pnpm</code>, <code>typescript</code>,
    <code>typos</code>, and <code>nixfmt</code>.
  </p>

  <h2>Available commands</h2>
  <table>
    <thead>
      <tr>
        <th>Command</th>
        <th>Description</th>
      </tr>
    </thead>
    <tbody>
      <tr>
        <td><code>pnpm start</code></td>
        <td>Run subtrack directly from TypeScript source via <code>tsx</code></td>
      </tr>
      <tr>
        <td><code>pnpm build</code></td>
        <td>Build the CLI bundle to <code>dist/index.mjs</code> via <code>tsdown</code></td>
      </tr>
      <tr>
        <td><code>pnpm test</code></td>
        <td>Run tests with <code>vitest</code></td>
      </tr>
      <tr>
        <td><code>pnpm test:watch</code></td>
        <td>Run tests in watch mode</td>
      </tr>
      <tr>
        <td><code>pnpm lint:typos</code></td>
        <td>Check for spelling errors with <code>typos</code></td>
      </tr>
    </tbody>
  </table>

  <h2>Tech stack</h2>
  <table>
    <thead>
      <tr>
        <th>Category</th>
        <th>Choice</th>
      </tr>
    </thead>
    <tbody>
      <tr><td>Runtime</td><td>Node.js</td></tr>
      <tr><td>Language</td><td>TypeScript (strict mode, ESM)</td></tr>
      <tr><td>CLI framework</td><td><code>commander</code></td></tr>
      <tr><td>Interactive prompts</td><td><code>@inquirer/prompts</code></td></tr>
      <tr><td>Terminal output</td><td><code>consola</code>, <code>picocolors</code>, <code>cli-table3</code></td></tr>
      <tr><td>Database</td><td><code>sql.js</code> (SQLite via WASM)</td></tr>
      <tr><td>Exchange rates</td><td><a href="https://open.er-api.com">open.er-api.com</a></td></tr>
      <tr><td>Build tool</td><td><code>tsdown</code></td></tr>
      <tr><td>Test framework</td><td><code>vitest</code></td></tr>
      <tr><td>Package manager</td><td><code>pnpm</code></td></tr>
      <tr><td>Documentation</td><td>SvelteKit (this site)</td></tr>
    </tbody>
  </table>

  <h2>Contributing</h2>
  <p>
    See <a href="https://github.com/nazozokc/subtrack/blob/main/CONTRIBUTING.md">CONTRIBUTING.md</a>
    for branch policy, AI agent guidelines, and PR requirements.
  </p>

  <h3>Before opening a PR</h3>
  <pre><code>pnpm build
pnpm test</code></pre>
  <p>Make sure both pass and CI is green.</p>

  <h2>Project structure</h2>
  <pre><code>subtrack/
├── subtrack/         # CLI tool (TypeScript/ESM)
│   ├── src/
│   │   ├── index.ts      # Entry point, commander setup
│   │   ├── commands.ts   # Command handlers
│   │   ├── db.ts         # SQLite database layer
│   │   ├── display.ts    # Table rendering, currency conversion
│   │   └── prompts.ts    # Input validation, prompt helpers
│   └── dist/             # Built output
├── docs/             # Documentation site (SvelteKit)
├── flake.nix         # Nix devShell
└── pnpm-workspace.yaml</code></pre>
</div>
