import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { JSX } from "react";
import {
  Action,
  ActionPanel,
  Icon,
  Keyboard,
  List,
  useNavigation,
} from "@raycast/api";
import { SubscriptionActions, SubscriptionDetail } from "./lib/components";
import { ErrorList } from "./lib/errors";
import { formatPrice, statusIcon, statusLabel } from "./lib/format";
import { listSubscriptions, tags as allTags } from "./lib/subtrack";
import type { Subscription } from "./lib/types";
import { EditSubscriptionForm } from "./edit-subscription";

const STATUSES = [
  { title: "All", value: "" },
  { title: "Active", value: "active" },
  { title: "Paused", value: "paused" },
  { title: "Cancelled", value: "cancelled" },
  { title: "Archived", value: "archived" },
];

const SORTS = [
  { title: "Name", value: "name" },
  { title: "Price", value: "price" },
  { title: "Currency", value: "currency" },
  { title: "Cycle", value: "cycle" },
];

export default function Subscriptions(): JSX.Element {
  const navigation = useNavigation();
  const [subscriptions, setSubscriptions] = useState<Subscription[]>([]);
  const [tagOptions, setTagOptions] = useState<string[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<unknown>(null);

  const [searchText, setSearchText] = useState("");
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState("");
  const [tag, setTag] = useState("");
  const [sort, setSort] = useState("name");
  const [descending, setDescending] = useState(false);
  const [includeArchived, setIncludeArchived] = useState(false);

  // Debounce so narrowing a few hundred rows does not run on every keystroke.
  const debounce = useRef<NodeJS.Timeout | undefined>(undefined);
  useEffect(() => {
    clearTimeout(debounce.current);
    debounce.current = setTimeout(
      () => setQuery(searchText.trim().toLowerCase()),
      200,
    );
    return () => clearTimeout(debounce.current);
  }, [searchText]);

  const load = useCallback(async () => {
    setIsLoading(true);
    try {
      setSubscriptions(
        await listSubscriptions({
          status,
          sort,
          desc: descending,
          includeArchived,
        }),
      );
      setError(null);
    } catch (err) {
      setError(err);
      setSubscriptions([]);
    } finally {
      setIsLoading(false);
    }
  }, [status, sort, descending, includeArchived]);

  useEffect(() => {
    void load();
  }, [load]);

  // Tag choices come from the data, so the filter never lists a stale tag.
  useEffect(() => {
    allTags()
      .then(setTagOptions)
      .catch(() => setTagOptions([]));
  }, []);

  // The CLI already filtered by status and sorted; search and tag narrow the
  // result further without spawning a process per keystroke.
  const results = useMemo(() => {
    const matched =
      query === ""
        ? subscriptions
        : subscriptions.filter((s) =>
            [s.name, s.notes ?? "", ...s.tags].some((f) =>
              f.toLowerCase().includes(query),
            ),
          );
    if (tag === "") return matched;
    const wanted = new Set(tag.split(","));
    return matched.filter(
      (s) => wanted.size > 0 && s.tags.every((t) => wanted.has(t)),
    );
  }, [subscriptions, query, tag]);

  function openDetail(sub: Subscription): void {
    navigation.push(
      <SubscriptionDetail
        sub={sub}
        onChanged={load}
        onEdit={() => openEdit(sub)}
      />,
    );
  }

  function openEdit(sub: Subscription): void {
    navigation.push(<EditSubscriptionForm subscription={sub} onSaved={load} />);
  }

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
      searchText={searchText}
      onSearchTextChange={setSearchText}
      searchBarPlaceholder="Search name, note, or tag"
      // Only one dropdown is supported in the search bar; the rest of the
      // filters live in the action panel submenu.
      searchBarAccessory={
        <List.Dropdown
          tooltip="Status"
          value={status}
          onChange={setStatus}
          storeValue
        >
          {STATUSES.map((s) => (
            <List.Dropdown.Item key={s.value} title={s.title} value={s.value} />
          ))}
        </List.Dropdown>
      }
      actions={
        <ActionPanel>
          <ActionPanel.Submenu title="Filters" icon={Icon.Filter}>
            {SORTS.map((s) => (
              <Action
                key={s.value}
                title={`Sort by ${s.title}`}
                icon={sort === s.value ? Icon.Check : Icon.ArrowRight}
                onAction={() => setSort(s.value)}
              />
            ))}
            <Action
              title={descending ? "Descending" : "Ascending"}
              icon={descending ? Icon.ArrowDown : Icon.ArrowUp}
              onAction={() => setDescending(!descending)}
            />
            <Action
              title={includeArchived ? "Hide Archived" : "Include Archived"}
              icon={includeArchived ? Icon.Check : Icon.Folder}
              onAction={() => setIncludeArchived(!includeArchived)}
            />
            <Action
              title="Clear Filters"
              icon={Icon.Xmark}
              onAction={() => {
                setStatus("");
                setTag("");
                setSort("name");
                setDescending(false);
                setIncludeArchived(false);
                setSearchText("");
              }}
            />
          </ActionPanel.Submenu>

          <ActionPanel.Submenu title="Tag" icon={Icon.Tag}>
            <Action
              title="All Tags"
              icon={tag === "" ? Icon.Check : Icon.Circle}
              onAction={() => setTag("")}
            />
            {tagOptions.map((t) => (
              <Action
                key={t}
                title={t}
                icon={tag === t ? Icon.Check : Icon.Circle}
                onAction={() => setTag(t)}
              />
            ))}
          </ActionPanel.Submenu>

          <Action
            title="Refresh"
            icon={Icon.ArrowClockwise}
            shortcut={Keyboard.Shortcut.Common.Refresh}
            onAction={() => void load()}
          />
        </ActionPanel>
      }
    >
      {!isLoading && results.length === 0 ? (
        <List.EmptyView
          icon={query === "" ? Icon.Tray : Icon.MagnifyingGlass}
          title={query === "" ? "No subscriptions" : "No matches"}
          description={
            query === ""
              ? "Use the Add Subscription command to start tracking spend."
              : `Nothing matches “${searchText}”.`
          }
        />
      ) : null}

      {results.map((sub) => (
        <List.Item
          key={String(sub.id)}
          icon={statusIcon(sub.status)}
          title={sub.name}
          subtitle={[
            formatPrice(sub.price, sub.currency),
            sub.cycle.replace("-", " "),
            sub.tags.join(", "),
          ]
            .filter((part) => part !== "")
            .join(" · ")}
          accessories={[
            { text: statusLabel(sub.status) },
            ...(sub.planTier ? [{ text: sub.planTier }] : []),
          ]}
          actions={
            <SubscriptionActions
              sub={sub}
              onChanged={load}
              onEdit={() => openEdit(sub)}
              onOpen={() => openDetail(sub)}
            />
          }
        />
      ))}
    </List>
  );
}
