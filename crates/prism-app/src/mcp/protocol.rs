//! The Model Context Protocol messages (brief 21), with no sockets: one
//! JSON-RPC message in, the HTTP status and body out. Both eras are spoken:
//! the `initialize` handshake (2025-03-26 to 2025-11-25) and the stateless
//! 2026-07-28 revision (version in each request's `_meta`).

use serde_json::{Value, json};

/// The stateless revision.
pub const MODERN: &str = "2026-07-28";
/// Versions that start with `initialize`, newest first.
pub const LEGACY: [&str; 3] = ["2025-11-25", "2025-06-18", "2025-03-26"];

const VERSION_KEY: &str = "io.modelcontextprotocol/protocolVersion";
const SERVER_INFO_KEY: &str = "io.modelcontextprotocol/serverInfo";

/// What a tool gives back.
#[derive(Clone, Debug, PartialEq)]
pub enum Output {
    Text(String),
    /// A PNG, and a line about it.
    Image {
        png: Vec<u8>,
        text: String,
    },
}

impl From<String> for Output {
    fn from(s: String) -> Self {
        Output::Text(s)
    }
}

/// What runs the tools (the app, or a fake in tests).
pub trait Host {
    /// Run tool `name`. `Err` is a tool error the assistant sees (not a
    /// protocol error).
    fn call(&mut self, name: &str, args: &Value) -> Result<Output, String>;
}

/// The HTTP answer: a status, and a JSON body unless there is none.
#[derive(Clone, Debug, PartialEq)]
pub struct Reply {
    pub status: u16,
    pub body: Option<Value>,
}

impl Reply {
    fn ok(id: &Value, result: Value) -> Reply {
        Reply {
            status: 200,
            body: Some(json!({"jsonrpc": "2.0", "id": id, "result": result})),
        }
    }

    fn error(status: u16, id: &Value, code: i64, message: &str, data: Option<Value>) -> Reply {
        let mut e = json!({"code": code, "message": message});
        if let Some(d) = data {
            e["data"] = d;
        }
        Reply {
            status,
            body: Some(json!({"jsonrpc": "2.0", "id": id, "error": e})),
        }
    }
}

fn server_info() -> Value {
    json!({"name": "Prism", "version": env!("CARGO_PKG_VERSION")})
}

const INSTRUCTIONS: &str = "Prism is a 3D modeler. get_model describes the open document; \
    list_actions shows what can run now; run_action runs one exactly as a menu click would, \
    and every change can be undone with run_action edit.undo.";

/// The tools, in a fixed order.
pub fn tools() -> Value {
    let none = json!({"type": "object", "properties": {}});
    json!([
        {
            "name": "get_model",
            "title": "Describe the model",
            "description": "The open document: units, the tool, counts of faces, edges and points, the open group, the groups and components (as the Outliner shows them), the view style, and whether Undo and Redo can run.",
            "inputSchema": none,
        },
        {
            "name": "get_selection",
            "title": "Describe the selection",
            "description": "What is selected, as Entity Info shows it: a group or component (name, kind, tag, hidden), or loose geometry (counts and face area).",
            "inputSchema": none,
        },
        {
            "name": "get_health",
            "title": "Model Health",
            "description": "Model Health's checks on the open group's model: each check, how many problems, details, and whether a fix exists.",
            "inputSchema": none,
        },
        {
            "name": "list_actions",
            "title": "List actions",
            "description": "Prism's actions (the same ones as the menus, toolbars and keys): id, name, menu path, key, and whether it can run now (and why not). Filter with an optional query matched against id, name and menu path.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Words to match, such as \"group\" or \"view\"."}
                },
            },
        },
        {
            "name": "get_geometry",
            "title": "List faces and edges",
            "description": "The open group's faces (id, area in square metres, outer loop points, number of holes, and a flat face's unit normal or a curved face's `cylinder` (axis origin, unit axis, radius), `cone` (apex, unit axis into the cone, half angle in degrees), `sphere` (centre, the axis it was turned about, radius) or `torus` (centre, unit axis, major and minor radius), each with whether it faces outward) and edges (id, end points, length; an arc also gives its centre, axis and radius, and the edges of one circle, arc or polygon share a `curve` number; `soft` edges are not drawn and shading runs smoothly across `smooth` ones; `approx` edges are fine straight segments standing in for a curve that is not a line or arc), in world coordinates and metres. Up to `limit` of each (default 200); the totals are always given.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": {"type": "integer", "minimum": 0, "description": "Most faces and most edges to list."}
                },
            },
        },
        {
            "name": "use_tool",
            "title": "Use a tool",
            "description": "Use Prism's tools as a person would. Optionally pick `tool` (an id such as tool.line, tool.rect, tool.pushpull, tool.move; see list_actions; tool.circle, tool.polygon and the arcs work here although greyed in the menus: circle and polygon take the centre, then the radius, sides typed as 8s; arc.2 takes start, end, then bulge, or a typed bulge or radius as 2r; arc.3 three points on it; arc.c and arc.p the centre, the start, then the end, or a typed radius and angle in degrees), then run `steps` in order. A step is one of: {\"click\": [x, y, z]} (press and release at a model point, in metres), {\"move\": [x, y, z]} (hover there, for direction and inference), {\"type\": \"2.5\"} (type into the measurement box and press Enter; units such as 400mm work), {\"key\": \"Enter\"|\"Escape\"|\"Backspace\"}, {\"lock\": \"red\"|\"green\"|\"blue\"} (lock an axis). Points are projected through the current view, so they must be visible; inference snaps a click to nearby points, edges and faces, and each click reports where it landed. Exact sizes come from typed values. Every change is undoable (run_action edit.undo).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tool": {"type": "string", "description": "A tool's action id, picked before the steps."},
                    "steps": {
                        "type": "array",
                        "items": {"type": "object"},
                        "description": "Clicks, moves, typed values, keys and axis locks, in order."
                    }
                },
            },
        },
        {
            "name": "screenshot",
            "title": "Screenshot",
            "description": "A picture of the viewport as it is after the latest change (PNG, at most 1024 pixels wide).",
            "inputSchema": none,
        },
        {
            "name": "replay_session",
            "title": "Replay a recorded session",
            "description": "Play a session recorded with Help > Record Session, from a file path or its JSON text, in a separate session (the open document is not changed). Says whether the end matches what was recorded, and the end's counts.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "The recording's file on this computer."},
                    "json": {"type": "string", "description": "Or the recording's JSON text."}
                },
            },
        },
        {
            "name": "run_action",
            "title": "Run an action",
            "description": "Run one action by id (from list_actions), exactly as clicking it in a menu would. Tools such as tool.line are picked, not used. Changes go through the undo history: run_action edit.undo takes one back.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "The action's id, such as edit.selall or view.xray."}
                },
                "required": ["id"],
            },
        },
    ])
}

