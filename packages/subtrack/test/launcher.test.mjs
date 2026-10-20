import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, test } from "node:test";

import {
	NATIVE_PACKAGES,
	createNativeSpawner,
	ensureNativeBinaryExecutable,
	getNativeBinarySubpath,
	getNativePackageName,
	isMainModule,
	resolveCliRuntime,
	resolveNativeBinary,
	runCli,
} from "../src/launcher.mjs";

test("NATIVE_PACKAGES fully matches the optionalDependencies declared in package.json", () => {
	const manifest = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
	const expected = Object.fromEntries(
		Object.keys(manifest.optionalDependencies).map((name) => [name.replace(/^@[^/]+\/subtrack-/, ""), name]),
	);
	assert.deepEqual(NATIVE_PACKAGES, expected);
});

test("getNativePackageName maps every supported platform/arch", () => {
	const expected = {
		"linux-x64": "@subtrack-org/subtrack-linux-x64",
		"linux-arm64": "@subtrack-org/subtrack-linux-arm64",
		"darwin-x64": "@subtrack-org/subtrack-darwin-x64",
		"darwin-arm64": "@subtrack-org/subtrack-darwin-arm64",
		"win32-x64": "@subtrack-org/subtrack-win32-x64",
		"win32-arm64": "@subtrack-org/subtrack-win32-arm64",
	};
	for (const [key, name] of Object.entries(expected)) {
		const [platform, arch] = key.split("-");
		assert.equal(getNativePackageName(platform, arch), name);
	}
	assert.equal(getNativePackageName("freebsd", "x64"), undefined);
	assert.equal(getNativePackageName("linux", "riscv64"), undefined);
});

test("getNativeBinarySubpath adds .exe only on Windows", () => {
	assert.equal(getNativeBinarySubpath("win32"), "bin/subtrack.exe");
	assert.equal(getNativeBinarySubpath("linux"), "bin/subtrack");
	assert.equal(getNativeBinarySubpath("darwin"), "bin/subtrack");
});

test("resolveNativeBinary resolves the platform package path", () => {
	const seen = [];
	const path = resolveNativeBinary({
		platform: "win32",
		arch: "arm64",
		resolvePath: (id) => {
			seen.push(id);
			return `/fake/${id}`;
		},
	});
	assert.equal(path, "/fake/@subtrack-org/subtrack-win32-arm64/bin/subtrack.exe");
	assert.deepEqual(seen, ["@subtrack-org/subtrack-win32-arm64/bin/subtrack.exe"]);
});

test("resolveNativeBinary returns undefined when unsupported or missing", () => {
	assert.equal(resolveNativeBinary({ platform: "freebsd", arch: "x64", resolvePath: () => "/x" }), undefined);
	assert.equal(
		resolveNativeBinary({
			platform: "linux",
			arch: "x64",
			resolvePath: () => {
				throw new Error("MODULE_NOT_FOUND");
			},
		}),
		undefined,
	);
});

test("resolveCliRuntime prefers SUBTRACK_BINARY override", () => {
	const runtime = resolveCliRuntime({
		argv: ["list"],
		env: { SUBTRACK_BINARY: "/custom/subtrack" },
		nativeBinaryPath: "/native/subtrack",
	});
	assert.deepEqual(runtime, { args: ["list"], command: "/custom/subtrack" });
});

test("resolveCliRuntime falls back to the native binary", () => {
	const runtime = resolveCliRuntime({ argv: ["add"], env: {}, nativeBinaryPath: "/native/subtrack" });
	assert.deepEqual(runtime, { args: ["add"], command: "/native/subtrack" });
});

test("resolveCliRuntime reports a helpful error when no binary is available", () => {
	const runtime = resolveCliRuntime({ argv: [], env: {}, nativeBinaryPath: null, platform: "linux", arch: "x64" });
	assert.ok("errorMessage" in runtime);
	assert.match(runtime.errorMessage, /linux-x64/);
	assert.match(runtime.errorMessage, /Reinstall subtrack/);
});

test("ensureNativeBinaryExecutable skips Windows", () => {
	let chmodCalled = false;
	const error = ensureNativeBinaryExecutable({
		binaryPath: "/x/subtrack.exe",
		platform: "win32",
		chmodPath: () => {
			chmodCalled = true;
		},
	});
	assert.equal(error, undefined);
	assert.equal(chmodCalled, false);
});

