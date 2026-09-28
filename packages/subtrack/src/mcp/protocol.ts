/**
 * MCP protocol constants: server identity, the supported protocol version,
 * JSON-RPC error codes, and the `initialize` handshake response.
 */

export const SERVER_NAME = "subtrack-mcp"
export const SERVER_VERSION = "1.0.0"
export const DEFAULT_PROTOCOL_VERSION = "2024-11-05"

export const JSONRPC_ERROR = {
  PARSE: -32700,
  INVALID_REQUEST: -32600,
  METHOD_NOT_FOUND: -32601,
  INVALID_PARAMS: -32602,
} as const

/**
 * Build the `initialize` result. The client's protocol version is echoed
 * back verbatim (no capability negotiation beyond `tools`), falling back to
 * the default when the client sends none.
 */
export function buildInitializeResult(params: unknown): {
  protocolVersion: string
  capabilities: { tools: {} }
  serverInfo: { name: string; version: string }
} {
  const p = params as Record<string, unknown> | undefined
  const protocolVersion =
    typeof p?.protocolVersion === "string"
      ? p.protocolVersion
      : DEFAULT_PROTOCOL_VERSION
  return {
    protocolVersion,
    capabilities: { tools: {} },
    serverInfo: { name: SERVER_NAME, version: SERVER_VERSION },
  }
}