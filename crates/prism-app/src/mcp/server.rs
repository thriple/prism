//! The MCP endpoint's socket (brief 21): `http://127.0.0.1:<port>/mcp`,
//! on its own thread. It checks the token and the `Origin`, speaks the
//! protocol, and sends tool calls to the main thread, waiting for them.

use std::io::Read;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use serde_json::{Value, json};
use tiny_http::{Header, Method, Request, Response};

use super::protocol::{self, Host, Output};
use super::{Call, Wake};

/// The largest request body read.
const MAX_BODY: u64 = 1 << 20;
/// How long a tool call may wait for the main thread.
const CALL_WAIT: Duration = Duration::from_secs(60);

pub struct Server {
    http: Arc<tiny_http::Server>,
    port: u16,
    pub calls: Receiver<Call>,
    stop: Arc<std::sync::atomic::AtomicBool>,
    thread: std::thread::JoinHandle<()>,
}

impl Server {
    /// Listen on the local machine only. Port 0 picks a free one (tests).
    pub fn start(port: u16, token: String, wake: Wake) -> Result<Server, String> {
        // A server just stopped on this port lets it go a moment later.
        let mut tries = 0;
        let http = loop {
            match tiny_http::Server::http(("127.0.0.1", port)) {
                Ok(h) => break h,
                Err(_) if tries < 20 => {
                    tries += 1;
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => {
                    return Err(format!(
                        "Could not listen on port {port} ({e}). Another program may be using it."
                    ));
                }
            }
        };
        let port = http.server_addr().to_ip().map_or(port, |a| a.port());
        let http = Arc::new(http);
        let (tx, calls) = channel();
        let h = http.clone();
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stopping = stop.clone();
        let thread = std::thread::Builder::new()
            .name("prism-mcp".into())
            .spawn(move || {
                loop {
                    match h.recv() {
                        Ok(request) => respond(request, &token, &tx, &wake),
                        Err(_) if stopping.load(std::sync::atomic::Ordering::SeqCst) => break,
                        // A failed connection: keep listening.
                        Err(_) => std::thread::sleep(Duration::from_millis(50)),
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Server {
            http,
            port,
            calls,
            stop,
            thread,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Stop listening and wait for the thread, so the port is free again.
    /// Calls not yet answered are dropped, which frees a request waiting
    /// on the main thread at once.
    pub fn stop(self) {
        drop(self.calls);
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        self.http.unblock();
        let _ = self.thread.join();
    }
}

fn header<'a>(r: &'a Request, name: &str) -> Option<&'a str> {
    r.headers()
        .iter()
        .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name))
        .map(|h| h.value.as_str())
}

/// Browsers send `Origin`; only pages on this machine may call.
pub fn local_origin(origin: &str) -> bool {
    let Some((scheme, rest)) = origin.split_once("://") else {
        return false;
    };
    if scheme != "http" && scheme != "https" {
        return false;
    }
    let host = if let Some(v6) = rest.strip_prefix('[') {
        v6.split(']').next().unwrap_or("")
    } else {
        rest.split(':').next().unwrap_or("")
    };
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

fn json_response(status: u16, body: Option<&Value>) -> Response<std::io::Cursor<Vec<u8>>> {
    let bytes = body.map(|b| b.to_string().into_bytes()).unwrap_or_default();
    let mut r = Response::from_data(bytes).with_status_code(status);
    if body.is_some() {
        r.add_header(Header::from_bytes("Content-Type", "application/json").expect("header"));
    }
    r
}

fn error_body(code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": null, "error": {"code": code, "message": message}})
}

/// Tool calls go to the main thread.
struct Remote<'a> {
    tx: &'a Sender<Call>,
    wake: &'a Wake,
}

impl Host for Remote<'_> {
    fn call(&mut self, name: &str, args: &Value) -> Result<Output, String> {
        let (reply, answer) = channel();
        self.tx
            .send(Call {
                name: name.to_owned(),
                args: args.clone(),
                reply,
            })
            .map_err(|_| "Prism is closing.".to_owned())?;
        (self.wake)();
        answer
            .recv_timeout(CALL_WAIT)
            .unwrap_or_else(|_| Err("Prism did not answer in time (a dialog may be open).".into()))
    }
}

fn respond(mut request: Request, token: &str, tx: &Sender<Call>, wake: &Wake) {
    let reply = (|| {
        if request.url().split('?').next() != Some("/mcp") {
            return json_response(
                404,
                Some(&error_body(-32600, "Not the MCP endpoint (/mcp)")),
            );
        }
        if let Some(o) = header(&request, "Origin")
            && !local_origin(o)
        {
            return json_response(403, Some(&error_body(-32600, "Origin not allowed")));
        }
        let expected = format!("Bearer {token}");
        if header(&request, "Authorization") != Some(expected.as_str()) {
            let mut r = json_response(401, Some(&error_body(-32600, "Missing or wrong token")));
            r.add_header(Header::from_bytes("WWW-Authenticate", "Bearer").expect("header"));
            return r;
        }
        if *request.method() != Method::Post {
            let mut r = json_response(405, None);
            r.add_header(Header::from_bytes("Allow", "POST").expect("header"));
            return r;
        }
        let mut body = String::new();
        if request
            .as_reader()
            .take(MAX_BODY)
            .read_to_string(&mut body)
            .is_err()
        {
            return json_response(400, Some(&error_body(-32700, "Could not read the body")));
        }
        let Ok(msg) = serde_json::from_str::<Value>(&body) else {
            return json_response(400, Some(&error_body(-32700, "Parse error")));
        };
        let version = header(&request, "MCP-Protocol-Version").map(str::to_owned);
        let r = protocol::handle(&msg, version.as_deref(), &mut Remote { tx, wake });
        json_response(r.status, r.body.as_ref())
    })();
    let _ = request.respond(reply);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn only_local_origins() {
        assert!(local_origin("http://localhost:3000"));
        assert!(local_origin("http://127.0.0.1"));
        assert!(local_origin("https://[::1]:8080"));
        assert!(!local_origin("https://evil.example"));
        assert!(!local_origin("http://localhost.evil.example"));
        assert!(!local_origin("null"));
    }

    /// Send a raw request; returns the status and the body.
    fn send(port: u16, method: &str, headers: &[&str], body: &str) -> (u16, String) {
        let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect");
        let mut req = format!("{method} /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n");
        for h in headers {
            req.push_str(h);
            req.push_str("\r\n");
        }
        req.push_str(&format!("Content-Length: {}\r\n\r\n{body}", body.len()));
        s.write_all(req.as_bytes()).expect("write");
        let mut out = String::new();
        s.read_to_string(&mut out).expect("read");
        let status = out
            .split_whitespace()
            .nth(1)
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);
        let body = out.split("\r\n\r\n").nth(1).unwrap_or("").to_owned();
        (status, body)
    }

    #[test]
    fn the_endpoint_checks_token_origin_and_method() {
        let server = Server::start(0, "secret".into(), Arc::new(|| {})).expect("starts");
        let port = server.port();
        let auth = "Authorization: Bearer secret";
        let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#;
        assert_eq!(send(port, "POST", &[], init).0, 401);
        assert_eq!(
            send(port, "POST", &["Authorization: Bearer nope"], init).0,
            401
        );
        assert_eq!(
            send(port, "POST", &[auth, "Origin: https://evil.example"], init).0,
            403
        );
        assert_eq!(send(port, "GET", &[auth], "").0, 405);
        let (status, body) = send(port, "POST", &[auth, "Origin: http://localhost"], init);
        assert_eq!(status, 200);
        let v: Value = serde_json::from_str(&body).expect("json");
        assert_eq!(v["result"]["protocolVersion"], "2025-06-18");
        let note = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        assert_eq!(send(port, "POST", &[auth], note).0, 202);
        server.stop();
        // The port is free again at once.
        let again = Server::start(port, "secret".into(), Arc::new(|| {})).expect("restarts");
        again.stop();
    }

    #[test]
    fn tool_calls_are_answered_by_the_main_thread() {
        let server = Server::start(0, "t".into(), Arc::new(|| {})).expect("starts");
        let port = server.port();
        let call = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_model","arguments":{}}}"#;
        let client =
            std::thread::spawn(move || send(port, "POST", &["Authorization: Bearer t"], call));
        // The "main thread": answer the one call.
        let c = server
            .calls
            .recv_timeout(Duration::from_secs(5))
            .expect("a call arrives");
        assert_eq!(c.name, "get_model");
        c.reply.send(Ok(Output::Text("a model".into()))).unwrap();
        let (status, body) = client.join().unwrap();
        assert_eq!(status, 200);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["result"]["content"][0]["text"], "a model");
        server.stop();
    }
}
