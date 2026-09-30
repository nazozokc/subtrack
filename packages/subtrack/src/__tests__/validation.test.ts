import { test, expect, describe } from "vitest"
import {
  validateName,
  validateNotes,
  validatePaymentMethod,
  validateVendorName,
  validatePlanTier,
  validateTrialName,
  validateModelName,
  validateTags,
} from "../validation.ts"

const ESC = String.fromCharCode(27)

/**
 * Free-text fields are stored verbatim and then re-emitted by `subtrack list`,
 * `export`, `notify`, the MCP tools, and the Raycast extension. Control
 * characters in them are pure attack surface: `ESC [ 2 J` repaints the user's
 * terminal, `ESC ] 0 ; ... BEL` renames the window, and a bare newline or tab
 * forges extra columns in table output.
 */
describe("control characters in single-line fields", () => {
  const fields = [
    ["validateName", validateName],
    ["validatePaymentMethod", validatePaymentMethod],
    ["validateVendorName", validateVendorName],
    ["validatePlanTier", validatePlanTier],
    ["validateTrialName", validateTrialName],
    ["validateModelName", validateModelName],
  ] as const

  for (const [label, fn] of fields) {
    test(`${label} rejects ESC (ANSI/OSC injection)`, () => {
      const result = fn(`Netflix${ESC}[2J`)
      expect(result).not.toBe(true)
      expect(String(result)).toMatch(/control character/i)
    })

    test(`${label} rejects a bare newline (column forging)`, () => {
      expect(fn("Netflix\nNetflix Premium")).not.toBe(true)
    })

    test(`${label} rejects a bare tab`, () => {
      expect(fn("Netflix\tPremium")).not.toBe(true)
    })

    test(`${label} rejects NUL, DEL, and C1`, () => {
      expect(fn("Netflix\u0000")).not.toBe(true)
      expect(fn("Netflix\u007F")).not.toBe(true)
      expect(fn("Netflix\u0085")).not.toBe(true)
    })

    test(`${label} still accepts ordinary text`, () => {
      expect(fn("Netflix Premium")).toBe(true)
    })
  }
})

describe("control characters in notes", () => {
  // Notes are the one genuinely multi-line field, so tab/newline/CR stay.
  test("accepts newlines and tabs", () => {
    expect(validateNotes("Line one\nLine two")).toBe(true)
    expect(validateNotes("a\tb")).toBe(true)
  })

  test("rejects ESC", () => {
    expect(validateNotes(`note${ESC}[2J`)).not.toBe(true)
  })

  test("rejects NUL and DEL", () => {
    expect(validateNotes("note\u0000")).not.toBe(true)
    expect(validateNotes("note\u007F")).not.toBe(true)
  })
})

describe("control characters in tags", () => {
  test("rejects ESC inside a tag", () => {
    const result = validateTags(`work,${ESC}[2J`)
    expect(result).not.toBe(true)
    expect(String(result)).toMatch(/control character/i)
  })

  test("rejects a newline inside a tag", () => {
    expect(validateTags("work\nhome")).not.toBe(true)
  })

  test("still accepts plain comma-separated tags", () => {
    expect(validateTags("work, streaming")).toBe(true)
  })
})
