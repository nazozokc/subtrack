use subtrack_core::logger;
use subtrack_core::parser::ParsedArgs;

use super::handlers::call_tool;
use super::response::{build_initialize_result, id_to_string, send_error, send_result};
use super::schema::MAX_REQUEST_SIZE;
use super::tools::tools_json;

// ── protocol constants ───────────────────────────────────

pub(super) const SERVER_NAME: &str = "subtrack-mcp";
pub(super) const SERVER_VERSION: &str = "1.0.0";
pub(super) const DEFAULT_PROTOCOL_VERSION: &str = "2024-11-05";

const ERR_PARSE: i64 = -32700;
const ERR_INVALID_REQUEST: i64 = -32600;
const ERR_METHOD_NOT_FOUND: i64 = -32601;
const ERR_INVALID_PARAMS: i64 = -32602;

struct Server {
    initialized: bool,
    out: Vec<String>,
    deferred: Vec<String>,
}

impl Server {
    fn new() -> Self {
        Server {
            initialized: false,
            out: Vec::new(),
            deferred: Vec::new(),
        }
    }

    fn handle_message(&mut self, msg: &serde_json::Value) {
        let m = match msg.as_object() {
            Some(o) => o,
            None => {
                self.out
                    .push(send_error("null", ERR_INVALID_REQUEST, "Invalid Request"));
                return;
            }
        };

        let id = m.get("id");
        let is_notification = matches!(id, None | Some(serde_json::Value::Null));
        if is_notification {
            return;
        }

        let method = match m.get("method").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                self.out
                    .push(send_error("null", ERR_INVALID_REQUEST, "Invalid Request"));
                return;
            }
        };
        let id_v = id.unwrap();
        if !id_v.is_number() && !id_v.is_string() {
            self.out
                .push(send_error("null", ERR_INVALID_REQUEST, "Invalid Request"));
            return;
        }
        let id_str = id_to_string(id_v);

        if !self.initialized && method != "initialize" {
            self.out.push(send_error(
                &id_str,
                ERR_INVALID_REQUEST,
                "Invalid Request: initialize required",
            ));
            return;
        }

        match method {
            "initialize" => {
                self.initialized = true;
                let result = build_initialize_result(m.get("params"));
                self.out.push(send_result(&id_str, &result));
            }
            "ping" => {
                self.out.push(send_result(&id_str, "{}"));
            }
            "tools/list" => {
                let result = format!("{{\"tools\":{}}}", tools_json());
                self.out.push(send_result(&id_str, &result));
            }
            "tools/call" => {
                let params = m.get("params").and_then(|v| v.as_object());
                let name = params.and_then(|p| p.get("name")).and_then(|v| v.as_str());
                let name = match name {
                    Some(n) => n.to_string(),
                    None => {
                        self.out.push(send_error(
                            &id_str,
                            ERR_INVALID_PARAMS,
                            "tools/call requires a string params.name",
                        ));
                        return;
                    }
                };
                let args = params
                    .and_then(|p| p.get("arguments"))
                    .and_then(|v| v.as_object());
                let resp = call_tool(&name, args);
                self.deferred.push(send_result(&id_str, &resp.to_json()));
            }
            other => {
                self.out.push(send_error(
                    &id_str,
                    ERR_METHOD_NOT_FOUND,
                    &format!("Method not found: {}", other),
                ));
            }
        }
    }

    fn handle_raw(&mut self, text: &str) {
        if text.len() > MAX_REQUEST_SIZE {
            self.out.push(send_error(
                "null",
                ERR_PARSE,
                &format!("Request too large (max {} bytes)", MAX_REQUEST_SIZE),
            ));
            return;
        }
        match serde_json::from_str::<serde_json::Value>(text) {
            Ok(v) => self.handle_message(&v),
            Err(_) => self.out.push(send_error("null", ERR_PARSE, "Parse error")),
        }
    }

    /// Consume a byte buffer using the same framing state machine as
    /// transport.ts (line mode auto-switches to Content-Length headers).
    fn consume(&mut self, input: &[u8]) {
        let buffer: Vec<u8> = input.to_vec();
        let mut pos = 0usize;
        let mut mode = 0u8; // 0 = line, 1 = headers, 2 = body
        let mut header_text = String::new();
        let mut body_remaining = 0usize;
        let mut skip_remaining = 0usize;

        loop {
            if skip_remaining > 0 {
                if pos >= buffer.len() {
                    break;
                }
                let drop = std::cmp::min(buffer.len() - pos, skip_remaining);
                pos += drop;
                skip_remaining -= drop;
                if skip_remaining == 0 {
                    mode = 0;
                }
                continue;
            }

            if mode == 0 {
                let nl = buffer[pos..].iter().position(|&b| b == 0x0a);
                let nl = match nl {
                    Some(n) => pos + n,
                    None => {
                        if buffer.len() - pos >= MAX_REQUEST_SIZE {
                            self.out.push(send_error(
                                "null",
                                ERR_PARSE,
                                &format!("Request too large (max {} bytes)", MAX_REQUEST_SIZE),
                            ));
                            pos = buffer.len();
                        }
                        break;
                    }
                };
                let mut line = buffer[pos..nl].to_vec();
                pos = nl + 1;
                if line.last() == Some(&0x0d) {
                    line.pop();
                }
                if line.is_empty() {
                    continue;
                }
                if line.len() > MAX_REQUEST_SIZE {
                    self.out.push(send_error(
                        "null",
                        ERR_PARSE,
                        &format!("Request too large (max {} bytes)", MAX_REQUEST_SIZE),
                    ));
                    continue;
                }
                let text = String::from_utf8_lossy(&line).to_string();
                if is_content_length_line(text.trim()) {
                    mode = 1;
                    header_text = text;
                    continue;
                }
                self.handle_raw(&text);
                continue;
            }

            if mode == 1 {
                let nl = buffer[pos..].iter().position(|&b| b == 0x0a);
                let nl = match nl {
                    Some(n) => pos + n,
                    None => {
                        if header_text.len() + (buffer.len() - pos) > MAX_REQUEST_SIZE {
                            self.out.push(send_error(
                                "null",
                                ERR_PARSE,
                                &format!("Request too large (max {} bytes)", MAX_REQUEST_SIZE),
                            ));
                            header_text.clear();
                            pos = buffer.len();
                        }
                        break;
                    }
                };
                let mut line = String::from_utf8_lossy(&buffer[pos..nl]).to_string();
                pos = nl + 1;
                if line.ends_with('\r') {
                    line.pop();
                }
                if line.is_empty() {
                    let length = parse_content_length(&header_text);
                    header_text.clear();
                    let length = match length {
                        Some(l) => l,
                        None => {
                            self.out.push(send_error(
                                "null",
                                ERR_INVALID_REQUEST,
                                "Invalid Request: missing Content-Length header",
                            ));
                            mode = 0;
                            continue;
                        }
                    };
                    if length > MAX_REQUEST_SIZE {
                        self.out.push(send_error(
                            "null",
                            ERR_PARSE,
                            &format!("Request too large (max {} bytes)", MAX_REQUEST_SIZE),
                        ));
                        skip_remaining = length;
                        mode = 0;
                        continue;
                    }
                    mode = 2;
                    body_remaining = length;
                    continue;
                }
                if header_text.is_empty() {
                    header_text = line;
                } else {
                    header_text.push('\n');
                    header_text.push_str(&line);
                }
                continue;
            }

            // mode == 2 (body)
            if buffer.len() - pos < body_remaining {
                break;
            }
            let body = buffer[pos..pos + body_remaining].to_vec();
            pos += body_remaining;
            mode = 0;
            body_remaining = 0;
            self.handle_raw(&String::from_utf8_lossy(&body));
        }

        // Leftover bytes in line mode with no trailing newline (EOF).
        if mode == 0 && skip_remaining == 0 && pos < buffer.len() {
            let mut text = String::from_utf8_lossy(&buffer[pos..]).to_string();
            if text.ends_with('\r') {
                text.pop();
            }
            if text.len() > MAX_REQUEST_SIZE {
                self.out.push(send_error(
                    "null",
                    ERR_PARSE,
                    &format!("Request too large (max {} bytes)", MAX_REQUEST_SIZE),
                ));
            } else if !text.is_empty() {
                self.handle_raw(&text);
            }
        }
    }
}

