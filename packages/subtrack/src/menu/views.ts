/**
 * "View & Search" and "Reports" sub-menus — everything that displays
 * subscription data.
 */

import { tagRepository } from "../application/index.ts"
import { select, checkbox } from "../prompts.ts"
import { consola } from "@subtrack/lib/logger"
import { BACK, pickSubscription, pickPeriod } from "./shared.ts"
import { handleList, handleTags } from "../subscription/core.ts"
import { handleSearch } from "../search.ts"
import { handleUpcoming } from "../upcoming.ts"
import { handleCalendar } from "../calendar.ts"
import { handleHistory } from "../history.ts"
import { handleTimeline } from "../timeline.ts"
import { handleStats } from "../stats.ts"
import { handleSummary, handlePayment } from "../payment.ts"
import { handleAnalytics } from "../analytics.ts"
import { handleCompare } from "../compare.ts"
import { handleForecast } from "../forecast.ts"
import { handleOptimize } from "../optimize.ts"
import { handleNotify } from "../notify.ts"

export async function runViewMenu(): Promise<void> {
  while (true) {
    const action = await select({
      message: "view & search",
      pageSize: 10,
      choices: [
        { name: "List", description: "Show all subscriptions", value: "list" },
        { name: "Search", description: "Search by name, notes, or tags", value: "search" },
        { name: "Tag filter", description: "Filter subscriptions by tags (AND)", value: "tags" },
        { name: "Upcoming", description: "Upcoming bills within 7 days", value: "upcoming" },
        { name: "Calendar", description: "Monthly calendar with billing days", value: "calendar" },
        { name: "History", description: "Price change history", value: "history" },
        { name: "Timeline", description: "Monthly spending timeline", value: "timeline" },
        { name: "Stats", description: "Database statistics", value: "stats" },
        BACK,
      ],
    })

    switch (action) {
      case "list": await handleList({}); break
      case "search": await handleSearch(undefined); break
      case "tags": await runTagFilter(); break
      case "upcoming": await handleUpcoming(); break
      case "calendar": await handleCalendar({}); break
      case "history": {
        const id = await pickSubscription("select subscription")
        if (id !== null) await handleHistory(id)
        break
      }
      case "timeline": await handleTimeline(); break
      case "stats": await handleStats(); break
      case "back": return
    }
  }
}

async function runTagFilter(): Promise<void> {
  const tags = tagRepository.list()
  if (tags.length === 0) {
    consola.info("No tags found")
    return
  }
  const selected = await checkbox({
    message: "select tags (AND logic)",
    pageSize: 10,
    choices: tags.map((t) => ({ name: t, value: t })),
  })
  if (selected.length === 0) return
  await handleTags(selected)
}

export async function runReportMenu(): Promise<void> {
  while (true) {
    const action = await select({
      message: "reports",
      pageSize: 10,
      choices: [
        { name: "Summary", description: "Subscription summary statistics", value: "summary" },
        { name: "Payment", description: "Payment totals for a period", value: "payment" },
        { name: "Analytics", description: "Detailed subscription analytics", value: "analytics" },
        { name: "Compare", description: "Compare spending with previous period", value: "compare" },
        { name: "Forecast", description: "Spending forecast with what-if scenarios", value: "forecast" },
        { name: "Optimize", description: "Cost optimization suggestions", value: "optimize" },
        { name: "Notify", description: "Desktop notification for upcoming bills", value: "notify" },
        BACK,
      ],
    })

    switch (action) {
      case "summary": await handleSummary(); break
      case "payment": await handlePayment(await pickPeriod("select period"), {}); break
      case "analytics": handleAnalytics(); break
      case "compare": await handleCompare(await pickPeriod("select period")); break
      case "forecast": await handleForecast({}); break
      case "optimize": await handleOptimize(); break
      case "notify": await handleNotify(); break
      case "back": return
    }
  }
}