test("ensureNativeBinaryExecutable chmods a non-executable binary", () => {
	const chmods = [];
	const error = ensureNativeBinaryExecutable({
		binaryPath: "/x/subtrack",
		platform: "linux",
		statPath: () => ({ mode: 0o644 }),
		chmodPath: (path, mode) => chmods.push([path, mode]),
	});
	assert.equal(error, undefined);
	assert.deepEqual(chmods, [["/x/subtrack", 0o755]]);
});

test("ensureNativeBinaryExecutable leaves an executable binary alone", () => {
	let chmodCalled = false;
	const error = ensureNativeBinaryExecutable({
		binaryPath: "/x/subtrack",
		platform: "linux",
		statPath: () => ({ mode: 0o755 }),
		chmodPath: () => {
			chmodCalled = true;
		},
	});
	assert.equal(error, undefined);
	assert.equal(chmodCalled, false);
});

test("ensureNativeBinaryExecutable reports a missing binary", () => {
	const error = ensureNativeBinaryExecutable({
		binaryPath: "/x/subtrack",
		platform: "linux",
		statPath: () => {
			throw new Error("ENOENT");
		},
	});
	assert.match(error ?? "", /not executable/);
});

test("createNativeSpawner forwards the exit status", async () => {
	const child = new EventEmitter();
	child.kill = () => {};
	const spawnNative = createNativeSpawner({
		platform: "linux",
		signalSource: new EventEmitter(),
		spawnProcess: () => {
			queueMicrotask(() => child.emit("exit", 3, null));
			return child;
		},
	});
	const result = await spawnNative("/x/subtrack", ["list"]);
	assert.deepEqual(result, { signal: null, status: 3 });
});

test("createNativeSpawner forwards signals to the child", async () => {
	const child = new EventEmitter();
	const killed = [];
	child.kill = (signal) => killed.push(signal);
	const signalSource = new EventEmitter();
	const spawnNative = createNativeSpawner({
		platform: "win32",
		signalSource,
		spawnProcess: () => {
			queueMicrotask(() => {
				signalSource.emit("SIGINT");
				signalSource.emit("SIGTERM"); // not forwarded on win32
				child.emit("exit", 130, "SIGINT");
			});
			return child;
		},
	});
	const result = await spawnNative("/x/subtrack.exe", []);
	assert.deepEqual(killed, ["SIGINT"]);
	assert.equal(result.signal, "SIGINT");
});

test("isMainModule matches the invoked entry point", () => {
	const moduleUrl = new URL("../src/launcher.mjs", import.meta.url);
	assert.equal(isMainModule({ moduleUrl, argvEntry: new URL(moduleUrl).pathname, realpathPath: (p) => p }), true);
	assert.equal(isMainModule({ moduleUrl, argvEntry: "/other/entry.mjs", realpathPath: (p) => p }), false);
	assert.equal(isMainModule({ moduleUrl, argvEntry: undefined, realpathPath: (p) => p }), false);
});

const tmp = mkdtempSync(join(tmpdir(), "subtrack-launcher-"));
after(() => rmSync(tmp, { recursive: true, force: true }));

test("runCli spawns the resolved binary and returns its exit code", async () => {
	const binary = join(tmp, "subtrack");
	writeFileSync(binary, "#!/bin/sh\nexit 0\n");
	const previous = process.env.SUBTRACK_BINARY;
	process.env.SUBTRACK_BINARY = binary;
	try {
		const calls = [];
		const code = await runCli(["summary"], async (command, args) => {
			calls.push([command, args]);
			return { signal: null, status: 0 };
		});
		assert.equal(code, 0);
		assert.deepEqual(calls, [[binary, ["summary"]]]);
	} finally {
		if (previous == null) {
			delete process.env.SUBTRACK_BINARY;
		} else {
			process.env.SUBTRACK_BINARY = previous;
		}
	}
});

test("runCli returns 1 when spawning fails", async () => {
	const binary = join(tmp, "subtrack-fail");
	writeFileSync(binary, "");
	const previous = process.env.SUBTRACK_BINARY;
	process.env.SUBTRACK_BINARY = binary;
	try {
		const code = await runCli(["list"], async () => ({
			error: new Error("spawn EACCES"),
			signal: null,
			status: null,
		}));
		assert.equal(code, 1);
	} finally {
		if (previous == null) {
			delete process.env.SUBTRACK_BINARY;
		} else {
			process.env.SUBTRACK_BINARY = previous;
		}
	}
});
