import type { AddLlmUsageFromLogArgs } from "./types.ts"

export type ScanResult = {
  source: string
  entries: AddLlmUsageFromLogArgs[]
}

export interface Scanner {
  readonly name: string
  scan(from?: string, to?: string): ScanResult
}
