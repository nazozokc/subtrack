export type FxRates = {
  base: string
  rates: Record<string, number>
}

const FX_FETCH_TIMEOUT_MS = 10_000

export async function fetchFxRates(): Promise<FxRates> {
  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), FX_FETCH_TIMEOUT_MS)
  try {
    const res = await fetch("https://open.er-api.com/v6/latest/USD", {
      signal: controller.signal,
    })
    if (!res.ok) {
      throw new Error(`FX API responded with ${res.status}`)
    }
    return res.json() as Promise<FxRates>
  } finally {
    clearTimeout(timer)
  }
}

export function convertPrice(
  price: number,
  from: string,
  to: string,
  rates: Record<string, number>,
): number {
  if (from === to) return price
  const fromRate = rates[from]
  const toRate = rates[to]
  if (!fromRate || !toRate) {
    throw new Error(`No rate available for ${from} → ${to}`)
  }
  // Convert via USD base: source → USD → target
  const inUsd = from === "USD" ? price : price / fromRate
  return to === "USD" ? inUsd : inUsd * toRate
}
