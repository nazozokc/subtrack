import { Box, Text } from "ink"
import Gradient from "ink-gradient"
import { useMemo } from "react"
import { getSubscriptions } from "../../db.ts"
import { formatPrice } from "../../price.ts"
import { useTui } from "../context/app-context.tsx"
import { REPORT_TAB_LABELS, REPORT_TABS } from "../types.ts"
import type { ReportsTab } from "../types.ts"
import { OCCURRENCES_PER_YEAR, type Cycle } from "../../types.ts"

// ── Helpers ───────────────────────────────────────────

function monthlyFactor(cycle: string): number {
  return (OCCURRENCES_PER_YEAR[cycle as Cycle] ?? 12) / 12
}

function yearlyFactor(cycle: string): number {
  return OCCURRENCES_PER_YEAR[cycle as Cycle] ?? 12
}

// ── Bar Chart Component ──────────────────────────────

type BarItem = {
  label: string
  value: number
  color?: string
}

function BarChart({ items, maxWidth = 16, currency = "USD" }: { items: BarItem[]; maxWidth?: number; currency?: string }) {
  if (items.length === 0) return null

  const maxValue = Math.max(...items.map((i) => i.value), 1)

  return (
    <Box flexDirection="column" gap={0}>
      {items.map((item) => {
        const ratio = item.value / maxValue
        const filled = Math.round(ratio * maxWidth)
        const empty = maxWidth - filled
        const bar = "█".repeat(filled) + "░".repeat(Math.max(0, empty))

        return (
          <Box key={item.label} minHeight={1}>
            <Box width={18}>
              <Text bold wrap="truncate-end">
                {item.label.padEnd(18).slice(0, 18)}
              </Text>
            </Box>
            <Box width={maxWidth + 2}>
              <Text color={item.color ?? "cyan"}>{bar}</Text>
            </Box>
            <Box width={14} justifyContent="flex-end">
              <Text bold color="yellow">
                {formatPrice(item.value, currency)}
              </Text>
            </Box>
          </Box>
        )
      })}
    </Box>
  )
}

// ── Tab bar ───────────────────────────────────────────

function TabBar({ activeTab }: { activeTab: ReportsTab }) {
  const barWidth = REPORT_TABS.length * 20

  return (
    <Box flexDirection="column" marginBottom={1}>
      <Box width={barWidth}>
        {REPORT_TABS.map((tab, i) => {
          const isActive = tab === activeTab
          const label = `${REPORT_TAB_LABELS[tab]}`
          const paddedLabel = ` ${label.padEnd(16)} `
          const separator = i < REPORT_TABS.length - 1 ? (
            <Text dimColor>│</Text>
          ) : null

          return (
            <Box key={tab}>
              {isActive ? (
                <Text bold color="cyan" inverse>{paddedLabel}</Text>
              ) : (
                <Text dimColor>{paddedLabel}</Text>
              )}
              {separator}
            </Box>
          )
        })}
      </Box>
      <Text dimColor>
        {"─".repeat(barWidth)}
      </Text>
    </Box>
  )
}

// ── Summary tab ───────────────────────────────────────

function SummaryTab() {
  const subs = useMemo(() => getSubscriptions(), [])

  const totalActive = subs.filter((s) => s.status === "active").length
  const totalPaused = subs.filter((s) => s.status === "paused").length
  const totalCancelled = subs.filter((s) => s.status === "cancelled").length

  const monthlyByCurrency = useMemo(() => {
    const map = new Map<string, number>()
    for (const sub of subs) {
      if (sub.status === "cancelled") continue
      const factor = monthlyFactor(sub.cycle)
      map.set(sub.currency, (map.get(sub.currency) ?? 0) + Math.round(sub.price * factor))
    }
    return map
  }, [subs])

  return (
    <Box flexDirection="column">
      <Box marginBottom={1} flexDirection="column">
        <Box marginBottom={1}>
          <Text bold underline>At a Glance</Text>
        </Box>
        <Box><Box width={20}><Text dimColor>Total subscriptions:</Text></Box><Text bold>{subs.length}</Text></Box>
        <Box><Box width={20}><Text dimColor>Active:</Text></Box><Text bold color="green">{totalActive}</Text></Box>
        <Box><Box width={20}><Text dimColor>Paused:</Text></Box><Text bold color="yellow">{totalPaused}</Text></Box>
        <Box><Box width={20}><Text dimColor>Cancelled:</Text></Box><Text bold color="red">{totalCancelled}</Text></Box>
      </Box>

      {monthlyByCurrency.size > 0 && (
        <Box flexDirection="column" marginTop={1}>
          <Box marginBottom={1}>
            <Text bold underline>Monthly Cost</Text>
          </Box>
          {Array.from(monthlyByCurrency.entries()).map(([currency, total]) => (
            <Box key={currency} flexDirection="column">
              <Box marginBottom={1}>
                <Text bold color="cyan">{currency}</Text>
                <Text dimColor>  {formatPrice(total, currency)}/mo</Text>
              </Box>
              <BarChart
                items={[{ label: "Total", value: total }]}
                maxWidth={12}
                currency={currency}
              />
            </Box>
          ))}
        </Box>
      )}
    </Box>
  )
}

