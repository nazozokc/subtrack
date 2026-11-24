# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in subtrack, please report it privately.

**Do not** report security vulnerabilities via public GitHub issues.

Instead, send a description of the issue (including steps to reproduce, affected versions, and any relevant code context) to one of the following:

- Open a **private security advisory** at: https://github.com/nazozokc/subtrack/security/advisories/new
- Email: **nazozokc@gmail.com**

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

- **Provenance attestation**: npm releases use `pnpm publish --provenance` with `publishConfig.provenance: true` in `packages/subtrack/package.json`. Release binaries do **not** currently have `actions/attest-build-provenance` attestation.
- **SBOM generation**: Every release generates an SPDX Software Bill of Materials via `anchore/sbom-action`; it is uploaded as a CI artifact and is not attached to the GitHub release.
- **Dependency review**: Every pull request is scanned for new vulnerabilities via GitHub's dependency review action.
- **OSV-Scanner**: Scheduled CI runs Google's OSV-Scanner against all dependencies to detect known vulnerabilities across open source databases.
- **Renovate bot**: Dependencies are updated with a 14-day minimum release age (`.github/renovate.json`). Renovate PRs are only auto-approved when auto-merge is enabled (skipping major/dashboard-approval updates). OSV vulnerability alerts are enabled in the dependency dashboard.
- **Dependabot**: Defense-in-depth alongside Renovate — catches vulnerabilities faster via GitHub Advisory Database integration.
- **Lockfile**: A `pnpm-lock.yaml` is committed and verified with `--frozen-lockfile` in CI. No `npm-shrinkwrap.json` is generated.
- **Limited build scripts**: Only `esbuild` is permitted to run install scripts (`allowBuilds` in `pnpm-workspace.yaml`).
- **Harden-Runner**: Every CI workflow uses `step-security/harden-runner` (pinned to v2.23.0 SHA `aebdcf6156ab6691adf63218240e476564a1ed4c`) for runtime egress monitoring and threat detection.
- **CodeQL**: Static analysis runs on pushes and pull requests against `main`, plus a weekly schedule.
- **pnpm audit**: Runs in CI to catch known vulnerabilities.
- **OpenSSF Scorecard**: Automated supply chain health assessment with results published to the repository's Security tab.
- **Tag signing**: Release tags should be signed with GPG/SSH — see [Signing tags](https://docs.github.com/en/authentication/managing-commit-signature-verification/signing-tags).
- **Socket.dev**: Published package monitored on [Socket.dev](https://socket.dev/npm/package/subtrack) for supply chain risk indicators.
