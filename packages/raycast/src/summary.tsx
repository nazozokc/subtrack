import { useCallback, useEffect, useState } from "react";
import type { JSX } from "react";
import { Action, ActionPanel, Detail, Icon, showHUD } from "@raycast/api";
import { ErrorDetail } from "./lib/errors";
import { cycleLabel, formatPrice, statusLabel } from "./lib/format";
import { payment, summary } from "./lib/subtrack";
import type { PaymentData, SummaryData } from "./lib/types";

const PERIODS = [
  { title: "Weekly", value: "weekly" },
  { title: "Every 2 Weeks", value: "bi-weekly" },
  { title: "Monthly", value: "monthly" },
  { title: "Quarterly", value: "quarterly" },
  { title: "Every 6 Months", value: "semi-annual" },
  { title: "Yearly", value: "yearly" },
];

export default function Summary(): JSX.Element {
  const [period, setPeriod] = useState("monthly");
  const [summaryData, setSummaryData] = useState<SummaryData | null>(null);
  const [paymentData, setPaymentData] = useState<PaymentData | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<unknown>(null);

  const load = useCallback(async () => {
    setIsLoading(true);
    try {
      const [s, p] = await Promise.all([summary(), payment(period)]);
      setSummaryData(s);
      setPaymentData(p);
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

  if (isLoading || summaryData === null) {
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

  const currencyTotals = Object.entries(summaryData.monthlyByCurrency);
  const topTags = Object.entries(summaryData.monthlyByTag).sort(
    (a, b) => b[1].count - a[1].count,
  );
  const periodLabel = PERIODS.find((p) => p.value === period)?.title ?? period;

  const total = currencyTotals
    .map(([c, v]) => `**${formatPrice(v, c)}**`)
    .join("  \n");

  const tagSection = topTags.slice(0, 12).map(([tag, data]) => {
    const amount = Object.entries(data.monthly)
      .map(([c, v]) => formatPrice(v, c))
      .join(" + ");
    return `- \`#${tag}\` — ${data.count} · ${amount}`;
  });

  const rows = paymentData?.subscriptions ?? [];
  const table = [
    "| Subscription | Cycle | Status | Amount |",
    "| --- | --- | --- | --- |",
    ...rows
      .slice(0, 20)
      .map(
        (r) =>
          `| ${r.name} | ${cycleLabel(r.cycle)} | ${statusLabel(r.status)} | ${formatPrice(r.periodPrice, r.currency)} |`,
      ),
  ].join("\n");

  const markdown = [
    "# Monthly Spend",
    "",
    `${summaryData.totalCount} active subscription${summaryData.totalCount === 1 ? "" : "s"}`,
    "",
    total,
    "",
    summaryData.mostExpensive
      ? `Most expensive: **${summaryData.mostExpensive.name}** at ${formatPrice(summaryData.mostExpensive.price, summaryData.mostExpensive.currency)}`
      : "",
    tagSection.length > 0
      ? ["", "## By tag", "", ...tagSection].join("\n")
      : "",
    rows.length > 0
      ? [
          "",
          `## Per ${periodLabel.toLowerCase()}`,
          "",
          table,
          rows.length > 20 ? `\n_Showing 20 of ${rows.length}._` : "",
        ].join("\n")
      : "",
  ]
    .filter((block) => block !== "")
    .join("\n");

  return (
    <Detail
      markdown={markdown}
      actions={
        <ActionPanel>
          <Action
            title="Refresh"
            icon={Icon.ArrowClockwise}
            onAction={async () => {
              await load();
              await showHUD("Refreshed");
            }}
          />
          <ActionPanel.Submenu
            title="Change Period"
            icon={Icon.Calendar}
            shortcut={{
              macOS: { modifiers: ["cmd", "shift"], key: "p" },
              Windows: { modifiers: ["ctrl", "shift"], key: "p" },
            }}
          >
            {PERIODS.map((p) => (
              <Action
                key={p.value}
                title={p.title}
                icon={p.value === period ? Icon.Check : Icon.Circle}
                onAction={() => setPeriod(p.value)}
              />
            ))}
          </ActionPanel.Submenu>
          <Action.CopyToClipboard
            title="Copy Total"
            content={currencyTotals
              .map(([c, v]) => formatPrice(v, c))
              .join(" / ")}
          />
        </ActionPanel>
      }
    />
  );
}
