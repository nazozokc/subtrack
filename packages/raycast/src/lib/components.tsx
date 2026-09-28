import type { JSX } from "react";
import {
  Action,
  ActionPanel,
  Detail,
  Icon,
  Toast,
  confirmAlert,
  showHUD,
  showToast,
} from "@raycast/api";
import {
  cancelSubscription,
  deleteSubscriptions,
  pauseSubscriptions,
  resumeSubscriptions,
} from "./subtrack";
import type { Subscription } from "./types";
import { formatPrice, statusLabel, subscriptionFieldRows } from "./format";

/**
 * Actions for a subscription.
 *
 * Status-changing actions are offered conditionally so the panel never offers
 * something the CLI would reject, and every destructive action is guarded by
 * `confirmAlert` because the CLI's own confirmation needs a TTY.
 */
export function SubscriptionActions({
  sub,
  onChanged,
  onEdit,
  onOpen,
}: {
  sub: Subscription;
  onChanged: () => Promise<void>;
  onEdit: () => void;
  onOpen?: () => void;
}): JSX.Element {
  async function guard(
    title: string,
    action: () => Promise<void>,
  ): Promise<void> {
    const confirmed = await confirmAlert({ title });
    if (!confirmed) return;
    try {
      await action();
    } catch (error) {
      await showToast({
        style: Toast.Style.Failure,
        title: error instanceof Error ? error.message : String(error),
      });
      return;
    }
    await showHUD(`${sub.name} updated`);
    await onChanged();
  }

  return (
    <ActionPanel>
      {/* The first action is what Return triggers, and `List.Item` has no
          onAction of its own — so opening the detail view goes here. */}
      {onOpen ? (
        <Action title="Show Details" icon={Icon.Eye} onAction={onOpen} />
      ) : null}

      <Action title="Edit Subscription" icon={Icon.Pencil} onAction={onEdit} />

      {sub.status === "active" ? (
        <>
          <Action
            title="Pause Subscription"
            icon={Icon.Pause}
            onAction={() =>
              guard(`Pause ${sub.name}?`, () => pauseSubscriptions([sub.id]))
            }
          />
          <Action
            title="Cancel Subscription"
            icon={Icon.XmarkCircle}
            onAction={() =>
              guard(`Cancel ${sub.name}?`, () => cancelSubscription(sub.id))
            }
          />
        </>
      ) : null}

      {sub.status === "paused" || sub.status === "cancelled" ? (
        <Action
          title="Resume Subscription"
          icon={Icon.Play}
          onAction={() =>
            guard(`Resume ${sub.name}?`, () => resumeSubscriptions([sub.id]))
          }
        />
      ) : null}

      {sub.vendorUrl ? (
        <Action.OpenInBrowser title="Open Vendor" url={sub.vendorUrl} />
      ) : null}

      <Action.CopyToClipboard
        title="Copy Price"
        content={`${sub.name} — ${formatPrice(sub.price, sub.currency)}/${sub.cycle}`}
      />
      <Action.CopyToClipboard title="Copy Name" content={sub.name} />

      <Action
        title="Delete Subscription"
        icon={Icon.Trash}
        style={Action.Style.Destructive}
        shortcut={{
          macOS: { modifiers: ["cmd", "shift"], key: "backspace" },
          Windows: { modifiers: ["ctrl", "shift"], key: "delete" },
        }}
        onAction={() =>
          guard(`Permanently delete ${sub.name}? This cannot be undone.`, () =>
            deleteSubscriptions([sub.id]),
          )
        }
      />
    </ActionPanel>
  );
}

export function SubscriptionDetail({
  sub,
  onChanged,
  onEdit,
}: {
  sub: Subscription;
  onChanged: () => Promise<void>;
  onEdit: () => void;
}): JSX.Element {
  const lines = [
    `# ${sub.name}`,
    "",
    `**${formatPrice(sub.price, sub.currency)}** every ${sub.cycle.replace("-", " ")} · ${statusLabel(sub.status)}`,
  ];
  if (sub.tags.length > 0)
    lines.push("", sub.tags.map((t) => `\`#${t}\``).join(" "));
  if (sub.notes) lines.push("", sub.notes);

  return (
    <Detail
      markdown={lines.join("\n")}
      metadata={
        <Detail.Metadata>
          {subscriptionFieldRows(sub).map((row) => (
            <Detail.Metadata.Label
              key={row.label}
              title={row.label}
              text={row.text}
            />
          ))}
        </Detail.Metadata>
      }
      actions={
        <SubscriptionActions sub={sub} onChanged={onChanged} onEdit={onEdit} />
      }
    />
  );
}
