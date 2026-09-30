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
] as const;

/** Config keys the CLI itself advertises for each period. */
const SET_COMMANDS = {
  monthly: "subtrack config set monthlyBudget 50",
  yearly: "subtrack config set yearlyBudget 600",
} as const;

export default function Budget(): JSX.Element {
  const [period, setPeriod] =
    useState<(typeof PERIODS)[number]["value"]>("monthly");
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
      <ActionPanel.Submenu title="Change Period…" icon={Icon.Calendar}>
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
          SET_COMMANDS[period],
          "```",
        ].join("\n")}
        actions={actions}
      />
    );
  }

  // The CLI already reports `spending`, `remaining`, and `over` for the
  // resolved budget, so nothing here has to be recomputed.
  const currency = data.currency;
  const limit = data.budget;
  const remaining = data.remaining;
  const used = limit <= 0 ? null : Math.min((data.spending / limit) * 100, 100);

  const barWidth = 30;
  const filled = used === null ? 0 : Math.round((used / 100) * barWidth);
  const bar = `${"█".repeat(filled)}${"░".repeat(barWidth - filled)}`;

  return (
    <Detail
      markdown={[
        "# Budget",
        "",
        data.over ? "## ⚠️ Over budget" : "## On track",
        "",
        `${formatPrice(data.spending, currency)} of ${limit <= 0 ? "no limit" : formatPrice(limit, currency)}`,
        "",
        limit <= 0 ? "" : `\n${bar}\n`,
        used === null ? "" : `**${used.toFixed(0)}% used**`,
        "",
        remaining >= 0
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
