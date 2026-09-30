/**
 * The single boundary between the extension and the subtrack CLI.
 *
 * Everything here shells out to the installed binary rather than importing
 * subtrack, so the extension stays decoupled from subtrack's internal module
 * layout. The contract is narrow on purpose: `subtrack <cmd> --json` is a
 * machine-readable surface, and the banner is suppressed when `--json` is
 * present so stdout is exactly one JSON document.
 */
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { getPreferenceValues } from "@raycast/api";
import type {
  BudgetData,
  PaymentData,
  Subscription,
  SummaryData,
  UpcomingEntry,
} from "./types";

const run = promisify(execFile);

/**
 * Hard ceiling on a single CLI invocation.
 *
 * Raycast keeps the view open until the promise settles, so without this a
 * subtrack process blocked on a stale database lock would leave the command
 * spinning forever with no error and no way out.
 */
const TIMEOUT_MS = 30_000;

/** Generous for a JSON dump of any realistic subscription table, small enough to bound memory. */
const MAX_BUFFER = 16 * 1024 * 1024;

type Preferences = {
  binaryPath: string;
  dbDir: string;
  defaultCurrency: string;
};

export class SubtrackError extends Error {
  readonly hint: string | undefined;

  constructor(message: string, hint?: string) {
    super(message);
    this.name = "SubtrackError";
    this.hint = hint;
  }
}

function preferences(): Preferences {
  return getPreferenceValues<Preferences>();
}

/**
 * Resolve the executable to invoke.
 *
 * An explicit preference wins; otherwise `execFile` resolves the bare name
 * through PATH itself, which avoids a second resolution mechanism that could
 * disagree with the shell's.
 */
function binary(): string {
  const configured = preferences().binaryPath.trim();
  return configured === "" ? "subtrack" : configured;
}

function env(): NodeJS.ProcessEnv {
  const dbDir = preferences().dbDir.trim();
  if (dbDir === "") return process.env;
  return { ...process.env, SUBSC_CLI_DB_DIR: dbDir };
}

/** Preferences are empty until Raycast hands them over; treat that as "unset". */
function currency(): string | undefined {
  const value = preferences().defaultCurrency.trim();
  return value === "" ? undefined : value;
}

// A stray colorized line would otherwise leak control bytes into the UI.
// Splitting on ESC and stripping the SGR parameters avoids a regex that would
// have to embed a control character.
const ESC = String.fromCharCode(27);

/**
 * Every C0 control, DEL, and the C1 range.
 *
 * The CLI strips SGR sequences but not the rest: a subscription named
 * `^[[2J` (clear screen) or `^[]0;title^G` (set terminal title) reaches the UI
 * verbatim, and the Raycast list is not a terminal but a React text node that
 * will happily render the control bytes. Removing all of them is cheap and
 * leaves nothing for a future code path to forget about.
 */
// eslint-disable-next-line no-control-regex
const CONTROL_CHARS = /[\u0000-\u001F\u007F-\u009F]/g;

