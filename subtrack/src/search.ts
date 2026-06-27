import { input } from "@inquirer/prompts"
import { consola } from "consola"
import { getDb, mapTags } from "./db.ts"
import { spreadSubscription } from "./display.ts"
import type { SharedArgs } from "./types.ts"
import type { SqlValue } from "sql.js"

/**
 * Search subscriptions by name, notes, and/or tags.
 * When no field flags are given, searches all fields.
 */
export async function handleSearch(
  query: string,
  options: { names?: boolean; notes?: boolean; tags?: boolean },
): Promise<void> {
  // Interactive prompt if no query provided
  if (!query) {
    const answer = await input({
      message: "search query",
      validate: (v: string) => (v.trim() ? true : "Query cannot be empty"),
    })
    query = answer.trim()
  }

  const fields = {
    names: options.names ?? (!options.notes && !options.tags),
    notes: options.notes ?? (!options.names && !options.tags),
    tags: options.tags ?? (!options.names && !options.notes),
  }

  const results = searchSubscriptions(query, fields)

  if (results.length === 0) {
    consola.info(`No results for "${query}"`)
    return
  }

  consola.info(
    `Found ${results.length} result${results.length > 1 ? "s" : ""} for "${query}":`,
  )
  await spreadSubscription(results)
}

function searchSubscriptions(
  query: string,
  fields: { names: boolean; notes: boolean; tags: boolean },
): SharedArgs[] {
  const db = getDb()
  const pattern = `%${query}%`
  const conditions: string[] = []
  const params: SqlValue[] = []

  if (fields.names) {
    conditions.push("s.name LIKE ?")
    params.push(pattern)
  }
  if (fields.notes) {
    conditions.push("s.notes LIKE ?")
    params.push(pattern)
  }
  if (fields.tags) {
    conditions.push(
      "s.id IN (SELECT st.subscription_id FROM subscription_tags st JOIN tags t ON t.id = st.tag_id WHERE t.name LIKE ?)",
    )
    params.push(pattern)
  }

  const whereClause = conditions.length > 0 ? `WHERE ${conditions.join(" OR ")}` : ""
  const sql = `SELECT DISTINCT s.id, s.name, s.price, s.currency, s.cycle, s.status, s.billing_day AS billingDay, s.created_at AS createdAt, s.notes, s.payment_method AS paymentMethod FROM subscriptions s ${whereClause} ORDER BY s.name`

  const results = db.exec(sql, params)
  if (!results.length) return []

  const { columns, values } = results[0]
  const subs: SharedArgs[] = values.map((row) => {
    const obj: Record<string, unknown> = {}
    for (let i = 0; i < columns.length; i++) {
      obj[columns[i]] = row[i]
    }
    return {
      id: Number(obj.id),
      name: String(obj.name),
      price: Number(obj.price),
      currency: String(obj.currency),
      cycle: String(obj.cycle),
      status: String(obj.status),
      billingDay: obj.billingDay !== null ? Number(obj.billingDay) : null,
      createdAt: String(obj.createdAt),
      notes: obj.notes !== null ? String(obj.notes) : null,
      paymentMethod: obj.paymentMethod !== null ? String(obj.paymentMethod) : null,
      tags: [],
    } as SharedArgs
  })

  return mapTags(subs)
}
