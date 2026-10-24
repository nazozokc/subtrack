#!/usr/bin/env node
// @ts-check
// Sets the release version across the published npm packages:
//   - packages/subtrack
//   - packages/subtrack-<os>-<arch>  (the 6 native packages)
//
// Usage: node scripts/set-version.mjs {version}
//
// The Rust binary reports its version via the SUBTRACK_VERSION build-time env
// var (see rust/crates/subtrack/src/cli.rs), so Cargo.toml does not need to be
// rewritten here. Keep rust/Cargo.toml's workspace version in sync manually as
// the fallback for local builds.
import { globSync, readFileSync, writeFileSync } from "node:fs";
import process from "node:process";

const version = process.argv[2];
if (version == null || !/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
	process.stderr.write("usage: node scripts/set-version.mjs <valid semver>\n");
	process.exit(1);
}

const manifests = globSync("packages/subtrack/package.json").concat(
	globSync("packages/subtrack-*/package.json"),
);

if (manifests.length === 0) {
	process.stderr.write("no package manifests found\n");
	process.exit(1);
}

for (const manifest of manifests) {
	const raw = readFileSync(manifest, "utf8");
	const pkg = JSON.parse(raw);
	if (pkg.private === true || pkg.name == null) {
		continue;
	}
	pkg.version = version;
	writeFileSync(manifest, `${JSON.stringify(pkg, null, 2)}\n`);
	process.stdout.write(`${pkg.name}@${version}\n`);
}