// ── Payment tab ───────────────────────────────────────

function PaymentTab() {
  const subs = useMemo(() => getSubscriptions(), [])
  const active = subs.filter((s) => s.status === "active")

  const monthlyByCurrency = useMemo(() => {
    const map = new Map<string, number>()
    for (const sub of active) {
      const factor = monthlyFactor(sub.cycle)
      map.set(sub.currency, (map.get(sub.currency) ?? 0) + Math.round(sub.price * factor))
    }
    return Array.from(map.entries()).sort((a, b) => b[1] - a[1])
  }, [active])

  const yearlyByCurrency = useMemo(() => {
    const map = new Map<string, number>()
    for (const sub of active) {
      const factor = yearlyFactor(sub.cycle)
      map.set(sub.currency, (map.get(sub.currency) ?? 0) + Math.round(sub.price * factor))
    }
    return Array.from(map.entries()).sort((a, b) => b[1] - a[1])
  }, [active])

  return (
    <Box flexDirection="column">
      <Box marginBottom={1}><Text bold underline>Payment Totals</Text></Box>
      {active.length === 0 ? (
        <Text dimColor>No active subscriptions</Text>
      ) : (
        <>
          <Box flexDirection="column" marginBottom={1}>
            <Text bold color="cyan">Monthly</Text>
            {monthlyByCurrency.map(([currency, total]) => (
              <Box key={currency} flexDirection="column" marginBottom={1}>
                <Box>
                  <Text dimColor>{currency}</Text>
                  <Text bold color="yellow">  {formatPrice(total, currency)}</Text>
                </Box>
                <BarChart
                  items={[{ label: "Monthly", value: total }]}
                  maxWidth={12}
                  currency={currency}
                />
              </Box>
            ))}
          </Box>
          <Box flexDirection="column">
            <Text bold color="cyan">Yearly</Text>
            {yearlyByCurrency.map(([currency, total]) => (
              <Box key={currency} flexDirection="column" marginBottom={1}>
                <Box>
                  <Text dimColor>{currency}</Text>
                  <Text bold color="yellow">  {formatPrice(total, currency)}</Text>
                </Box>
                <BarChart
                  items={[{ label: "Yearly", value: total }]}
                  maxWidth={12}
                  currency={currency}
                />
              </Box>
            ))}
          </Box>
        </>
      )}
    </Box>
  )
}

// ── Upcoming tab ──────────────────────────────────────

function computeNextBill(day: number, from: Date, until: Date): Date | null {
  const year = from.getFullYear()
  const month = from.getMonth()
  const lastDay = new Date(year, month + 1, 0).getDate()
  const clampedDay = Math.min(day, lastDay)
  let d = new Date(year, month, clampedDay)
  if (d < from) {
    const nextLastDay = new Date(year, month + 2, 0).getDate()
    d = new Date(year, month + 1, Math.min(day, nextLastDay))
  }
  if (d <= until) return d
  return null
}

