#!/usr/bin/env node
// @ts-check
// Thin launcher that runs the native `subtrack` binary shipped in the
// platform-specific optional dependency. `npm` installs only the package whose
// `os`/`cpu` fields match the current machine, so this file resolves that
// package and forwards argv, stdio, signals, and the exit code unchanged.
import { spawn } from "node:child_process";
import { chmodSync, realpathSync, statSync } from "node:fs";
import { createRequire } from "node:module";
import process from "node:process";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);

/** @typedef {{ args: string[]; command: string } | { errorMessage: string }} CliRuntime */
/** @typedef {{ mode: number }} FileMode */
/** @typedef {{ error?: Error; signal?: NodeJS.Signals | null; status: number | null }} NativeSpawnResult */
/** @typedef {(command: string, args: string[]) => Promise<NativeSpawnResult>} NativeSpawner */
/** @typedef {{ argvEntry?: string; moduleUrl: string; realpathPath?: (path: string) => string }} MainModuleOptions */
/** @typedef {{ off: (signal: NodeJS.Signals, handler: () => void) => unknown; on: (signal: NodeJS.Signals, handler: () => void) => unknown }} SignalSource */
/** @typedef {{ kill: (signal: NodeJS.Signals) => unknown; on: import("node:events").EventEmitter["on"] }} NativeChild */
/** @typedef {(command: string, args: string[], options: { stdio: "inherit" }) => NativeChild} NativeSpawnProcess */

const NATIVE_PACKAGE_SCOPE = "@subtrack-org";
const NATIVE_PLATFORM_ARCHES = [
  "linux-x64",
  "linux-arm64",
  "darwin-x64",
  "darwin-arm64",
  "win32-x64",
  "win32-arm64",
];

/**
 * @type {Readonly<Record<string, string>>}
 */
const NATIVE_PACKAGES = Object.fromEntries(
  NATIVE_PLATFORM_ARCHES.map((platformArch) => [platformArch, `${NATIVE_PACKAGE_SCOPE}/subtrack-${platformArch}`]),
);

/**
 * @param {string} platform
 * @param {string} arch
 * @returns {string | undefined}
 */
function getNativePackageName(platform, arch) {
  return NATIVE_PACKAGES[`${platform}-${arch}`];
}

/**
 * @param {string} [platform]
 * @returns {string}
 */
function getNativeBinarySubpath(platform = process.platform) {
  return platform === "win32" ? "bin/subtrack.exe" : "bin/subtrack";
}

/**
 * @param {{ arch?: string; platform?: string; resolvePath?: (id: string) => string }} [options]
 * @returns {string | undefined}
 */
function resolveNativeBinary({
  arch = process.arch,
  platform = process.platform,
  resolvePath = (id) => require.resolve(id),
} = {}) {
  const packageName = getNativePackageName(platform, arch);
  if (packageName == null) {
    return undefined;
  }

  try {
    return resolvePath(`${packageName}/${getNativeBinarySubpath(platform)}`);
  } catch {
    return undefined;
  }
}

/**
 * @param {{ arch?: string; argv: string[]; nativeBinaryPath?: string | null; platform?: string; env?: NodeJS.ProcessEnv }} options
 * @returns {CliRuntime}
 */
function resolveCliRuntime({
  argv,
  arch = process.arch,
  nativeBinaryPath = resolveNativeBinary({ arch }),
  platform = process.platform,
  env = process.env,
}) {
  // Local development / nix: point at a freshly built binary directly.
  const override = env.SUBTRACK_BINARY;
  if (override != null && override !== "") {
    return { args: argv, command: override };
  }

  if (nativeBinaryPath != null) {
    return { args: argv, command: nativeBinaryPath };
  }

  return {
    errorMessage: `subtrack native binary is not available for ${platform}-${arch}. Reinstall subtrack so optional native dependencies are installed.\n`,
  };
}

/**
 * @param {unknown} error
 * @returns {string}
 */
function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

/**
 * @param {{ binaryPath: string; chmodPath?: (path: string, mode: number) => void; platform?: string; statPath?: (path: string) => FileMode }} options
 * @returns {string | undefined}
 */
