//! Domain row types — the data structures shared by every layer: the DB
//! module reads them, the CLI renders them, MCP serializes them, FX and
//! export build documents over them (`@db/types` + `db/*` row shapes heritage
//! port). Keeping them database-free lets `subtrack-core` stay SQLite-free.

#[derive(Clone, Debug)]
pub struct Subscription {
    pub id: i64,
    pub name: String,
    pub price: f64,
    pub currency: String,
    pub cycle: String,
    pub status: String,
    pub billing_day: Option<i64>,
    pub created_at: String,
    pub notes: Option<String>,
    pub payment_method: Option<String>,
    pub contract_start: Option<String>,
    pub contract_end: Option<String>,
    pub auto_renewal: bool,
    pub vendor_name: Option<String>,
    pub vendor_url: Option<String>,
    pub plan_tier: Option<String>,
    pub discount_amount: Option<f64>,
    pub discount_type: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Default, Clone)]
pub struct QueryOptions {
    pub sort: Option<String>,
    pub desc: bool,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub include_archived: bool,
    pub status: Option<String>,
    pub min_price: Option<f64>,
    pub max_price: Option<f64>,
    pub tags: Vec<String>,
}

/// A subscription to insert (port of the fields read by `writeSubscription`).
#[derive(Default, Clone)]
pub struct NewSubscription {
    pub name: String,
    pub price: f64,
    pub currency: String,
    pub cycle: String,
    pub status: String,
    pub billing_day: Option<i64>,
    pub notes: Option<String>,
    pub payment_method: Option<String>,
    pub contract_start: Option<String>,
    pub contract_end: Option<String>,
    pub auto_renewal: bool,
    pub vendor_name: Option<String>,
    pub vendor_url: Option<String>,
    pub plan_tier: Option<String>,
    pub discount_amount: Option<f64>,
    pub discount_type: Option<String>,
    pub tags: Vec<String>,
}

/// A parameter value for prepared statements (port of the TS `SqlValue`
/// union). Converting to `rusqlite` values is the DB layer's job.
pub enum SqlValue {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
}

pub struct AuditEntry {
    pub id: i64,
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<i64>,
    pub details: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug)]
pub struct PriceChange {
    pub id: i64,
    pub subscription_id: i64,
    pub old_price: Option<f64>,
    pub new_price: f64,
    pub old_currency: Option<String>,
    pub new_currency: Option<String>,
    pub changed_at: String,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct Trial {
    pub id: i64,
    pub name: String,
    pub expires_at: String,
    pub price: Option<f64>,
    pub currency: Option<String>,
    pub cycle: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
}

/// An LLM usage row (`cost` in USD cents).
#[derive(Clone, Debug)]
pub struct LlmUsage {
    pub id: i64,
    pub provider: String,
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost: f64,
    pub date: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Suggestion {
    pub id: i64,
    pub name: String,
    pub price: Option<f64>,
    pub currency: Option<String>,
    pub cycle: Option<String>,
    pub vendor_name: Option<String>,
    pub vendor_url: Option<String>,
    pub plan_tier: Option<String>,
    pub payment_method: Option<String>,
    pub source: String,
    pub source_detail: Option<String>,
    pub email_subject: Option<String>,
    pub email_from: Option<String>,
    pub email_date: Option<String>,
    pub confidence: f64,
    pub status: String,
    pub matched_sub_id: Option<i64>,
    pub created_at: Option<String>,
}

/// A suggestion row to insert (port of the fields read by `writeSuggestion`).
pub struct NewSuggestion {
    pub name: String,
    pub price: Option<f64>,
    pub currency: Option<String>,
    pub cycle: Option<String>,
    pub source: String,
    pub source_detail: Option<String>,
    pub confidence: f64,
}
