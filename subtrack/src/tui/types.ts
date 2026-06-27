export type Screen =
  | "list"
  | "search"
  | "add"
  | "edit"
  | "delete"
  | "tags"
  | "tag-manage"
  | "trials"
  | "trial-add"
  | "trial-expiring"
  | "bulk"
  | "summary"
  | "payment"
  | "upcoming"
  | "analytics"
  | "compare"
  | "forecast"
  | "config"
  | "usage"
  | "export"
  | "import"
  | "backup"
  | "restore"
  | "help"

export type Mode = "NORMAL" | "COMMAND" | "SEARCH"

export type Focus = "sidebar" | "content"

export type SidebarSection = "data" | "tags-trials-bulk" | "reports" | "system"

export type SidebarItem = {
  screen: Screen
  label: string
  icon: string
  section: SidebarSection
}

export const SIDEBAR_ITEMS: SidebarItem[] = [
  // Data
  { screen: "list", label: "List", icon: "📋", section: "data" },
  { screen: "search", label: "Find", icon: "🔍", section: "data" },
  { screen: "add", label: "Add", icon: "➕", section: "data" },
  { screen: "edit", label: "Edit", icon: "✏️", section: "data" },
  { screen: "delete", label: "Delete", icon: "🗑️", section: "data" },
  // Tags / Trials / Bulk
  { screen: "tags", label: "Tags", icon: "🏷️", section: "tags-trials-bulk" },
  { screen: "trials", label: "Trials", icon: "🆕", section: "tags-trials-bulk" },
  { screen: "bulk", label: "Bulk", icon: "📦", section: "tags-trials-bulk" },
  // Reports
  { screen: "summary", label: "Summary", icon: "📊", section: "reports" },
  { screen: "payment", label: "Payment", icon: "💰", section: "reports" },
  { screen: "upcoming", label: "Upcoming", icon: "📅", section: "reports" },
  { screen: "analytics", label: "Analytics", icon: "📈", section: "reports" },
  { screen: "compare", label: "Compare", icon: "↔️", section: "reports" },
  { screen: "forecast", label: "Forecast", icon: "🔮", section: "reports" },
  // System
  { screen: "config", label: "Config", icon: "⚙️", section: "system" },
  { screen: "usage", label: "Usage", icon: "📊", section: "system" },
  { screen: "export", label: "Export", icon: "📤", section: "system" },
  { screen: "import", label: "Import", icon: "📥", section: "system" },
  { screen: "backup", label: "Backup", icon: "💾", section: "system" },
  { screen: "restore", label: "Restore", icon: "🔄", section: "system" },
  { screen: "help", label: "Help", icon: "❓", section: "system" },
]

export const SCREEN_TITLES: Record<Screen, string> = {
  list: "Subscription List",
  search: "Search Subscriptions",
  add: "Add Subscription",
  edit: "Edit Subscription",
  delete: "Delete Subscriptions",
  tags: "Tags Filter",
  "tag-manage": "Tag Management",
  trials: "Trial List",
  "trial-add": "Add Trial",
  "trial-expiring": "Expiring Trials",
  bulk: "Bulk Operations",
  summary: "Summary",
  payment: "Payment Totals",
  upcoming: "Upcoming Bills",
  analytics: "Analytics",
  compare: "Compare Spending",
  forecast: "Spending Forecast",
  config: "Configuration",
  usage: "LLM Usage",
  export: "Export",
  import: "Import",
  backup: "Backup Database",
  restore: "Restore Database",
  help: "Help",
}
