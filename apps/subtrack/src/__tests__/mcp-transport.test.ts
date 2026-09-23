import { describe, test, expect, beforeAll, afterAll } from "vitest"
import { PassThrough, Writable } from "node:stream"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { startTransport } from "../mcp/transport.ts"
import { MAX_REQUEST_SIZE } from "../mcp/security.ts"
import { closeDb } from "../db/connection.ts"

const INIT_MSG =
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0.0"}}}'
const GOLDEN_INITIALIZE =
  '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"subtrack-mcp","version":"1.0.0"}}}\n'

class CollectingWritable extends Writable {
  chunks: Buffer[] = []
  override _write(chunk: Buffer, _encoding: BufferEncoding, callback: () => void): void {
    this.chunks.push(Buffer.from(chunk))
    callback()
  }
  text(): string {
    return Buffer.concat(this.chunks).toString("utf8")
  }
}

function runSession(input: string): Promise<string> {
  const stdin = new PassThrough()
  const stdout = new CollectingWritable()
  const done = startTransport(stdin, stdout)
  stdin.write(input)
  stdin.end()
  return done.then(() => stdout.text())
}

function runToEnd(input: string): Promise<string> {
  return runSession(input)
}

let dbDir: string

beforeAll(() => {
  dbDir = mkdtempSync(join(tmpdir(), "subtrack-mcp-transport-"))
  process.env.SUBSC_CLI_DB_DIR = dbDir
})

afterAll(() => {
  closeDb()
  delete process.env.SUBSC_CLI_DB_DIR
  rmSync(dbDir, { recursive: true, force: true })
})

describe("MCP stdio transport — framing", () => {
  test("Content-Length framed input is accepted", async () => {
    const ping = '{"jsonrpc":"2.0","id":7,"method":"ping"}'
    const out = await runToEnd(
      `Content-Length: ${Buffer.byteLength(INIT_MSG)}\r\n\r\n${INIT_MSG}\r\n` +
        `Content-Length: ${Buffer.byteLength(ping)}\r\n\r\n${ping}\r\n`,
    )
    const lines = out.trim().split("\n")
    expect(lines[0]).toBe(GOLDEN_INITIALIZE.trim())
    expect(lines[1]).toBe('{"jsonrpc":"2.0","id":7,"result":{}}')
  })

  test("Content-Length body may span multiple lines", async () => {
    const payload = JSON.stringify({ jsonrpc: "2.0", id: 3, method: "ping" }, null, 2)
    const out = await runToEnd(
      `Content-Length: ${Buffer.byteLength(INIT_MSG)}\n\n${INIT_MSG}\n` +
        `Content-Length: ${Buffer.byteLength(payload)}\n\n${payload}\r\n`,
    )
    const lines = out.trim().split("\n")
    expect(lines[0]).toBe(GOLDEN_INITIALIZE.trim())
    expect(lines[1]).toBe('{"jsonrpc":"2.0","id":3,"result":{}}')
  })

  test("Content-Length and NDJSON messages interleave per-message", async () => {
    const framed = `Content-Length: ${Buffer.byteLength(INIT_MSG)}\r\n\r\n${INIT_MSG}\r\n`
    const out = await runToEnd(`${framed}{"jsonrpc":"2.0","id":5,"method":"ping"}\n`)
    const lines = out.trim().split("\n")
    expect(lines[0]).toBe(GOLDEN_INITIALIZE.trim())
    expect(lines[1]).toBe('{"jsonrpc":"2.0","id":5,"result":{}}')
  })
})