function UpcomingTab() {
  const subs = useMemo(() => getSubscriptions(), [])
  const today = new Date()
  const in7Days = new Date(today.getTime() + 7 * 86400000)

  const upcoming = useMemo(() => {
    return subs
      .filter((s) => s.status === "active" && s.billingDay !== null)
      .map((s) => ({
        ...s,
        nextBill: computeNextBill(s.billingDay!, today, in7Days),
      }))
      .filter((s) => s.nextBill !== null)
      .sort((a, b) => a.nextBill!.getTime() - b.nextBill!.getTime())
  }, [subs])

  return (
    <Box flexDirection="column">
      <Box marginBottom={1}><Text bold underline>Upcoming Bills (next 7 days)</Text></Box>
      {upcoming.length === 0 ? (
        <Text dimColor>No bills due in the next 7 days</Text>
      ) : (
        upcoming.map((s) => (
          <Box key={s.id}>
            <Box width={24}><Text bold wrap="truncate-end">{s.name}</Text></Box>
            <Box width={16}>
              <Text color="yellow">
                {s.nextBill!.toLocaleDateString("en-US", { month: "short", day: "numeric" })}
              </Text>
            </Box>
            <Box width={14}><Text>{formatPrice(s.price, s.currency)}</Text></Box>
            <Text dimColor>/{s.cycle}</Text>
          </Box>
        ))
      )}
    </Box>
  )
}

// ── Analytics tab ─────────────────────────────────────

function AnalyticsTab() {
  const subs = useMemo(() => getSubscriptions(), [])
  const activeSubs = subs.filter((s) => s.status === "active")

  const tagFreq = useMemo(() => {
    const map = new Map<string, number>()
    for (const sub of activeSubs) {
      for (const tag of sub.tags) {
        map.set(tag, (map.get(tag) ?? 0) + 1)
      }
    }
    return Array.from(map.entries()).sort((a, b) => b[1] - a[1])
  }, [activeSubs])

  const cycleCost = useMemo(() => {
    const map = new Map<string, { cycle: string; currency: string; total: number }>()
    for (const sub of activeSubs) {
      const key = `${sub.cycle}::${sub.currency}`
      const existing = map.get(key) ?? { cycle: sub.cycle, currency: sub.currency, total: 0 }
      existing.total += sub.price
      map.set(key, existing)
    }
    return Array.from(map.values()).sort((a, b) => b.total - a.total)
  }, [activeSubs])

  const sortedByPrice = useMemo(
    () => [...activeSubs].sort((a, b) => b.price - a.price).slice(0, 5),
    [activeSubs],
  )

  return (
    <Box flexDirection="column">
      <Box marginBottom={1}><Text bold underline>Analytics</Text></Box>
      {activeSubs.length === 0 ? (
        <Text dimColor>No active subscriptions</Text>
      ) : (
        <>
          <Box flexDirection="column" marginBottom={1}>
            <Text bold color="cyan">Most Expensive (top 5)</Text>
            <Box marginTop={1}>
              <BarChart
                items={sortedByPrice.map((sub) => ({
                  label: sub.name,
                  value: sub.price,
                  color: "yellow",
                }))}
                maxWidth={16}
                currency={sortedByPrice[0]?.currency ?? "USD"}
              />
            </Box>
            <Box marginTop={1}>
              {sortedByPrice.map((sub, i) => (
                <Box key={sub.id}>
                  <Box width={4}><Text dimColor>{(i + 1) + "."}</Text></Box>
                  <Box width={24}><Text bold wrap="truncate-end">{sub.name}</Text></Box>
                  <Text dimColor>/{sub.cycle}</Text>
                </Box>
              ))}
            </Box>
          </Box>

          <Box flexDirection="column" marginBottom={1}>
            <Text bold color="cyan">Cost by Cycle</Text>
            <Box marginTop={1}>
              {/* Group by currency for display */}
              {Array.from(new Set(cycleCost.map((c) => c.currency))).map((currency) => (
                <Box key={currency} flexDirection="column" marginBottom={1}>
                  <Text dimColor>{currency}:</Text>
                  <BarChart
                    items={cycleCost
                      .filter((c) => c.currency === currency)
                      .map((c) => ({ label: c.cycle, value: c.total }))}
                    maxWidth={16}
                    currency={currency}
                  />
                </Box>
              ))}
            </Box>
          </Box>

          <Box flexDirection="column">
            <Text bold color="cyan">Top Tags</Text>
            {tagFreq.length === 0 ? (
              <Text dimColor>No tags</Text>
            ) : (
              tagFreq.slice(0, 10).map(([tag, count]) => (
                <Box key={tag}>
                  <Box width={20}><Text bold wrap="truncate-end">{tag}</Text></Box>
                  <Text dimColor>{count} subscription{count !== 1 ? "s" : ""}</Text>
                </Box>
              ))
            )}
          </Box>
        </>
      )}
    </Box>
  )
}

// ── Compare tab ───────────────────────────────────────

