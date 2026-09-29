import { useCallback, useEffect, useState } from "react";
import type { JSX } from "react";
import { Action, ActionPanel, Detail, Icon, showHUD } from "@raycast/api";
import { ErrorDetail } from "./lib/errors";
import { formatPrice } from "./lib/format";
import { budget } from "./lib/subtrack";
import type { BudgetData } from "./lib/types";

const PERIODS = [
  { title: "Monthly", value: "monthly" },
  { title: "Yearly", value: "yearly" },
];

function number(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

export default function Budget(): JSX.Element {
  const [period, setPeriod] = useState("monthly");
  const [data, setData] = useState<BudgetData | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<unknown>(null);

  const load = useCallback(async () => {
    setIsLoading(true);
    try {
      setData(await budget(period));
      setError(null);
    } catch (err) {
      setError(err);
    } finally {
      setIsLoading(false);
    }
  }, [period]);

  useEffect(() => {
    void load();
  }, [load]);

  if (error !== null) {
    return <ErrorDetail error={error} onRetry={load} />;
  }

  if (isLoading || data === null) {
    return (
      <Detail
        isLoading
        markdown="Loading…"
        actions={
          <ActionPanel>
            <Action
              title="Refresh"
              icon={Icon.ArrowClockwise}
              onAction={() => void load()}
            />
          </ActionPanel>
        }
      />
    );
  }

  const periodLabel = PERIODS.find((p) => p.value === period)?.title ?? period;
  const actions = (
    <ActionPanel>
      <Action
        title="Refresh"
        icon={Icon.ArrowClockwise}
        onAction={async () => {
          await load();
          await showHUD("Refreshed");
        }}
      />
      <ActionPanel.Submenu title="Change Period" icon={Icon.Calendar}>
        {PERIODS.map((p) => (
          <Action
            key={p.value}
            title={p.title}
            icon={p.value === period ? Icon.Check : Icon.Circle}
            onAction={() => setPeriod(p.value)}
          />
        ))}
      </ActionPanel.Submenu>
    </ActionPanel>
  );

  if (!data.set) {
    return (
      <Detail
        markdown={[
          "# Budget",
          "",
          `No ${periodLabel.toLowerCase()} budget is set.`,
          "",
          "Set one in your terminal:",
          "",
          "```",
          "subtrack config budget set --amount 50",
          "```",
        ].join("\n")}
        actions={actions}
      />
    );
  }

  const currency = data.currency ?? "USD";
  // The CLI has used several key names for these over time; accept whichever
  // the installed version emits rather than pinning one.
  const spent =
    number(data.total) ?? number(data.spent) ?? number(data.amount) ?? 0;
  const limit =
    number(data.limit) ?? number(data.budget) ?? number(data.budgetAmount);
  const remaining = limit === null ? null : limit - spent;
  const used =
    limit === null || limit === 0 ? null : Math.min((spent / limit) * 100, 100);
  const over = data.over ?? (limit !== null && spent > limit);

  const barWidth = 30;
  const filled = used === null ? 0 : Math.round((used / 100) * barWidth);
  const bar = `${"█".repeat(filled)}${"░".repeat(barWidth - filled)}`;

  return (
    <Detail
      markdown={[
        "# Budget",
        "",
        over ? "## ⚠️ Over budget" : "## On track",
        "",
        `${formatPrice(spent, currency)} of ${limit === null ? "no limit" : formatPrice(limit, currency)}`,
        "",
        limit === null ? "" : `\n${bar}\n`,
        limit === null
          ? ""
          : used === null
            ? ""
            : `**${used.toFixed(0)}% used**`,
        "",
        remaining === null
          ? ""
          : remaining >= 0
            ? `${formatPrice(remaining, currency)} left`
            : `${formatPrice(-remaining, currency)} over`,
        "",
        `Period: ${periodLabel}${data.budgetName ? ` · ${data.budgetName}` : ""}`,
      ]
        .filter((line) => line !== "")
        .join("\n")}
      actions={actions}
    />
  );
}
