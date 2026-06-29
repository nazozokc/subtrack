export type Screen =
  | "list"
  | "add"
  | "edit"
  | "delete"
  | "detail"
  | "reports"
  | "config"
  | "tools"
  | "help"

export type Mode = "NORMAL" | "COMMAND"

export type Focus = "sidebar" | "content"

export type ReportsTab =
  | "summary"
  | "payment"
  | "upcoming"
  | "analytics"
  | "compare"
  | "forecast"

export type ToolsTab =
  | "export"
  | "import"
  | "backup"
  | "restore"
  | "usage"

export type SidebarSection = "data" | "reports" | "system"

export type SidebarItem = {
  screen: Screen
  label: string
  section: SidebarSection
}

/** Sidebar panel width in characters */
export const SIDEBAR_WIDTH = 20

export const SIDEBAR_ITEMS: SidebarItem[] = [
  // Data
  { screen: "list", label: "List", section: "data" },
  { screen: "add", label: "Add", section: "data" },
  // Reports
  { screen: "reports", label: "Reports", section: "reports" },
  // System
  { screen: "config", label: "Config", section: "system" },
  { screen: "tools", label: "Tools", section: "system" },
  { screen: "help", label: "Help", section: "system" },
]

export const SCREEN_TITLES: Record<Screen, string> = {
  list: "Subscription List",
  add: "Add Subscription",
  edit: "Edit Subscription",
  delete: "Delete Subscription",
  detail: "Subscription Detail",
  reports: "Reports",
  config: "Configuration",
  tools: "Tools",
  help: "Help",
}

export const REPORT_TAB_LABELS: Record<ReportsTab, string> = {
  summary: "Summary",
  payment: "Payment",
  upcoming: "Upcoming",
  analytics: "Analytics",
  compare: "Compare",
  forecast: "Forecast",
}

export const TOOLS_TAB_LABELS: Record<ToolsTab, string> = {
  export: "Export",
  import: "Import",
  backup: "Backup",
  restore: "Restore",
  usage: "Usage",
}

export const REPORT_TABS: ReportsTab[] = [
  "summary",
  "payment",
  "upcoming",
  "analytics",
  "compare",
  "forecast",
]

export const TOOLS_TABS: ToolsTab[] = [
  "export",
  "import",
  "backup",
  "restore",
  "usage",
]
