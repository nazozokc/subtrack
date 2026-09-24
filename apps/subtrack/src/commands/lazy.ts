/**
 * Lazy command definition loaders.
 *
 * The barrel (commands/index.ts) evaluates all 50+ command definitions on
 * import. Startup instead dispatches on the first positional token and loads
 * only the matched command's definition module. The map below must mirror the
 * barrel's subCommands exactly — parity is asserted in
 * src/__tests__/commands-lazy.test.ts.
 */

import type { Command } from "../cli/types.ts"

export const commandLoaders: Record<string, () => Promise<Command<any>>> = {
  // core.ts
  list: () => import("./core.ts").then((m) => m.listCommand),
  add: () => import("./core.ts").then((m) => m.addCommand),
  edit: () => import("./core.ts").then((m) => m.editCommand),
  delete: () => import("./core.ts").then((m) => m.deleteCommand),
  cancel: () => import("./core.ts").then((m) => m.cancelCommand),
  clone: () => import("./core.ts").then((m) => m.cloneCommand),
  archive: () => import("./core.ts").then((m) => m.archiveCommand),
  unarchive: () => import("./core.ts").then((m) => m.unarchiveCommand),
  search: () => import("./core.ts").then((m) => m.searchCommand),
  // tag.ts
  tags: () => import("./tag.ts").then((m) => m.tagsCommand),
  tag: () => import("./tag.ts").then((m) => m.tagCommand),
  // trial.ts
  trial: () => import("./trial.ts").then((m) => m.trialCommand),
  // bulk.ts
  bulk: () => import("./bulk.ts").then((m) => m.bulkCommand),
  // io.ts
  export: () => import("./io.ts").then((m) => m.exportCommand),
  import: () => import("./io.ts").then((m) => m.importCommand),
  // backup.ts
  backup: () => import("./backup.ts").then((m) => m.backupCommand),
  restore: () => import("./backup.ts").then((m) => m.restoreCommand),
  // config.ts
  config: () => import("./config.ts").then((m) => m.configCommand),
  // usage.ts
  usage: () => import("./usage.ts").then((m) => m.usageCommand),
  // report.ts
  summary: () => import("./report.ts").then((m) => m.summaryCommand),
  payment: () => import("./report.ts").then((m) => m.paymentCommand),
  upcoming: () => import("./report.ts").then((m) => m.upcomingCommand),
  analytics: () => import("./report.ts").then((m) => m.analyticsCommand),
  compare: () => import("./report.ts").then((m) => m.compareCommand),
  calendar: () => import("./report.ts").then((m) => m.calendarCommand),
  forecast: () => import("./report.ts").then((m) => m.forecastCommand),
  history: () => import("./report.ts").then((m) => m.historyCommand),
  notify: () => import("./report.ts").then((m) => m.notifyCommand),
  timeline: () => import("./report.ts").then((m) => m.timelineCommand),
  optimize: () => import("./report.ts").then((m) => m.optimizeCommand),
  stats: () => import("./report.ts").then((m) => m.statsCommand),
  budget: () => import("./report.ts").then((m) => m.budgetCommand),
  report: () => import("./report.ts").then((m) => m.reportCommand),
  // misc.ts
  mcp: () => import("./misc.ts").then((m) => m.mcpCommand),
  profile: () => import("./misc.ts").then((m) => m.profileCommand),
  audit: () => import("./misc.ts").then((m) => m.auditCommand),
  maintenance: () => import("./misc.ts").then((m) => m.maintenanceCommand),
  cleanup: () => import("./misc.ts").then((m) => m.cleanupCommand),
  currency: () => import("./misc.ts").then((m) => m.currencyCommand),
  dedupe: () => import("./misc.ts").then((m) => m.dedupeCommand),
  // suggest.ts
  suggest: () => import("./suggest.ts").then((m) => m.suggestCommand),
  // features.ts
  pause: () => import("./features.ts").then((m) => m.pauseCommand),
  resume: () => import("./features.ts").then((m) => m.resumeCommand),
  renew: () => import("./features.ts").then((m) => m.renewCommand),
  review: () => import("./features.ts").then((m) => m.reviewCommand),
  yearly: () => import("./features.ts").then((m) => m.yearlyCommand),
  check: () => import("./features.ts").then((m) => m.checkCommand),
  changes: () => import("./features.ts").then((m) => m.changesCommand),
  receipt: () => import("./features.ts").then((m) => m.receiptCommand),
  template: () => import("./features.ts").then((m) => m.templateCommand),
}