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

function clean(text: string): string {
  return text
    .split(ESC)
    .map((chunk) => chunk.replace(/^\[[0-9;]*m/, ""))
    .join("")
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
  };
  if (err.code === "ENOENT" || err.code === "EACCES") return explain(err);
  // subtrack reports failures through the logger and a non-zero exit code, so
  // stdout carries the message even though the promise rejected.
  const reported = clean(err.stdout ?? "");
  const last = reported.split("\n").filter(Boolean).pop();
  return new SubtrackError(last ?? err.message);
}

/** Run a read command and parse its JSON stdout. */
async function readJson<T>(args: string[]): Promise<T> {
  try {
    const { stdout } = await run(binary(), [...args, "--json"], {
      env: env(),
      maxBuffer: 64 * 1024 * 1024,
    });
    return JSON.parse(stdout) as T;
  } catch (error) {
    throw toError(error);
  }
}

/** Run a mutating command. These have no JSON output, so only the exit matters. */
async function mutate(args: string[]): Promise<void> {
  try {
    await run(binary(), args, { env: env(), maxBuffer: 8 * 1024 * 1024 });
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
  const c = currency();
  if (c !== undefined) args.push("--currency", c);
  if (filter.status !== undefined) args.push("--status", filter.status);
  if (filter.tags !== undefined) args.push("--tags", filter.tags);
  if (filter.minPrice !== undefined) args.push("--min-price", filter.minPrice);
  if (filter.maxPrice !== undefined) args.push("--max-price", filter.maxPrice);
  if (filter.sort !== undefined) args.push("--sort", filter.sort);
  if (filter.desc === true) args.push("--desc");
  if (filter.includeArchived === true) args.push("--include-archived");
  if (filter.limit !== undefined) args.push("--limit", filter.limit);
  if (filter.offset !== undefined) args.push("--offset", filter.offset);
  return readJson<Subscription[]>(args);
}

export function upcoming(days?: number): Promise<UpcomingEntry[]> {
  const args = days === undefined ? ["upcoming"] : ["upcoming", String(days)];
  const c = currency();
  if (c !== undefined) args.push("--currency", c);
  return readJson<UpcomingEntry[]>(args);
}

/** `summary` takes no currency flag upstream, so totals stay in their original currencies. */
export function summary(): Promise<SummaryData> {
  return readJson<SummaryData>(["summary"]);
}

export function payment(period = "monthly"): Promise<PaymentData> {
  const args = ["payment", period];
  const c = currency();
  if (c !== undefined) args.push("--currency", c);
  return readJson<PaymentData>(args);
}

export function budget(period = "monthly"): Promise<BudgetData> {
  const args = ["budget", "--period", period];
  const c = currency();
  if (c !== undefined) args.push("--currency", c);
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
    if (value !== undefined && value !== "") args.push(`--${key}`, value);
  }
  for (const key of OPTIONAL_FIELDS) {
    const value = input[key];
    if (value !== undefined && value !== "") args.push(`--${key}`, value);
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
