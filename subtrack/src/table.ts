import Table from "cli-table3";
import { consola } from "consola";
import { type SharedArgs, getSubscriptions } from "./basefs.ts";

type FxRate = {
  base: string
  target: string
  rate: number
  timestamp: string
}

async function fetchFxRate(): Promise<FxRate> {
  const res = await fetch("https://fxapi.app/api/USD/JPY.json")
  if (!res.ok) {
    throw new Error(`fxapi responded with ${res.status}`)
  }
  return res.json() as Promise<FxRate>
}

function convertPrice(
  price: number,
  from: "JPY" | "USD",
  to: "JPY" | "USD",
  rate: number,
): number {
  if (from === to) return price
  // from: USD → to: JPY  → price * rate
  // from: JPY → to: USD  → price / rate
  return from === "USD" ? price * rate : price / rate
}

export const spreadSubscription = async (
  get?: SharedArgs[],
  currency?: "JPY" | "USD",
): Promise<void> => {
  const list = get ?? getSubscriptions()

  if (list.length === 0) {
    consola.info("No subscriptions found")
    return
  }

  const table = new Table({
    head: ["name", "cycle", "tags", "price"],
  })

  if (currency) {
    // --currency specified: fetch rate and convert all to the target currency
    let rate: number | null = null
    try {
      const fx = await fetchFxRate()
      rate = fx.rate
    } catch (e) {
      consola.error(
        `Failed to fetch exchange rate, falling back to separate display: ${e}`,
      )
      // fall through to the no-currency path below
    }

    if (rate !== null) {
      const fmt = new Intl.NumberFormat(
        currency === "JPY" ? "ja-JP" : "en-US",
        {
          style: "currency",
          currency,
          minimumFractionDigits: currency === "JPY" ? 0 : 2,
          maximumFractionDigits: currency === "JPY" ? 0 : 2,
        },
      )

      let total = 0
      for (const sub of list) {
        const converted = convertPrice(sub.price, sub.currency, currency, rate)
        total += converted
        table.push([
          String(sub.name),
          String(sub.cycle),
          sub.tags.length > 0 ? sub.tags.join(", ") : "-",
          fmt.format(converted),
        ])
      }

      table.push([
        "",
        "",
        `${currency} TOTAL`,
        fmt.format(total),
      ])

      consola.log(table.toString())
      return
    }
    // fallback: continue to the no-currency path below
  }

  // No currency filter: display JPY and USD separately (original behavior)
  for (const sub of list) {
    table.push([
      String(sub.name),
      String(sub.cycle),
      sub.tags.length > 0 ? sub.tags.join(", ") : "-",
      sub.currency === "USD"
        ? String(`$${sub.price}`)
        : String(`¥${sub.price}`),
    ])
  }

  const usdTotal = list
    .filter((n) => n.currency === "USD")
    .reduce((sum, n) => sum + n.price, 0)

  const jpyTotal = list
    .filter((n) => n.currency === "JPY")
    .reduce((sum, n) => sum + n.price, 0)

  table.push(
    ["", "", "JPY TOTAL", `¥${jpyTotal}`],
    ["", "", "USD TOTAL", `$${usdTotal}`],
  )

  consola.log(table.toString())
}