function clean(text: string): string {
  return text
    .split(ESC)
    .map((chunk) => chunk.replace(/^\[[0-9;]*m/, ""))
    .join("")
    .replace(CONTROL_CHARS, " ")
    .trim();
}

/**
 * Explain the common failures, since a bare "command not found" leaves the user
 * guessing whether Raycast or subtrack is at fault.
 */
function explain(error: NodeJS.ErrnoException): SubtrackError {
  if (error.code === "ENOENT") {
    const configured = preferences().binaryPath.trim();
    return new SubtrackError(
      configured === ""
        ? "Could not find the subtrack CLI on your PATH."
        : `No executable at ${configured}.`,
      'Install it with "npm install -g subtrack", or set the binary path in the extension preferences.',
    );
  }
  if (error.code === "EACCES") {
    return new SubtrackError(
      "The subtrack executable is not runnable.",
      "Check the file permissions, or set a different binary path in the extension preferences.",
    );
  }
  return new SubtrackError(error.message);
}

/** Turn a failed spawn into something worth putting in front of a user. */
function toError(error: unknown): SubtrackError {
  const err = error as NodeJS.ErrnoException & {
    code?: string;
    stdout?: string;
    killed?: boolean;
    signal?: NodeJS.Signals;
  };
  if (err.code === "ENOENT" || err.code === "EACCES") return explain(err);
  // execFile reports a timeout as a kill rather than an exit code.
  if (err.killed === true) {
    return new SubtrackError(
      `The subtrack CLI did not finish within ${TIMEOUT_MS / 1000} seconds and was stopped.`,
      "A lock file left by a crashed run can do this. Check `subtrack diagnostics`, then try again.",
    );
  }
  // subtrack reports failures through the logger and a non-zero exit code, so
  // stdout carries the message even though the promise rejected.
  const reported = clean(err.stdout ?? "");
  const last = reported.split("\n").filter(Boolean).pop();
  return new SubtrackError(last ?? err.message);
}

/**
 * Build a single `--key=value` argument.
 *
 * `execFile` gives us no shell, which stops word splitting — but the CLI still
 * runs every argv element through its own tokenizer, and that tokenizer treats
 * any element starting with `-` as a new flag. Passing the value as its own
 * argv slot therefore lets user-typed text become a flag: a subscription named
 * `--force` sent as `["--name", "--force"]` is re-read by subtrack as the
 * `--force` flag, and `--name` is left valueless. A lone `--` is worse, since it
 * terminates option parsing and silently drops every argument after it.
 *
 * The inline `--key=value` form binds the value to the key before tokenizing
 * (subtrack's `parser.ts` splits on the first `=` at index 3 or later), so the
 * value can never be reinterpreted no matter what it contains. The `String()`
 * coercion also guarantees a string, since a stray number would otherwise be
 * rejected by `execFile` with an opaque error.
 */
function flag(key: string, value: string): string {
  return `--${key}=${String(value)}`;
}

/** Run a read command and parse its JSON stdout. */
async function readJson<T>(args: string[]): Promise<T> {
  try {
    const { stdout } = await run(binary(), [...args, "--json"], {
      env: env(),
      maxBuffer: MAX_BUFFER,
      timeout: TIMEOUT_MS,
    });
    return JSON.parse(stdout) as T;
  } catch (error) {
    throw toError(error);
  }
}

/** Run a mutating command. These have no JSON output, so only the exit matters. */
async function mutate(args: string[]): Promise<void> {
  try {
    await run(binary(), args, {
      env: env(),
      maxBuffer: MAX_BUFFER,
      timeout: TIMEOUT_MS,
    });
  } catch (error) {
    throw toError(error);
  }
}

// ── Read commands ───────────────────────────────────────

export type ListFilter = {
  status?: string;
  tags?: string;
  minPrice?: string;
  maxPrice?: string;
  sort?: string;
  desc?: boolean;
  limit?: string;
  offset?: string;
  includeArchived?: boolean;
};

export async function listSubscriptions(
  filter: ListFilter = {},
): Promise<Subscription[]> {
  const args = ["list"];
  // Every value goes through `flag()`: `--tags` and `--sort` are populated from
  // subscription data, and a tag literally named `--desc` would otherwise be
  // re-read by the CLI's tokenizer as a boolean flag.
  const c = currency();
  if (c !== undefined) args.push(flag("currency", c));
  if (filter.status !== undefined) args.push(flag("status", filter.status));
  if (filter.tags !== undefined) args.push(flag("tags", filter.tags));
  if (filter.minPrice !== undefined)
    args.push(flag("min-price", filter.minPrice));
  if (filter.maxPrice !== undefined)
    args.push(flag("max-price", filter.maxPrice));
  if (filter.sort !== undefined) args.push(flag("sort", filter.sort));
  if (filter.desc === true) args.push("--desc");
  if (filter.includeArchived === true) args.push("--include-archived");
  if (filter.limit !== undefined) args.push(flag("limit", filter.limit));
  if (filter.offset !== undefined) args.push(flag("offset", filter.offset));
  return readJson<Subscription[]>(args);
}

export function upcoming(days?: number): Promise<UpcomingEntry[]> {
  const args = days === undefined ? ["upcoming"] : ["upcoming", String(days)];
  const c = currency();
  if (c !== undefined) args.push(flag("currency", c));
  return readJson<UpcomingEntry[]>(args);
}

/** `summary` takes no currency flag upstream, so totals stay in their original currencies. */
export function summary(): Promise<SummaryData> {
  return readJson<SummaryData>(["summary"]);
}

export function payment(period = "monthly"): Promise<PaymentData> {
  const args = ["payment", period];
  const c = currency();
  if (c !== undefined) args.push(flag("currency", c));
  return readJson<PaymentData>(args);
}

export function budget(period = "monthly"): Promise<BudgetData> {
  const args = ["budget", flag("period", period)];
  const c = currency();
  if (c !== undefined) args.push(flag("currency", c));
  return readJson<BudgetData>(args);
}

/** Every tag currently in use, so filter dropdowns need no hardcoded list. */
export async function tags(): Promise<string[]> {
  const subs = await listSubscriptions({ includeArchived: true });
  return [...new Set(subs.flatMap((s) => s.tags))].sort();
}

// ── Mutating commands ───────────────────────────────────
//
// `--force` is mandatory for pause/resume/cancel: without a TTY the CLI would
// sit on a confirmation prompt that Raycast cannot answer. `delete` is the
// exception — given explicit IDs it deletes straight away and has no such flag.

/** Form values, kept as strings because that is what the CLI flags expect. */
export type SubscriptionInput = {
  name: string;
  price: string;
  currency: string;
  cycle: string;
  tags: string;
  status: string;
  billingDay?: string;
  notes?: string;
  paymentMethod?: string;
  vendorName?: string;
  vendorUrl?: string;
  planTier?: string;
  discountAmount?: string;
  discountType?: string;
  contractStart?: string;
  contractEnd?: string;
  autoRenewal?: string;
};

/**
 * Fields `add` cannot proceed without. Sending every one of them keeps the CLI
 * out of interactive mode: `add` falls back to a prompt for `status` even
 * though the flag is documented as defaulting to `active`.
 */
const REQUIRED_FIELDS = [
  "name",
  "price",
  "currency",
  "cycle",
  "tags",
  "status",
] as const;

const OPTIONAL_FIELDS = [
  "billingDay",
  "notes",
  "paymentMethod",
  "vendorName",
  "vendorUrl",
  "planTier",
  "discountAmount",
  "discountType",
  "contractStart",
  "contractEnd",
  "autoRenewal",
] as const;

function withValues(
  args: string[],
  input: Partial<SubscriptionInput>,
): string[] {
  for (const key of REQUIRED_FIELDS) {
    const value = input[key];
    if (value !== undefined && value !== "") args.push(flag(key, value));
  }
  for (const key of OPTIONAL_FIELDS) {
    const value = input[key];
    if (value !== undefined && value !== "") args.push(flag(key, value));
  }
  return args;
}

export function addSubscription(input: SubscriptionInput): Promise<void> {
  return mutate(withValues(["add"], input));
}

export function editSubscription(
  id: number,
  input: Partial<SubscriptionInput>,
): Promise<void> {
  return mutate(withValues(["edit", String(id)], input));
}

export function pauseSubscriptions(ids: number[]): Promise<void> {
  return mutate(["pause", "--force", ...ids.map(String)]);
}

export function resumeSubscriptions(ids: number[]): Promise<void> {
  return mutate(["resume", "--force", ...ids.map(String)]);
}

export function cancelSubscription(id: number): Promise<void> {
  return mutate(["cancel", "--force", String(id)]);
}

export function deleteSubscriptions(ids: number[]): Promise<void> {
  return mutate(["delete", ...ids.map(String)]);
}
