import { useCallback, useEffect, useMemo, useState } from "react";
import type { JSX } from "react";
import {
  Action,
  ActionPanel,
  Detail,
  Icon,
  List,
  useNavigation,
  Keyboard,
} from "@raycast/api";
import { SubscriptionActions, SubscriptionDetail } from "./lib/components";
import { ErrorList } from "./lib/errors";
import { formatDate, formatPrice, relativeDays } from "./lib/format";
import { listSubscriptions, upcoming } from "./lib/subtrack";
import type { Subscription, UpcomingEntry } from "./lib/types";
import { EditSubscriptionForm } from "./edit-subscription";

const DEFAULT_DAYS = 30;
const WINDOWS = [7, 14, 30, 90, 365];

/** Fallback detail for the rare case where the subscription was deleted meanwhile. */
function UpcomingDetail({ entry }: { entry: UpcomingEntry }): JSX.Element {
  return (
    <Detail
      markdown={[
        `# ${entry.name}`,
        "",
        `**${formatPrice(entry.amount, entry.currency)}** due ${formatDate(entry.nextDate)} (${relativeDays(entry.nextDate)})`,
      ].join("\n")}
      metadata={
        <Detail.Metadata>
          <Detail.Metadata.Label title="Cycle" text={entry.cycle} />
          <Detail.Metadata.Label
            title="Next charge"
            text={formatDate(entry.nextDate)}
          />
        </Detail.Metadata>
      }
    />
  );
}

export default function Upcoming({
  args,
}: {
  args: { days?: string };
}): JSX.Element {
  const navigation = useNavigation();
  const [entries, setEntries] = useState<UpcomingEntry[]>([]);
  const [subscriptions, setSubscriptions] = useState<Subscription[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<unknown>(null);

  // The command takes an optional `days` argument, which seeds the window. The
  // submenu below then changes it in place rather than re-invoking the command.
  const [days, setDays] = useState(() => {
    const parsed = Number(args.days);
    return Number.isInteger(parsed) && parsed > 0 ? parsed : DEFAULT_DAYS;
  });

  const load = useCallback(async () => {
    setIsLoading(true);
    try {
      // Upcoming rows carry only a slice of each subscription, so pull the full
      // records to drive the detail view and action panel.
      const [rows, all] = await Promise.all([
        upcoming(days),
        listSubscriptions(),
      ]);
      setEntries(rows);
      setSubscriptions(all);
      setError(null);
    } catch (err) {
      setError(err);
      setEntries([]);
    } finally {
      setIsLoading(false);
    }
  }, [days]);

  useEffect(() => {
    void load();
  }, [load]);

  const due = useMemo(() => {
    const totals = entries.reduce<Record<string, number>>((acc, e) => {
      acc[e.currency] = (acc[e.currency] ?? 0) + e.amount;
      return acc;
    }, {});
    return Object.entries(totals)
      .map(([currency, total]) => `${formatPrice(total, currency)} due`)
      .join(" · ");
  }, [entries]);

  if (error !== null) {
    return (
      <List>
        <ErrorList error={error} onRetry={load} />
      </List>
    );
  }

  return (
    <List
      isLoading={isLoading}
      searchBarPlaceholder={`Renewals in the next ${days} days`}
      actions={
        <ActionPanel>
          <Action
            title="Refresh"
            icon={Icon.ArrowClockwise}
            shortcut={Keyboard.Shortcut.Common.Refresh}
            onAction={() => void load()}
          />
          <ActionPanel.Submenu title="Time Window" icon={Icon.Calendar}>
            {WINDOWS.map((w) => (
              <Action
                key={w}
                title={`Next ${w} Days`}
                icon={w === days ? Icon.Check : Icon.Circle}
                onAction={() => setDays(w)}
              />
            ))}
          </ActionPanel.Submenu>
          {due !== "" ? (
            <Action.CopyToClipboard title="Copy Total Due" content={due} />
          ) : null}
        </ActionPanel>
      }
    >
      {!isLoading && entries.length === 0 ? (
        <List.EmptyView
          icon={Icon.Calendar}
          title="Nothing due"
          description={`No renewals in the next ${days} days.`}
        />
      ) : null}

      {entries.map((entry) => {
        const sub = subscriptions.find((s) => s.id === entry.id);

        if (sub === undefined) {
          return (
            <List.Item
              key={`${entry.id}-${entry.nextDate}`}
              icon={Icon.CreditCard}
              title={entry.name}
              subtitle={`${formatPrice(entry.amount, entry.currency)} · ${entry.cycle.replace("-", " ")}`}
              accessories={[{ text: relativeDays(entry.nextDate) }]}
              actions={
                <ActionPanel>
                  <Action
                    title="Show Details"
                    icon={Icon.Eye}
                    onAction={() =>
                      navigation.push(<UpcomingDetail entry={entry} />)
                    }
                  />
                </ActionPanel>
              }
            />
          );
        }

        const editForm = (
          <EditSubscriptionForm subscription={sub} onSaved={load} />
        );
        return (
          <List.Item
            key={`${entry.id}-${entry.nextDate}`}
            icon={Icon.CreditCard}
            title={entry.name}
            subtitle={`${formatPrice(entry.amount, entry.currency)} · ${sub.cycle.replace("-", " ")}`}
            accessories={[
              { text: relativeDays(entry.nextDate) },
              { text: formatDate(entry.nextDate) },
            ]}
            actions={
              <SubscriptionActions
                sub={sub}
                onChanged={load}
                onEdit={() => navigation.push(editForm)}
                onOpen={() =>
                  navigation.push(
                    <SubscriptionDetail
                      sub={sub}
                      onChanged={load}
                      onEdit={() => navigation.push(editForm)}
                    />,
                  )
                }
              />
            }
          />
        );
      })}
    </List>
  );
}
