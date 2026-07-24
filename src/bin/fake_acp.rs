//! A fake ACP agent for offline tests of the create-app pipeline.
//!
//! Speaks just enough newline-delimited JSON-RPC to stand in for `octos acp`:
//! `initialize` → `session/new` → `session/prompt` with streamed
//! `session/update` chunks and a final `stopReason` reply. Deterministic and
//! LLM-free; the scenario is chosen by what the prompt text contains:
//!
//! - contains `"refuse"` → replies with `stopReason: "refusal"`.
//! - contains `"broken"` → first prompt streams an invalid Splash script (the
//!   validator must reject it); the repair prompt (recognized by the phrase
//!   "failed to compile") then streams a valid one.
//! - anything else → streams a valid pomodoro-ish app, split across two
//!   chunks mid-fence to exercise accumulation.
//!
//! Point the launcher at it with
//! `HOST_LAUNCHER_AGENT_CMD=$PWD/target/debug/fake_acp`.

use std::io::{BufRead, Write};

use serde_json::{json, Value};

const GOOD_APP: &str = "// name: Pomodoro\n// icon: 🍅\n// tint: #E84D3D\n\
let secs = 1500\n\
fn show(){ ui.time.set_text(\"\" + secs) }\n\
View{\n\
    width: Fill height: Fit flow: Down spacing: 12 padding: 16\n\
    align: Align{x: 0.5}\n\
    glass.H1{text: \"Pomodoro\"}\n\
    time := Label{text: \"1500\"}\n\
    glass.GlassButton{text: \"Tick\" width: 90 height: 44 on_click: || { secs -= 1 show() }}\n\
}";

const BROKEN_APP: &str = "// name: Pomodoro\n// icon: 🍅\n// tint: #E84D3D\n\
DefinitelyNotAWidget{ &&& this does not parse }";

fn send(out: &mut impl Write, v: Value) {
    let mut line = v.to_string();
    line.push('\n');
    out.write_all(line.as_bytes()).unwrap();
    out.flush().unwrap();
}

fn chunk(out: &mut impl Write, session_id: &str, text: &str) {
    send(
        out,
        json!({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {
                "sessionId": session_id,
                "update": {
                    "sessionUpdate": "agent_message_chunk",
                    "content": {"type": "text", "text": text},
                },
            },
        }),
    );
}

fn main() {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let session_id = "fake-session-1";

    for line in stdin.lock().lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(msg) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let id = msg.get("id").cloned().unwrap_or(Value::Null);
        match method {
            "initialize" => send(
                &mut stdout,
                json!({
                    "jsonrpc": "2.0", "id": id,
                    "result": {"protocolVersion": 1, "agentCapabilities": {}},
                }),
            ),
            "session/new" => send(
                &mut stdout,
                json!({
                    "jsonrpc": "2.0", "id": id,
                    "result": {"sessionId": session_id},
                }),
            ),
            "session/prompt" => {
                let text = msg
                    .pointer("/params/prompt/0/text")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if text.contains("refuse") {
                    send(
                        &mut stdout,
                        json!({
                            "jsonrpc": "2.0", "id": id,
                            "result": {"stopReason": "refusal"},
                        }),
                    );
                    continue;
                }
                // Before answering, fire an agent-originated request with a
                // STRING id (legal JSON-RPC) — the client must refuse it and
                // carry on; a client that drops it would leave a real agent
                // hanging. The reply itself is read below and discarded.
                send(
                    &mut stdout,
                    json!({
                        "jsonrpc": "2.0", "id": "agent-req-1",
                        "method": "session/request_permission",
                        "params": {"sessionId": session_id},
                    }),
                );

                // The repair prompt is recognizable by the pipeline's own copy.
                let is_repair = text.contains("failed to compile");
                let source = if text.contains("broken") && !is_repair {
                    BROKEN_APP
                } else {
                    GOOD_APP
                };
                let reply = format!("```splash\n{source}\n```");
                // Split mid-reply so the client has to accumulate chunks.
                let mid = reply.len() / 2;
                let mid = (mid..reply.len())
                    .find(|&i| reply.is_char_boundary(i))
                    .unwrap_or(reply.len());
                chunk(&mut stdout, session_id, &reply[..mid]);
                chunk(&mut stdout, session_id, &reply[mid..]);
                send(
                    &mut stdout,
                    json!({
                        "jsonrpc": "2.0", "id": id,
                        "result": {"stopReason": "end_turn"},
                    }),
                );
            }
            // No method: it's the client's RESPONSE to our agent-originated
            // request above — consume silently (replying to a reply is a
            // JSON-RPC violation).
            "" => {}
            // Notifications (session/cancel) and unknown requests: unknown
            // *requests* still get a reply so nothing hangs.
            _ => {
                if !id.is_null() {
                    send(
                        &mut stdout,
                        json!({
                            "jsonrpc": "2.0", "id": id,
                            "error": {"code": -32601, "message": "not supported"},
                        }),
                    );
                }
            }
        }
    }
}
