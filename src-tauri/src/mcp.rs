//! A small MCP server over HTTP ("streamable HTTP", JSON responses only).
//!
//! It listens on 127.0.0.1 on a free port. The URL path holds a random
//! token: over SSH the port is forwarded to another machine, where other
//! users may reach that machine's loopback interface.

use std::io;
use std::sync::Arc;

use serde_json::{json, Value};
use tiny_http::{Header, Method, Response, Server};

/// The tools a server offers.
pub trait Toolbox: Send + Sync + 'static {
    /// The tool definitions for `tools/list`: name, description and
    /// inputSchema of each.
    fn definitions(&self) -> Value;
    /// Runs a tool. The text goes back to the model; an error is shown to
    /// the model as a failed call.
    fn call(&self, name: &str, arguments: &Value) -> Result<String, String>;
}

pub struct Running {
    pub url: String,
}

const PROTOCOL: &str = "2025-06-18";

pub fn serve(name: &'static str, toolbox: impl Toolbox) -> io::Result<Running> {
    let server = Server::http("127.0.0.1:0").map_err(io::Error::other)?;
    let port = server
        .server_addr()
        .to_ip()
        .map(|a| a.port())
        .ok_or_else(|| io::Error::other("no TCP port"))?;
    let path = format!("/mcp/{}", lejce::new_session_id().replace('-', ""));
    let url = format!("http://127.0.0.1:{port}{path}");
    let toolbox = Arc::new(toolbox);
    std::thread::spawn(move || {
        for request in server.incoming_requests() {
            respond(name, &path, toolbox.as_ref(), request);
        }
    });
    Ok(Running { url })
}

fn json_header() -> Header {
    Header::from_bytes("Content-Type", "application/json").expect("valid header")
}

fn respond(name: &str, path: &str, toolbox: &dyn Toolbox, mut request: tiny_http::Request) {
    if request.url() != path {
        let _ = request.respond(Response::empty(404));
        return;
    }
    // No server-sent events: GET for a stream and DELETE of a session are
    // not supported, which the protocol allows.
    if *request.method() != Method::Post {
        let _ = request.respond(Response::empty(405));
        return;
    }
    let mut body = String::new();
    if request.as_reader().read_to_string(&mut body).is_err() {
        let _ = request.respond(Response::empty(400));
        return;
    }
    let reply = match serde_json::from_str::<Value>(&body) {
        Ok(Value::Array(messages)) => {
            let replies: Vec<Value> = messages
                .iter()
                .filter_map(|m| handle(name, toolbox, m))
                .collect();
            (!replies.is_empty()).then_some(Value::Array(replies))
        }
        Ok(message) => handle(name, toolbox, &message),
        Err(e) => Some(error(Value::Null, -32700, &format!("parse error: {e}"))),
    };
    let _ = match reply {
        Some(reply) => {
            request.respond(Response::from_string(reply.to_string()).with_header(json_header()))
        }
        // Notifications and responses get no answer.
        None => request.respond(Response::empty(202)),
    };
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// One JSON-RPC message. None for a notification.
pub fn handle(name: &str, toolbox: &dyn Toolbox, message: &Value) -> Option<Value> {
    let id = message.get("id")?.clone();
    // A response from the client: this server sends no requests.
    message.get("method")?;
    let params = &message["params"];
    let result = match message["method"].as_str().unwrap_or("") {
        "initialize" => json!({
            "protocolVersion": params["protocolVersion"].as_str().unwrap_or(PROTOCOL),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": name, "version": env!("CARGO_PKG_VERSION") },
        }),
        "ping" => json!({}),
        "tools/list" => json!({ "tools": toolbox.definitions() }),
        "tools/call" => {
            let tool = params["name"].as_str().unwrap_or("");
            let empty = json!({});
            let arguments = params.get("arguments").unwrap_or(&empty);
            let (text, failed) = match toolbox.call(tool, arguments) {
                Ok(text) => (text, false),
                Err(text) => (text, true),
            };
            json!({ "content": [{ "type": "text", "text": text }], "isError": failed })
        }
        other => return Some(error(id, -32601, &format!("unknown method: {other}"))),
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;

    struct Echo;

    impl Toolbox for Echo {
        fn definitions(&self) -> Value {
            json!([{ "name": "echo", "description": "Echo", "inputSchema": { "type": "object" } }])
        }
        fn call(&self, name: &str, arguments: &Value) -> Result<String, String> {
            match name {
                "echo" => Ok(arguments["text"].as_str().unwrap_or("").to_string()),
                _ => Err(format!("no tool {name}")),
            }
        }
    }

    fn call(method: &str, params: Value) -> Value {
        let message = json!({ "jsonrpc": "2.0", "id": 7, "method": method, "params": params });
        handle("test", &Echo, &message).unwrap()
    }

    #[test]
    fn initialize_echoes_the_protocol() {
        let reply = call("initialize", json!({ "protocolVersion": "2025-03-26" }));
        assert_eq!(reply["id"], 7);
        assert_eq!(reply["result"]["protocolVersion"], "2025-03-26");
        assert!(reply["result"]["capabilities"]["tools"].is_object());
    }

    #[test]
    fn notifications_get_no_reply() {
        let note = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
        assert_eq!(handle("test", &Echo, &note), None);
    }

    #[test]
    fn tools_list_and_call() {
        assert_eq!(
            call("tools/list", json!({}))["result"]["tools"][0]["name"],
            "echo"
        );
        let ok = call(
            "tools/call",
            json!({ "name": "echo", "arguments": { "text": "hej" } }),
        );
        assert_eq!(ok["result"]["content"][0]["text"], "hej");
        assert_eq!(ok["result"]["isError"], false);
        let bad = call("tools/call", json!({ "name": "nope" }));
        assert_eq!(bad["result"]["isError"], true);
        assert_eq!(call("resources/list", json!({}))["error"]["code"], -32601);
    }

    fn post(url: &str, path: &str, body: &str) -> String {
        let address = url.trim_start_matches("http://").split('/').next().unwrap();
        let mut stream = TcpStream::connect(address).unwrap();
        write!(
            stream,
            "POST {path} HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\n\
             Accept: application/json, text/event-stream\r\nContent-Length: {}\r\n\
             Connection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut reply = String::new();
        stream.read_to_string(&mut reply).unwrap();
        reply
    }

    #[test]
    fn serves_over_http_only_on_its_path() {
        let running = serve("test", Echo).unwrap();
        let path = &running.url[running.url.find("/mcp/").unwrap()..];
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"echo","arguments":{"text":"x"}}}"#;
        let reply = post(&running.url, path, body);
        assert!(reply.starts_with("HTTP/1.1 200"), "{reply}");
        assert!(reply.contains(r#""text":"x""#), "{reply}");
        assert!(post(&running.url, "/mcp/guess", body).starts_with("HTTP/1.1 404"));
        let note = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        assert!(post(&running.url, path, note).starts_with("HTTP/1.1 202"));
    }
}