describe("MCP stdio transport — NDJSON flow", () => {
  test("initialize returns the exact golden response", async () => {
    const out = await runToEnd(`${INIT_MSG}\n`)
    expect(out).toBe(GOLDEN_INITIALIZE)
  })

  test("initialize → tools/list → tools/call round-trips", async () => {
    const { TOOLS } = await import("../mcp/tools.ts")
    const out = await runToEnd(
      `${INIT_MSG}\n` +
        '{"jsonrpc":"2.0","id":2,"method":"tools/list"}\n' +
        '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get_summary","arguments":{}}}\n',
    )
    const lines = out.trim().split("\n")
    expect(lines).toHaveLength(3)
    expect(lines[0]).toBe(GOLDEN_INITIALIZE.trim())

    const listMsg = JSON.parse(lines[1]) as {
      id: number
      result: { tools: unknown[] }
    }
    expect(listMsg.id).toBe(2)
    expect(listMsg.result.tools).toEqual(TOOLS)

    const callMsg = JSON.parse(lines[2]) as {
      id: number
      result: { content: { type: string; text: string }[]; isError?: boolean }
    }
    expect(callMsg.id).toBe(3)
    expect(callMsg.result.isError).toBeUndefined()
    expect(callMsg.result.content).toHaveLength(1)
    const summary = JSON.parse(callMsg.result.content[0]!.text) as { totalCount: number }
    expect(summary.totalCount).toBe(0)
  })

  test("notifications/initialized and notifications/cancelled produce no reply", async () => {
    const out = await runToEnd(
      `${INIT_MSG}\n` +
        '{"jsonrpc":"2.0","method":"notifications/initialized"}\n' +
        '{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1,"reason":"manual"}}\n',
    )
    expect(out).toBe(GOLDEN_INITIALIZE)
  })

  test("unknown method returns -32601", async () => {
    const out = await runToEnd(`${INIT_MSG}\n{"jsonrpc":"2.0","id":9,"method":"foo/bar"}\n`)
    const last = JSON.parse(out.trim().split("\n").pop()!) as { id: number; error: { code: number } }
    expect(last).toMatchObject({ id: 9, error: { code: -32601 } })
  })

  test("tools/list before initialize returns -32600", async () => {
    const out = await runToEnd('{"jsonrpc":"2.0","id":5,"method":"tools/list"}\n')
    const parsed = JSON.parse(out.trim()) as { id: number; error: { code: number } }
    expect(parsed).toMatchObject({ id: 5, error: { code: -32600 } })
  })

  test("malformed JSON returns -32700 with null id", async () => {
    const out = await runToEnd(`${INIT_MSG}\n{not valid json}\n`)
    const last = JSON.parse(out.trim().split("\n").pop()!) as { id: null; error: { code: number } }
    expect(last).toMatchObject({ id: null, error: { code: -32700 } })
  })

  test("unknown tool returns isError content", async () => {
    const out = await runToEnd(
      `${INIT_MSG}\n` +
        '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"no_such_tool","arguments":{}}}\n',
    )
    const msg = JSON.parse(out.trim().split("\n").pop()!) as {
      id: number
      result: { isError: boolean; content: { text: string }[] }
    }
    expect(msg.id).toBe(4)
    expect(msg.result.isError).toBe(true)
    expect(msg.result.content[0]!.text).toMatch(/Unknown tool/)
  })

  test("numeric and string ids echo correctly", async () => {
    const out = await runToEnd(
      `${INIT_MSG}\n` +
        '{"jsonrpc":"2.0","id":42,"method":"tools/list"}\n' +
        '{"jsonrpc":"2.0","id":"str-id-1","method":"tools/list"}\n',
    )
    const lines = out.trim().split("\n")
    expect(JSON.parse(lines[1]!)!.id).toBe(42)
    expect(JSON.parse(lines[2]!)!.id).toBe("str-id-1")
  })
})

describe("MCP stdio transport — oversize handling", () => {
  test("oversized NDJSON line is rejected before parse without hanging", async () => {
    const hugeLine = `{"jsonrpc":"2.0","id":1,"method":"tools/list","padding":"${"x".repeat(MAX_REQUEST_SIZE + 100)}"}\n`
    const out = await runToEnd(`${INIT_MSG}\n${hugeLine}{"jsonrpc":"2.0","id":2,"method":"tools/list"}\n`)
    const lines = out.trim().split("\n")
    expect(lines).toHaveLength(3)
    expect(lines[0]).toBe(GOLDEN_INITIALIZE.trim())
    const oversize = JSON.parse(lines[1]!) as { id: null; error: { code: number; message: string } }
    expect(oversize.id).toBeNull()
    expect(oversize.error.code).toBe(-32700)
    expect(oversize.error.message).toMatch(/too large/)
    expect(JSON.parse(lines[2]!).id).toBe(2)
  })
})