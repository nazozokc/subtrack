#!/usr/bin/env node
// @ts-check
// `prepack` guard for the platform-specific native packages. Runs with the
// package directory as cwd (pnpm/npm invoke lifecycle scripts there), so it
// reads that package's manifest, finds its `bin/*` entry, and asserts the
// binary exists and is executable before the tarball is created. Publishing a
// native package without its binary would produce a broken `npx subtrack`.
import { constants } from "node:fs";
import { accessSync, readFileSync, statSync } from "node:fs";
import { resolve } from "node:path";
import process from "node:process";

const pkg = JSON.parse(readFileSync("package.json", "utf8"));
const entries = (pkg.files ?? []).filter((file) => typeof file === "string" && file.startsWith("bin/"));

if (entries.length === 0) {
	process.stderr.write(`${pkg.name}: no bin/* entry in "files"\n`);
	process.exit(1);
}

for (const entry of entries) {
	const binary = resolve(entry);
	let stat;
	try {
		stat = statSync(binary);
	} catch {
		process.stderr.write(`${pkg.name}: missing native binary ${entry}\n`);
		process.exit(1);
	}
	if (!stat.isFile()) {
		process.stderr.write(`${pkg.name}: ${entry} is not a file\n`);
		process.exit(1);
	}
	if (entry.endsWith(".exe")) {
		continue;
	}
	try {
		accessSync(binary, constants.X_OK);
	} catch {
		process.stderr.write(`${pkg.name}: ${entry} is not executable\n`);
		process.exit(1);
	}
}

process.stdout.write(`${pkg.name}: native binary verified\n`);
