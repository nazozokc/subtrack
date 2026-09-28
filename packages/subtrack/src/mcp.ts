/**
 * MCP module entry point.
 * Re-exports the public API from the mcp/ directory.
 * This barrel file preserves backward compatibility for consumers
 * that import from "./mcp.ts".
 */

export { formatDate as formatDateISO } from "@subtrack/lib/date"
export { startMcpServer } from "./mcp/index.ts"