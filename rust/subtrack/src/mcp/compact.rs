use crate::db::{self, Subscription};
use crate::json::{number, quote_string as js_str};

// ── JSON helpers (JS JSON.stringify semantics) ───────────

pub(super) fn opt_str(v: &Option<String>) -> String {
    v.as_deref()
        .map(js_str)
        .unwrap_or_else(|| "null".to_string())
}

pub(super) fn opt_num(v: Option<f64>) -> String {
    v.map(number).unwrap_or_else(|| "null".to_string())
}

pub(super) fn opt_i64(v: Option<i64>) -> String {
    v.map(|x| x.to_string())
        .unwrap_or_else(|| "null".to_string())
}

pub(super) fn arr_strings(v: &[String]) -> String {
    let items: Vec<String> = v.iter().map(|s| js_str(s)).collect();
    format!("[{}]", items.join(","))
}

/// Compact `JSON.stringify(subscription)` (key order = SUB_COLUMNS + tags).
pub(super) fn sub_compact(s: &Subscription) -> String {
    let f = [
        format!("\"id\":{}", s.id),
        format!("\"name\":{}", js_str(&s.name)),
        format!("\"price\":{}", number(s.price)),
        format!("\"currency\":{}", js_str(&s.currency)),
        format!("\"cycle\":{}", js_str(&s.cycle)),
        format!("\"status\":{}", js_str(&s.status)),
        format!("\"billingDay\":{}", opt_i64(s.billing_day)),
        format!("\"createdAt\":{}", js_str(&s.created_at)),
        format!("\"notes\":{}", opt_str(&s.notes)),
        format!("\"paymentMethod\":{}", opt_str(&s.payment_method)),
        format!("\"contractStart\":{}", opt_str(&s.contract_start)),
        format!("\"contractEnd\":{}", opt_str(&s.contract_end)),
        format!("\"autoRenewal\":{}", s.auto_renewal),
        format!("\"vendorName\":{}", opt_str(&s.vendor_name)),
        format!("\"vendorUrl\":{}", opt_str(&s.vendor_url)),
        format!("\"planTier\":{}", opt_str(&s.plan_tier)),
        format!("\"discountAmount\":{}", opt_num(s.discount_amount)),
        format!("\"discountType\":{}", opt_str(&s.discount_type)),
        format!("\"tags\":{}", arr_strings(&s.tags)),
    ];
    format!("{{{}}}", f.join(","))
}

pub(super) fn subs_compact(v: &[Subscription]) -> String {
    let items: Vec<String> = v.iter().map(sub_compact).collect();
    format!("[{}]", items.join(","))
}

pub(super) fn trial_compact(t: &db::Trial) -> String {
    format!(
        "{{\"id\":{},\"name\":{},\"expiresAt\":{},\"price\":{},\"currency\":{},\"cycle\":{},\"notes\":{},\"createdAt\":{}}}",
        t.id,
        js_str(&t.name),
        js_str(&t.expires_at),
        opt_num(t.price),
        opt_str(&t.currency),
        opt_str(&t.cycle),
        opt_str(&t.notes),
        js_str(&t.created_at),
    )
}

pub(super) fn price_change_compact(c: &db::PriceChange) -> String {
    format!(
        "{{\"id\":{},\"subscriptionId\":{},\"subscriptionName\":{},\"oldPrice\":{},\"newPrice\":{},\"oldCurrency\":{},\"newCurrency\":{},\"changedAt\":{}}}",
        c.id,
        c.subscription_id,
        js_str(&c.name),
        opt_num(c.old_price),
        number(c.new_price),
        opt_str(&c.old_currency),
        opt_str(&c.new_currency),
        js_str(&c.changed_at),
    )
}

pub(super) fn llm_usage_compact(u: &db::LlmUsage) -> String {
    format!(
        "{{\"id\":{},\"provider\":{},\"model\":{},\"input_tokens\":{},\"output_tokens\":{},\"cost\":{},\"date\":{},\"description\":{}}}",
        u.id,
        js_str(&u.provider),
        js_str(&u.model),
        u.input_tokens,
        u.output_tokens,
        number(u.cost),
        js_str(&u.date),
        opt_str(&u.description),
    )
}