function ensureNativeBinaryExecutable({
  binaryPath,
  chmodPath = chmodSync,
  platform = process.platform,
  statPath = statSync,
}) {
  if (platform === "win32") {
    return undefined;
  }

  try {
    const mode = statPath(binaryPath).mode;
    if ((mode & 0o111) !== 0) {
      return undefined;
    }
    chmodPath(binaryPath, 0o755);
    return undefined;
  } catch (error) {
    return `subtrack native binary is not executable: ${errorMessage(error)}\n`;
  }
}

/**
 * @param {MainModuleOptions} options
 * @returns {boolean}
 */
function isMainModule({
  argvEntry = process.argv[1],
  moduleUrl,
  realpathPath = realpathSync,
}) {
  if (argvEntry == null) {
    return false;
  }

  const modulePath = fileURLToPath(moduleUrl);
  try {
    if (realpathPath(modulePath) === realpathPath(argvEntry)) {
      return true;
    }
  } catch {
    // Paths could not be real-pathed (missing entry point); compare raws below.
  }
  return toComparablePath(modulePath) === toComparablePath(argvEntry);
}

/**
 * Normalizes Windows path styles so entry points compare equal regardless of
 * representation: an URL pathname (`file:///D:/x` stored as `/D:/x`) is turned
 * into a drive path and separators are unified to `\`.
 * @param {string} filePath
 * @returns {string}
 */
function toComparablePath(filePath) {
  if (process.platform !== "win32") {
    return filePath;
  }
  return filePath.replace(/^\/([A-Za-z]:)/, "$1").replaceAll("/", "\\");
}

/**
 * @param {string} [platform]
 * @returns {NodeJS.Signals[]}
 */
function getForwardedSignals(platform = process.platform) {
  if (platform === "win32") {
    return ["SIGINT", "SIGBREAK", "SIGHUP"];
  }

  return ["SIGINT", "SIGTERM", "SIGHUP", "SIGQUIT"];
}

/**
 * @param {{ signalSource?: SignalSource; spawnProcess?: NativeSpawnProcess; platform?: string }} [options]
 * @returns {NativeSpawner}
 */
function createNativeSpawner({
  platform = process.platform,
  signalSource = process,
  spawnProcess = spawn,
} = {}) {
  const forwardedSignals = getForwardedSignals(platform);

  return async (command, args) =>
    new Promise((resolve) => {
      const child = spawnProcess(command, args, { stdio: "inherit" });
      const signalHandlers = new Map();
      const cleanup = () => {
        for (const [signal, handler] of signalHandlers) {
          signalSource.off(signal, handler);
        }
        signalHandlers.clear();
      };
      /**
       * @param {NodeJS.Signals} signal
       */
      const handleSignal = (signal) => {
        child.kill(signal);
      };

      child.on("error", (error) => {
        cleanup();
        resolve({ error, signal: null, status: null });
      });
      child.on("exit", (status, signal) => {
        cleanup();
        resolve({ signal, status });
      });

      for (const signal of forwardedSignals) {
        const handler = () => handleSignal(signal);
        try {
          signalSource.on(signal, handler);
          signalHandlers.set(signal, handler);
        } catch {}
      }
    });
}

/**
 * @param {string[]} argv
 * @param {NativeSpawner} [spawnNative]
 * @returns {Promise<number>}
 */
async function runCli(argv, spawnNative = createNativeSpawner()) {
  const runtime = resolveCliRuntime({ argv });
  if ("errorMessage" in runtime) {
    process.stderr.write(runtime.errorMessage);
    return 1;
  }

  const executableError = ensureNativeBinaryExecutable({
    binaryPath: runtime.command,
  });
  if (executableError != null) {
    process.stderr.write(executableError);
    return 1;
  }

  const result = await spawnNative(runtime.command, runtime.args);
  if (result.error != null) {
    process.stderr.write(`${result.error.message}\n`);
    return 1;
  }
  if (result.signal != null) {
    process.kill(process.pid, result.signal);
    return 1;
  }
  return result.status ?? 1;
}

if (isMainModule({ moduleUrl: import.meta.url })) {
  process.exitCode = await runCli(process.argv.slice(2));
}

export {
  NATIVE_PACKAGES,
  createNativeSpawner,
  ensureNativeBinaryExecutable,
  getNativeBinarySubpath,
  getNativePackageName,
  isMainModule,
  resolveCliRuntime,
  resolveNativeBinary,
  runCli,
};
