//! Habla MCP por stdio con el binario real: initialize, tools/list y unas llamadas.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

struct Client {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Client {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_chimpsprite-mcp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("server starts");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            stdin,
            stdout,
            next_id: 1,
        }
    }

    fn send(&mut self, msg: Value) {
        writeln!(self.stdin, "{msg}").unwrap();
        self.stdin.flush().unwrap();
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        loop {
            let mut line = String::new();
            assert!(
                self.stdout.read_line(&mut line).unwrap() > 0,
                "server closed"
            );
            let msg: Value = serde_json::from_str(&line).expect("json line");
            if msg["id"] == json!(id) {
                assert!(msg.get("error").is_none(), "{msg}");
                return msg["result"].clone();
            }
        }
    }

    fn call(&mut self, tool: &str, args: Value) -> Value {
        self.request("tools/call", json!({"name": tool, "arguments": args}))
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.child.kill().ok();
    }
}

#[test]
fn draw_and_render_over_stdio() {
    let mut c = Client::start();
    let init = c.request(
        "initialize",
        json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "test", "version": "0"},
        }),
    );
    assert_eq!(init["serverInfo"]["name"], "chimpsprite");
    assert!(init["instructions"].as_str().unwrap().contains("render"));
    c.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));

    let tools = c.request("tools/list", json!({}));
    let names: Vec<&str> = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for expected in [
        "new_document",
        "draw_line",
        "render",
        "read_pixels",
        "inbetween",
        "convert_layer",
        "export",
    ] {
        assert!(
            names.contains(&expected),
            "{expected} missing from {names:?}"
        );
    }

    let created = c.call("new_document", json!({"width": 8, "height": 8}));
    assert!(
        created["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("d1")
    );
    let drawn = c.call(
        "draw_line",
        json!({"doc": "d1", "points": [[0, 0], [7, 7]], "color": "#ff0000"}),
    );
    assert_eq!(drawn["isError"], false, "{drawn}");

    let rendered = c.call("render", json!({"doc": "d1"}));
    assert_eq!(rendered["content"][1]["type"], "image");
    assert_eq!(rendered["content"][1]["mimeType"], "image/png");

    let read = c.call(
        "read_pixels",
        json!({"doc": "d1", "rect": {"x": 0, "y": 0, "w": 2, "h": 2}}),
    );
    assert!(
        read["content"][0]["text"]
            .as_str()
            .unwrap()
            .ends_with("rows:\na.\n.a\n")
    );

    let failed = c.call(
        "draw_line",
        json!({"doc": "d9", "points": [[0, 0]], "color": "#f00"}),
    );
    assert_eq!(failed["isError"], true);
    assert!(
        failed["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("unknown document")
    );
}
