use subtrack_core::json::quote_string as js_str;

use super::server::{DEFAULT_PROTOCOL_VERSION, SERVER_NAME, SERVER_VERSION};

// ── responses ────────────────────────────────────────────

pub(super) struct Resp {
    text: String,
    is_error: bool,
}

impl Resp {
    pub(super) fn ok(text: impl Into<String>) -> Self {
        Resp {
            text: text.into(),
            is_error: false,
        }
    }
    pub(super) fn err(text: impl Into<String>) -> Self {
        Resp {
            text: text.into(),
            is_error: true,
        }
    }
    pub(super) fn to_json(&self) -> String {
        let content = format!(
            "{{\"content\":[{{\"type\":\"text\",\"text\":{}}}]",
            js_str(&self.text)
        );
        if self.is_error {
            format!("{},\"isError\":true}}", content)
        } else {
            format!("{}}}", content)
        }
    }
}

// ── argument accessors ───────────────────────────────────

pub(super) fn arg_str<'a>(
    args: Option<&'a serde_json::Map<String, serde_json::Value>>,
    k: &str,
) -> Option<&'a str> {
    args.and_then(|m| m.get(k)).and_then(|v| v.as_str())
}

pub(super) fn arg_num(
    args: Option<&serde_json::Map<String, serde_json::Value>>,
    k: &str,
) -> Option<f64> {
    args.and_then(|m| m.get(k)).and_then(|v| v.as_f64())
}

pub(super) fn arg_bool(
    args: Option<&serde_json::Map<String, serde_json::Value>>,
    k: &str,
) -> Option<bool> {
    args.and_then(|m| m.get(k)).and_then(|v| v.as_bool())
}

pub(super) fn arg_present(
    args: Option<&serde_json::Map<String, serde_json::Value>>,
    k: &str,
) -> bool {
    args.map(|m| m.contains_key(k)).unwrap_or(false)
}

// ── JSON-RPC envelope ────────────────────────────────────

pub(super) fn send_result(id: &str, result: &str) -> String {
    format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{},\"result\":{}}}",
        id, result
    )
}

pub(super) fn send_error(id: &str, code: i64, message: &str) -> String {
    format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{},\"error\":{{\"code\":{},\"message\":{}}}}}",
        id,
        code,
        js_str(message)
    )
}

pub(super) fn id_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => js_str(s),
        _ => "null".to_string(),
    }
}

pub(super) fn build_initialize_result(params: Option<&serde_json::Value>) -> String {
    let protocol = params
        .and_then(|p| p.get("protocolVersion"))
        .and_then(|v| v.as_str())
        .unwrap_or(DEFAULT_PROTOCOL_VERSION);
    format!(
        "{{\"protocolVersion\":{},\"capabilities\":{{\"tools\":{{}}}},\"serverInfo\":{{\"name\":{},\"version\":{}}}}}",
        js_str(protocol),
        js_str(SERVER_NAME),
        js_str(SERVER_VERSION)
    )
}
