# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in subtrack, please report it privately.

**Do not** report security vulnerabilities via public GitHub issues.

Instead, send a description of the issue (including steps to reproduce, affected versions, and any relevant code context) to one of the following:

- Open a **private security advisory** at: https://github.com/nazozokc/subtrack/security/advisories/new
- Email: **nazozokc@icloud.com**

You should receive a response within **48 hours**. If you don't hear back, follow up via the advisory thread.

## Scope

This policy covers the `subtrack` npm package and the `subtrack-monorepo` at https://github.com/nazozokc/subtrack.

The following are **out of scope**:
- The documentation site under `docs/`
- Third-party dependencies (report those to the respective maintainers)
- Theoretical vulnerabilities without a practical exploit path

## What to Expect

- I will acknowledge receipt of your report within 48 hours
- I will investigate and provide a timeline for a fix
- Once a fix is ready, I will release a patch and credit you in the release notes (unless you prefer to remain anonymous)

## Supported Versions

| Version | Supported |
|---------|-----------|
| >= 4.x  | ✅ Active |
| < 4.x   | ❌ No longer supported |

## Supply Chain Security

subtrack takes supply chain security seriously:

- **Provenance attestation**: All npm releases use `pnpm publish --provenance` (SLSA Level 1+)
- **Dependency review**: Every pull request is scanned for new vulnerabilities via GitHub's dependency review action
- **Renovate bot**: Dependencies are updated with a 7-day minimum release age to detect malicious releases before they reach this project
- **Lockfile**: A `pnpm-lock.yaml` is committed and verified with `--frozen-lockfile` in CI
- **Limited build scripts**: Only `esbuild` is permitted to run install scripts (`allowBuilds` in `pnpm-workspace.yaml`)
- **CodeQL**: Static analysis runs on every push and PR
- **pnpm audit**: Runs in CI to catch known vulnerabilities
- **OpenSSF Scorecard**: Automated supply chain health assessment
