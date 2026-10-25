//! MCP (Model Context Protocol) stdio server — port of
//! `packages/subtrack/src/mcp/*` (protocol, transport, tools, security,
//! handlers). Implements JSON-RPC 2.0 over a newline-delimited (NDJSON) or
//! `Content-Length:`-framed byte stream read from stdin; one single-line
//! response per request is written to stdout.

pub mod mcp;