fn is_content_length_line(s: &str) -> bool {
    let lower = s.to_lowercase();
    let rest = match lower.strip_prefix("content-length:") {
        Some(r) => r,
        None => return false,
    };
    let trimmed = rest.trim();
    !trimmed.is_empty() && trimmed.chars().all(|c| c.is_ascii_digit())
}

fn parse_content_length(headers: &str) -> Option<usize> {
    for line in headers.lines() {
        let lower = line.to_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:")
            && let Ok(n) = rest.trim().parse::<usize>()
        {
            return Some(n);
        }
    }
    None
}

// ── entry point ──────────────────────────────────────────

pub fn run_mcp(_parsed: &ParsedArgs) -> i32 {
    use std::io::Read;
    // Bound the whole session, not just a single message; otherwise a client
    // can grow the process without limit before framing even starts.
    const MAX_STDIN_BYTES: u64 = 64 * 1024 * 1024;

    let mut input = Vec::new();
    let read = std::io::stdin()
        .lock()
        .take(MAX_STDIN_BYTES + 1)
        .read_to_end(&mut input);
    if read.is_err() || input.len() as u64 > MAX_STDIN_BYTES {
        logger::outln(&send_error(
            "null",
            ERR_PARSE,
            "Request stream too large (max 64 MB)",
        ));
        return 1;
    }

    let mut server = Server::new();
    server.consume(&input);
    // Async tool responses resolve after the current synchronous stack —
    // flush them last (mirrors the promise microtask ordering in transport.ts).
    let mut lines = server.out;
    lines.append(&mut server.deferred);
    for line in lines {
        logger::outln(&line);
    }
    0
}
