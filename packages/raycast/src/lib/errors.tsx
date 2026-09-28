import type { JSX } from "react";
import { Action, ActionPanel, Detail, Icon, List } from "@raycast/api";
import { SubtrackError } from "./subtrack";

function describe(error: unknown): { title: string; hint?: string } {
  if (error instanceof SubtrackError)
    return { title: error.message, hint: error.hint };
  if (error instanceof Error) return { title: error.message };
  return { title: String(error) };
}

function retryAction(onRetry?: () => Promise<void>): JSX.Element | undefined {
  if (onRetry === undefined) return undefined;
  return (
    <ActionPanel>
      <Action
        title="Try Again"
        icon={Icon.ArrowClockwise}
        onAction={() => void onRetry()}
      />
    </ActionPanel>
  );
}

/** Failure inside a List view. The caller must already be inside a `<List>`. */
export function ErrorList({
  error,
  onRetry,
}: {
  error: unknown;
  onRetry?: () => Promise<void>;
}): JSX.Element {
  const { title, hint } = describe(error);
  return (
    <List.EmptyView
      icon={Icon.ExclamationMark}
      title={title}
      description={hint}
      actions={retryAction(onRetry)}
    />
  );
}

/** Failure inside a Detail view, for the read-only report commands. */
export function ErrorDetail({
  error,
  onRetry,
}: {
  error: unknown;
  onRetry?: () => Promise<void>;
}): JSX.Element {
  const { title, hint } = describe(error);
  return (
    <Detail
      markdown={[
        `# ⚠️ ${title}`,
        "",
        ...(hint === undefined
          ? []
          : [hint, "", "Use **Try Again** in the action panel."]),
      ].join("\n")}
      actions={retryAction(onRetry)}
    />
  );
}
