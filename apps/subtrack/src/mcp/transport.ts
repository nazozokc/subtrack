/**
 * Minimal MCP stdio server implementing JSON-RPC 2.0 over a newline-delimited
 * (NDJSON) or Content-Length-framed byte stream. Only Node built-ins are used.
 *
 * Responsibility: byte-level framing and JSON-RPC envelope handling (routing,
 * error codes, idle/notification suppression). Tool business logic lives in
 * handlers.ts / security.ts.
 *
 * Framing is detected per message: an incoming line shaped like a
 * `Content-Length:` header switches to header-framed reading for that message.
 */

import type { Readable, Writable } from "node:stream"

import { callTool } from "./handlers.ts"
import {
  JSONRPC_ERROR,
  buildInitializeResult,
} from "./protocol.ts"
import { MAX_REQUEST_SIZE } from "./security.ts"
import { TOOLS } from "./tools.ts"

const CONTENT_LENGTH_RE = /^Content-Length:\s*(\d+)\s*$/im
const HEADER_LINE_RE = /^Content-Length:\s*(\d+)$/i

function sendError(id: string | number | null, code: number, message: string) {
  return { jsonrpc: "2.0", id, error: { code, message } }
}

/**
 * Start the MCP stdio server. Reads JSON-RPC messages from `stdin` and writes
 * one single-line response per request to `stdout`. Resolves once `stdin`
 * ends (EOF) or the stream errors.
 */