/// Handle one message. `header` is the `MCP-Protocol-Version` header.
pub fn handle(msg: &Value, header: Option<&str>, host: &mut dyn Host) -> Reply {
    let null = Value::Null;
    let Some(method) = msg.get("method").and_then(Value::as_str) else {
        return Reply::error(400, &null, -32600, "Invalid request", None);
    };
    // A notification (no id): accepted, nothing to answer.
    let Some(id) = msg.get("id") else {
        return Reply {
            status: 202,
            body: None,
        };
    };
    let params = msg.get("params").cloned().unwrap_or(json!({}));
    let meta_version = params
        .get("_meta")
        .and_then(|m| m.get(VERSION_KEY))
        .and_then(Value::as_str);

    // The stateless revision names its version in every request.
    let modern = match meta_version {
        Some(v) => {
            if let Some(h) = header
                && h != v
            {
                return Reply::error(
                    400,
                    id,
                    -32020,
                    &format!(
                        "Header mismatch: MCP-Protocol-Version header value '{h}' does not match body value '{v}'"
                    ),
                    None,
                );
            }
            if v != MODERN {
                return Reply::error(
                    400,
                    id,
                    -32022,
                    "Unsupported protocol version",
                    Some(json!({"supported": [MODERN], "requested": v})),
                );
            }
            true
        }
        None => false,
    };
    let finish = |mut result: Value| {
        result["resultType"] = json!("complete");
        if modern {
            result["_meta"] = json!({ SERVER_INFO_KEY: server_info() });
        }
        Reply::ok(id, result)
    };

    match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str);
            let version = asked.filter(|v| LEGACY.contains(v)).unwrap_or(LEGACY[0]);
            finish(json!({
                "protocolVersion": version,
                "capabilities": {"tools": {}},
                "serverInfo": server_info(),
                "instructions": INSTRUCTIONS,
            }))
        }
        "server/discover" => finish(json!({
            "supportedVersions": [MODERN],
            "capabilities": {"tools": {}},
            "instructions": INSTRUCTIONS,
            "ttlMs": 3_600_000,
            "cacheScope": "private",
        })),
        "ping" => finish(json!({})),
        "tools/list" => finish(json!({
            "tools": tools(),
            "ttlMs": 3_600_000,
            "cacheScope": "private",
        })),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let known = tools()
                .as_array()
                .is_some_and(|t| t.iter().any(|t| t["name"] == name));
            if !known {
                return Reply::error(200, id, -32602, &format!("Unknown tool: {name}"), None);
            }
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            let (content, is_error) = match host.call(name, &args) {
                Ok(Output::Text(t)) => (json!([{"type": "text", "text": t}]), false),
                Ok(Output::Image { png, text }) => (
                    json!([
                        {"type": "image", "data": super::image::base64(&png), "mimeType": "image/png"},
                        {"type": "text", "text": text},
                    ]),
                    false,
                ),
                Err(e) => (json!([{"type": "text", "text": e}]), true),
            };
            finish(json!({
                "content": content,
                "isError": is_error,
            }))
        }
        _ => Reply::error(
            if modern { 404 } else { 200 },
            id,
            -32601,
            &format!("Method not found: {method}"),
            None,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake(Vec<String>);
    impl Host for Fake {
        fn call(&mut self, name: &str, args: &Value) -> Result<Output, String> {
            self.0.push(name.to_owned());
            if name == "screenshot" {
                return Ok(Output::Image {
                    png: b"foo".to_vec(),
                    text: "800 x 600".into(),
                });
            }
            match args.get("id").and_then(Value::as_str) {
                Some("bad") => Err("greyed".into()),
                _ => Ok(format!("ran {name}").into()),
            }
        }
    }

    fn req(method: &str, params: Value) -> Value {
        json!({"jsonrpc": "2.0", "id": 7, "method": method, "params": params})
    }

    fn modern(method: &str, mut params: Value) -> Value {
        params["_meta"] = json!({ VERSION_KEY: MODERN });
        req(method, params)
    }

    fn result(r: &Reply) -> &Value {
        &r.body.as_ref().expect("a body")["result"]
    }

    #[test]
    fn initialize_answers_with_a_version_it_knows() {
        let mut h = Fake(vec![]);
        let r = handle(
            &req("initialize", json!({"protocolVersion": "2025-06-18"})),
            None,
            &mut h,
        );
        assert_eq!(r.status, 200);
        assert_eq!(result(&r)["protocolVersion"], "2025-06-18");
        assert_eq!(result(&r)["serverInfo"]["name"], "Prism");
        assert!(result(&r)["capabilities"]["tools"].is_object());
        let r = handle(
            &req("initialize", json!({"protocolVersion": "1999-01-01"})),
            None,
            &mut h,
        );
        assert_eq!(result(&r)["protocolVersion"], LEGACY[0]);
    }

    #[test]
    fn notifications_get_202_and_no_body() {
        let r = handle(
            &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            None,
            &mut Fake(vec![]),
        );
        assert_eq!(
            r,
            Reply {
                status: 202,
                body: None
            }
        );
    }

    #[test]
    fn discover_and_list_in_the_new_revision() {
        let mut h = Fake(vec![]);
        let r = handle(&modern("server/discover", json!({})), Some(MODERN), &mut h);
        assert_eq!(result(&r)["supportedVersions"], json!([MODERN]));
        assert_eq!(result(&r)["resultType"], "complete");
        assert_eq!(result(&r)["_meta"][SERVER_INFO_KEY]["name"], "Prism");
        let r = handle(&modern("tools/list", json!({})), None, &mut h);
        let names: Vec<_> = result(&r)["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(
            names,
            [
                "get_model",
                "get_selection",
                "get_health",
                "list_actions",
                "get_geometry",
                "use_tool",
                "screenshot",
                "replay_session",
                "run_action"
            ]
        );
        assert!(result(&r)["ttlMs"].is_number());
        assert_eq!(result(&r)["cacheScope"], "private");
    }

    #[test]
    fn tool_calls_reach_the_host_and_errors_are_tool_errors() {
        let mut h = Fake(vec![]);
        let call = |name: &str, id: &str| {
            req("tools/call", json!({"name": name, "arguments": {"id": id}}))
        };
        let r = handle(&call("run_action", "edit.selall"), None, &mut h);
        assert_eq!(result(&r)["content"][0]["text"], "ran run_action");
        assert_eq!(result(&r)["isError"], false);
        let r = handle(&call("run_action", "bad"), None, &mut h);
        assert_eq!(result(&r)["isError"], true);
        assert_eq!(result(&r)["content"][0]["text"], "greyed");
        // A picture comes back as image content.
        let r = handle(&call("screenshot", ""), None, &mut h);
        let c = &result(&r)["content"];
        assert_eq!(c[0]["type"], "image");
        assert_eq!(c[0]["mimeType"], "image/png");
        assert_eq!(c[0]["data"], "Zm9v");
        assert_eq!(c[1]["text"], "800 x 600");
        let r = handle(&call("no_such_tool", ""), None, &mut h);
        assert_eq!(r.body.unwrap()["error"]["code"], -32602);
        assert_eq!(h.0, ["run_action", "run_action", "screenshot"]);
    }

    #[test]
    fn unknown_methods_and_versions() {
        let mut h = Fake(vec![]);
        // Older clients: a JSON-RPC error in a 200.
        let r = handle(&req("resources/list", json!({})), None, &mut h);
        assert_eq!(
            (r.status, r.body.unwrap()["error"]["code"].clone()),
            (200, json!(-32601))
        );
        // The new revision: 404.
        let r = handle(&modern("resources/list", json!({})), None, &mut h);
        assert_eq!(r.status, 404);
        // A version we do not speak.
        let mut m = req("tools/list", json!({}));
        m["params"]["_meta"] = json!({ VERSION_KEY: "2099-01-01" });
        let r = handle(&m, None, &mut h);
        assert_eq!(r.status, 400);
        let e = &r.body.unwrap()["error"];
        assert_eq!(e["code"], -32022);
        assert_eq!(e["data"]["supported"], json!([MODERN]));
        // The header and the body disagree.
        let r = handle(&modern("tools/list", json!({})), Some("2025-11-25"), &mut h);
        assert_eq!(r.status, 400);
        assert_eq!(r.body.unwrap()["error"]["code"], -32020);
        // Not JSON-RPC at all.
        assert_eq!(handle(&json!({"x": 1}), None, &mut h).status, 400);
    }
}
