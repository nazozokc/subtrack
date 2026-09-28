import { test, expect, beforeAll, beforeEach } from "vitest"
import { consola } from "@subtrack/lib/logger"
import { CURRENCY_CHOICES } from "../prompts.ts"
import { handleCurrencyList } from "../currency.ts"

const logMessages: string[] = []

async function captureStdout(fn: () => void): Promise<string> {
  const writes: string[] = []
  const origWrite = process.stdout.write.bind(process.stdout)
  process.stdout.write = ((chunk: string | Uint8Array) => {
    writes.push(String(chunk))
    return true
  }) as typeof process.stdout.write
  try {
    fn()
  } finally {
    process.stdout.write = origWrite
  }
  return writes.join("")
}

beforeAll(() => {
  consola.mockTypes((_type: string, _defaults: object) => {
    return (...args: unknown[]) => {
      logMessages.push(args.map((a) => String(a)).join(" "))
    }
  })
})

beforeEach(() => {
  logMessages.length = 0
})

test("handleCurrencyList lists every supported currency", () => {
  handleCurrencyList()
  const out = logMessages.join("\n")
  expect(out).toContain("Supported Currencies")
  for (const c of CURRENCY_CHOICES) {
    expect(out).toContain(c.name)
  }
  expect(out).toContain(`${CURRENCY_CHOICES.length} currencies supported`)
})

test("handleCurrencyList --json outputs the raw choices array", async () => {
  const out = await captureStdout(() => handleCurrencyList({ json: true }))
  const parsed = JSON.parse(out) as Array<{ value: string; name: string }>
  expect(parsed).toEqual(CURRENCY_CHOICES)
  expect(logMessages).toHaveLength(0)
})

test("handleCurrencyList succeeds with no stored currency state", () => {
  handleCurrencyList()
  expect(process.exitCode ?? 0).toBe(0)
})