export function startTransport(stdin: Readable, stdout: Writable): Promise<void> {
  return new Promise<void>((resolve) => {
    let buffer = Buffer.alloc(0)
    let mode: "line" | "headers" | "body" = "line"
    let headerText = ""
    let bodyRemaining = -1
    let skipRemaining = 0
    let initialized = false
    let stopping = false

    function sendMessage(msg: Record<string, unknown>): void {
      try {
        stdout.write(`${JSON.stringify(msg)}\n`)
      } catch {
        stop()
      }
    }

    function sendResult(id: string | number, result: unknown): void {
      sendMessage({ jsonrpc: "2.0", id, result })
    }

    function sendOversizeError(): void {
      sendMessage(
        sendError(null, JSONRPC_ERROR.PARSE, `Request too large (max ${MAX_REQUEST_SIZE} bytes)`),
      )
    }

    function handleRawMessage(text: string): void {
      if (Buffer.byteLength(text, "utf8") > MAX_REQUEST_SIZE) {
        sendOversizeError()
        return
      }
      let msg: unknown
      try {
        msg = JSON.parse(text)
      } catch {
        sendMessage(sendError(null, JSONRPC_ERROR.PARSE, "Parse error"))
        return
      }
      handleMessage(msg)
    }

    function handleToolCall(id: string | number, msg: Record<string, unknown>): void {
      const params = msg.params as Record<string, unknown> | undefined
      const name = params?.name
      if (typeof name !== "string") {
        sendMessage(
          sendError(id, JSONRPC_ERROR.INVALID_PARAMS, "tools/call requires a string params.name"),
        )
        return
      }
      const rawArgs = params?.arguments
      const args: Record<string, unknown> | undefined =
        typeof rawArgs === "object" && rawArgs !== null
          ? (rawArgs as Record<string, unknown>)
          : undefined
      void callTool(name, args).then((response) => sendResult(id, response))
    }

    function handleMessage(msg: unknown): void {
      if (typeof msg !== "object" || msg === null) {
        sendMessage(sendError(null, JSONRPC_ERROR.INVALID_REQUEST, "Invalid Request"))
        return
      }
      const m = msg as Record<string, unknown>
      const rawId = m.id
      if (rawId === undefined || rawId === null) {
        // Notification — never reply (covers notifications/initialized, notifications/cancelled).
        return
      }
      if (typeof m.method !== "string") {
        sendMessage(sendError(null, JSONRPC_ERROR.INVALID_REQUEST, "Invalid Request"))
        return
      }
      if (typeof rawId !== "number" && typeof rawId !== "string") {
        sendMessage(sendError(null, JSONRPC_ERROR.INVALID_REQUEST, "Invalid Request"))
        return
      }
      const id = rawId
      if (!initialized && m.method !== "initialize") {
        sendMessage(
          sendError(id, JSONRPC_ERROR.INVALID_REQUEST, "Invalid Request: initialize required"),
        )
        return
      }
      switch (m.method) {
        case "initialize":
          initialized = true
          sendResult(id, buildInitializeResult(m.params))
          break
        case "ping":
          sendResult(id, {})
          break
        case "tools/list":
          sendResult(id, { tools: TOOLS })
          break
        case "tools/call":
          handleToolCall(id, m)
          break
        default:
          sendMessage(
            sendError(id, JSONRPC_ERROR.METHOD_NOT_FOUND, `Method not found: ${m.method}`),
          )
      }
    }

    function consume(): void {
      for (;;) {
        if (skipRemaining > 0) {
          if (buffer.length === 0) return
          const drop = Math.min(buffer.length, skipRemaining)
          buffer = buffer.subarray(drop)
          skipRemaining -= drop
          if (skipRemaining === 0) mode = "line"
          continue
        }

        if (mode === "line") {
          const nl = buffer.indexOf(0x0a)
          if (nl === -1) {
            if (buffer.length >= MAX_REQUEST_SIZE) {
              sendOversizeError()
              buffer = Buffer.alloc(0)
            }
            return
          }
          let line = buffer.subarray(0, nl)
          buffer = buffer.subarray(nl + 1)
          if (line.length > 0 && line[line.length - 1] === 0x0d) {
            line = line.subarray(0, line.length - 1)
          }
          if (line.length === 0) continue
          if (line.length > MAX_REQUEST_SIZE) {
            sendOversizeError()
            continue
          }
          const text = line.toString("utf8")
          if (HEADER_LINE_RE.test(text.trim())) {
            mode = "headers"
            headerText = text
            continue
          }
          handleRawMessage(text)
          continue
        }

        if (mode === "headers") {
          const nl = buffer.indexOf(0x0a)
          if (nl === -1) {
            if (headerText.length + buffer.length > MAX_REQUEST_SIZE) {
              sendOversizeError()
              headerText = ""
              buffer = Buffer.alloc(0)
            }
            return
          }
          let line = buffer.subarray(0, nl).toString("utf8")
          buffer = buffer.subarray(nl + 1)
          if (line.endsWith("\r")) line = line.slice(0, -1)
          if (line === "") {
            const match = CONTENT_LENGTH_RE.exec(headerText)
            headerText = ""
            if (!match) {
              sendMessage(
                sendError(
                  null,
                  JSONRPC_ERROR.INVALID_REQUEST,
                  "Invalid Request: missing Content-Length header",
                ),
              )
              mode = "line"
              continue
            }
            const length = Number(match[1])
            if (length > MAX_REQUEST_SIZE) {
              sendOversizeError()
              skipRemaining = length
              mode = "line"
              continue
            }
            mode = "body"
            bodyRemaining = length
            continue
          }
          headerText = headerText.length === 0 ? line : `${headerText}\n${line}`
          continue
        }

        if (buffer.length < bodyRemaining) return
        const bodyBuf = buffer.subarray(0, bodyRemaining)
        buffer = buffer.subarray(bodyRemaining)
        mode = "line"
        bodyRemaining = -1
        handleRawMessage(bodyBuf.toString("utf8"))
      }
    }

    function onData(chunk: Buffer): void {
      buffer = Buffer.concat([buffer, chunk])
      consume()
    }

    function onEnd(): void {
      if (!stopping && mode === "line" && buffer.length > 0) {
        let text = buffer.toString("utf8")
        if (text.endsWith("\r")) text = text.slice(0, -1)
        if (Buffer.byteLength(text, "utf8") > MAX_REQUEST_SIZE) {
          sendOversizeError()
        } else if (text.length > 0) {
          handleRawMessage(text)
        }
        buffer = Buffer.alloc(0)
      }
      stop()
    }

    function stop(): void {
      if (stopping) return
      stopping = true
      stdin.off("data", onData)
      stdin.off("end", onEnd)
      stdin.off("close", onEnd)
      resolve()
    }

    stdin.on("data", onData)
    stdin.on("end", onEnd)
    stdin.on("close", onEnd)
  })
}