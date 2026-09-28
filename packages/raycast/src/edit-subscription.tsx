import { useCallback, useEffect, useState } from "react";
import type { JSX } from "react";
import { Action, ActionPanel, Icon, List, useNavigation } from "@raycast/api";
import { ErrorList } from "./lib/errors";
import { formatPrice, statusIcon, statusLabel } from "./lib/format";
import { SubscriptionForm } from "./lib/subscription-form";
import { listSubscriptions } from "./lib/subtrack";
import type { Subscription } from "./lib/types";

/** Pushed from a list once a subscription has been chosen. */
export function EditSubscriptionForm({
  subscription,
  onSaved,
}: {
  subscription: Subscription;
  onSaved?: () => Promise<void>;
}): JSX.Element {
  return (
    <SubscriptionForm
      subscription={subscription}
      onSaved={onSaved}
      navigationTitle={`Edit ${subscription.name}`}
    />
  );
}

export default function EditSubscription(): JSX.Element {
  const navigation = useNavigation();
  const [subscriptions, setSubscriptions] = useState<Subscription[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<unknown>(null);
  const [searchText, setSearchText] = useState("");

  const load = useCallback(async () => {
    setIsLoading(true);
    try {
      setSubscriptions(await listSubscriptions({ sort: "name" }));
      setError(null);
    } catch (err) {
      setError(err);
      setSubscriptions([]);
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  if (error !== null) {
    return (
      <List>
        <ErrorList error={error} onRetry={load} />
      </List>
    );
  }

  const query = searchText.trim().toLowerCase();
  const results =
    query === ""
      ? subscriptions
      : subscriptions.filter((s) => s.name.toLowerCase().includes(query));

  return (
    <List
      isLoading={isLoading}
      searchText={searchText}
      onSearchTextChange={setSearchText}
      searchBarPlaceholder="Search subscriptions"
    >
      {!isLoading && results.length === 0 ? (
        <List.EmptyView
          icon={query === "" ? Icon.Tray : Icon.MagnifyingGlass}
          title={query === "" ? "No subscriptions" : "No matches"}
          description={
            query === ""
              ? "Add one first with the Add Subscription command."
              : `Nothing matches “${searchText}”.`
          }
        />
      ) : null}

      {results.map((sub) => (
        <List.Item
          key={String(sub.id)}
          icon={statusIcon(sub.status)}
          title={sub.name}
          subtitle={`${formatPrice(sub.price, sub.currency)} · ${sub.cycle.replace("-", " ")}`}
          accessories={[{ text: statusLabel(sub.status) }]}
          actions={
            <ActionPanel>
              <Action
                title="Edit Subscription"
                icon={Icon.Pencil}
                onAction={() =>
                  navigation.push(
                    <EditSubscriptionForm subscription={sub} onSaved={load} />,
                  )
                }
              />
            </ActionPanel>
          }
        />
      ))}
    </List>
  );
}
