export type FxRates = {
  base: string
  rates: Record<string, number>
}

export async function fetchFxRates(): Promise<FxRates> {
  const res = await fetch("https://open.er-api.com/v6/latest/USD")
  if (!res.ok) {
    throw new Error(`FX API responded with ${res.status}`)
  }
  return res.json() as Promise<FxRates>
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
