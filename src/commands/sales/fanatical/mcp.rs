//! A minimal client for Fanatical's MCP server.
//!
//! **Nothing here is generative.** "MCP server" names a way of exposing structured data over
//! JSON-RPC 2.0; a client is an HTTP client that speaks that framing. There is no model in this
//! path, no tokens are spent, and the same call made twice returns the same bytes. It is used
//! because Fanatical asks clients to prefer it in writing and because it answers with Steam
//! ids, not for anything to do with AI.
//!
//! The exchange is three requests: `initialize`, which returns a session id in a header; an
//! `initialized` notification the protocol requires before any tool runs; then `tools/call`.

use std::time::Duration;

use log::debug;

use super::parse::{self, MCP_URL};
use crate::{Error, Result};

/// Protocol revision this client speaks, and the one the server reported.
const PROTOCOL_VERSION: &str = "2025-11-25";

/// Most slugs `get_products` accepts in one call.
pub const PRODUCTS_PER_CALL: usize = 20;

/// Header carrying the session id, assigned by `initialize`.
const SESSION_HEADER: &str = "mcp-session-id";

/// An opened MCP session.
pub struct Session {
    http: reqwest::blocking::Client,
    id: String,
    /// Request id, incremented per call as JSON-RPC requires.
    next_id: u64,
}

impl Session {
    /// Opens a session: handshake, then the notification the protocol requires before tools.
    pub fn open(user_agent: &str, timeout: Duration) -> Result<Self> {
        let http = reqwest::blocking::Client::builder()
            .user_agent(user_agent.to_owned())
            .timeout(timeout)
            .gzip(true)
            .build()
            .map_err(|source| Error::Fetch {
                url: MCP_URL.into(),
                source: source.into(),
            })?;

        let hello = serde_json::json!({
            "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": {
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": { "name": "catalogames", "version": env!("CARGO_PKG_VERSION") },
            }
        });

        let response = post(&http, MCP_URL, None, &hello)?;
        let id = response
            .headers()
            .get(SESSION_HEADER)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| Error::Protocol {
                service: "fanatical mcp",
                detail: format!("initialize returned no {SESSION_HEADER}"),
            })?
            .to_owned();

        let body = text(response, MCP_URL)?;
        let result = parse::json_rpc_result(&body)?;
        debug!(
            "  mcp session with {} {}",
            result["serverInfo"]["name"].as_str().unwrap_or("?"),
            result["serverInfo"]["version"].as_str().unwrap_or("?")
        );

        let mut session = Self {
            http,
            id,
            next_id: 1,
        };
        session.notify_initialized()?;
        Ok(session)
    }

    fn notify_initialized(&mut self) -> Result<()> {
        let note = serde_json::json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
        post(&self.http, MCP_URL, Some(&self.id), &note)?;
        Ok(())
    }

    /// Calls a tool and returns the response body, SSE framing intact.
    ///
    /// Framing is left on so the caller parses it with the same pure function the tests drive
    /// against a saved response.
    pub fn call(&mut self, tool: &str, arguments: serde_json::Value) -> Result<String> {
        let id = self.next_id;
        self.next_id += 1;

        let request = serde_json::json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": { "name": tool, "arguments": arguments }
        });
        debug!("  mcp {tool}");
        let response = post(&self.http, MCP_URL, Some(&self.id), &request)?;
        text(response, MCP_URL)
    }
}

fn post(
    http: &reqwest::blocking::Client,
    url: &str,
    session: Option<&str>,
    body: &serde_json::Value,
) -> Result<reqwest::blocking::Response> {
    let mut request = http
        .post(url)
        .header("Content-Type", "application/json")
        // Both are required: the server answers tool calls as an event stream.
        .header("Accept", "application/json, text/event-stream");
    if let Some(session) = session {
        request = request.header(SESSION_HEADER, session);
    }

    let response = request.json(body).send().map_err(|source| Error::Fetch {
        url: url.into(),
        source: source.into(),
    })?;

    let status = response.status();
    if !status.is_success() {
        return Err(Error::Status {
            url: url.into(),
            status: status.as_u16(),
        });
    }
    Ok(response)
}

fn text(response: reqwest::blocking::Response, url: &str) -> Result<String> {
    response.text().map_err(|source| Error::Fetch {
        url: url.into(),
        source: source.into(),
    })
}