function CompareTab() {
  const subs = useMemo(() => getSubscriptions(), [])
  const active = subs.filter((s) => s.status === "active")

  const monthlyTotal = useMemo(() => {
    const map = new Map<string, number>()
    for (const sub of active) {
      const factor = monthlyFactor(sub.cycle)
      map.set(sub.currency, (map.get(sub.currency) ?? 0) + sub.price * factor)
    }
    for (const [currency, total] of map) {
      map.set(currency, Math.round(total))
    }
    return map
  }, [active])

  return (
    <Box flexDirection="column">
      <Box marginBottom={1}><Text bold underline>Period Comparison</Text></Box>
      {active.length === 0 ? (
        <Text dimColor>No active subscriptions</Text>
      ) : (
        Array.from(monthlyTotal.entries()).map(([currency, total]) => (
          <Box key={currency} flexDirection="column" marginBottom={1}>
            <Box marginBottom={1}>
              <Text bold color="cyan">{currency}/month</Text>
              <Text bold color="yellow">  {formatPrice(total, currency)}</Text>
            </Box>
            <BarChart
              items={[{ label: currency, value: total }]}
              maxWidth={16}
              currency={currency}
            />
          </Box>
        ))
      )}
    </Box>
  )
}

// ── Forecast tab ──────────────────────────────────────

function ForecastTab() {
  const subs = useMemo(() => getSubscriptions(), [])
  const active = subs.filter((s) => s.status === "active")

  const monthly = useMemo(() => {
    const map = new Map<string, number>()
    for (const sub of active) {
      const f = monthlyFactor(sub.cycle)
      map.set(sub.currency, (map.get(sub.currency) ?? 0) + Math.round(sub.price * f))
    }
    return map
  }, [active])

  const yearly = useMemo(() => {
    const map = new Map<string, number>()
    for (const sub of active) {
      const f = yearlyFactor(sub.cycle)
      map.set(sub.currency, (map.get(sub.currency) ?? 0) + Math.round(sub.price * f))
    }
    return map
  }, [active])

  return (
    <Box flexDirection="column">
      <Box marginBottom={1}><Text bold underline>Spending Forecast</Text></Box>
      {active.length === 0 ? (
        <Text dimColor>No active subscriptions</Text>
      ) : (
        <>
          <Box marginBottom={1} flexDirection="column">
            <Text bold color="cyan">Monthly Costs</Text>
            <Box marginTop={1}>
              {Array.from(monthly.entries()).map(([c, t]) => (
                <Box key={c} flexDirection="column" marginBottom={1}>
                  <Box>
                    <Text dimColor>{c}:</Text>
                    <Text bold color="yellow">  {formatPrice(t, c)}</Text>
                  </Box>
                  <BarChart items={[{ label: "Monthly", value: t }]} maxWidth={12} currency={c} />
                </Box>
              ))}
            </Box>
          </Box>
          <Box flexDirection="column">
            <Text bold color="cyan">Yearly Total</Text>
            <Box marginTop={1}>
              {Array.from(yearly.entries()).map(([c, t]) => (
                <Box key={c} flexDirection="column" marginBottom={1}>
                  <Box>
                    <Text dimColor>{c}:</Text>
                    <Text bold color="yellow">  {formatPrice(t, c)}</Text>
                  </Box>
                  <BarChart items={[{ label: "Yearly", value: t }]} maxWidth={12} currency={c} />
                </Box>
              ))}
            </Box>
          </Box>
        </>
      )}
    </Box>
  )
}

// ── Main component ────────────────────────────────────

export function ReportsScreen() {
  const { state } = useTui()

  return (
    <Box flexDirection="column" flexGrow={1} paddingX={1} paddingY={1}>
      <Box marginBottom={1} flexDirection="column">
        <Box>
        <Gradient name="pastel">
          <Text bold inverse>
            {" Reports "}
          </Text>
        </Gradient>
        </Box>
      </Box>

      <TabBar activeTab={state.reportsTab} />

      {state.reportsTab === "summary" && <SummaryTab />}
      {state.reportsTab === "payment" && <PaymentTab />}
      {state.reportsTab === "upcoming" && <UpcomingTab />}
      {state.reportsTab === "analytics" && <AnalyticsTab />}
      {state.reportsTab === "compare" && <CompareTab />}
      {state.reportsTab === "forecast" && <ForecastTab />}
    </Box>
  )
}
