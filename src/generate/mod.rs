//! AI app generation: the create-bar's backend. An agent writes Splash
//! source; the pipeline validates and packages it. The agent sits behind the
//! [`AgentTransport`] trait — the default backend spawns an external ACP
//! process (octos by default; `ssh host octos acp` works too, since stdio
//! composes), and the `agent-octos` cargo feature adds an in-process backend
//! that links the octos agent crates directly (no child process — the only
//! option on iOS, where exec() is prohibited).

pub mod acp_client;
#[cfg(feature = "agent-octos")]
pub mod octos_inproc;
pub mod pipeline;
pub mod setup;
#[cfg(feature = "agent-skills")]
pub mod skills;

use acp_client::{AcpClient, AcpEvent};

/// The Splash dialect guide — the entire "app-card memory" teaching the agent
/// THIS repo's dialect. Inlined into prompts by default; `agent-skills`
/// instead injects it persistently on the agent side and sends slim prompts.
pub(crate) const SPLASH_GUIDE: &str = include_str!("splash_guide.md");

/// What the generation pipeline needs from an agent, regardless of where it
/// runs. Implementations push `AcpEvent`s to an internal queue from their own
/// threads (waking the UI via `SignalToUI`); the UI thread drains.
pub trait AgentTransport: Send {
    /// Drains every queued event. Called from the UI thread's event handler.
    fn drain_events(&mut self) -> Vec<AcpEvent>;
    /// Sends the user's (or a repair) prompt on the current session.
    fn send_prompt(&mut self, text: &str);
    /// Asks the agent to abandon the in-flight turn.
    fn cancel(&mut self);
    /// Human-readable description of the backend, for error messages.
    fn desc(&self) -> &str;
}

impl AgentTransport for AcpClient {
    fn drain_events(&mut self) -> Vec<AcpEvent> {
        AcpClient::drain_events(self)
    }
    fn send_prompt(&mut self, text: &str) {
        AcpClient::send_prompt(self, text)
    }
    fn cancel(&mut self) {
        AcpClient::cancel(self)
    }
    fn desc(&self) -> &str {
        self.cmd_desc()
    }
}

/// Well-known provider API-key env vars → the octos provider they imply, in
/// preference order. Lets the launcher work with ZERO octos setup: a key
/// already exported in the shell is enough — the provider name is inferred
/// here and passed as a flag (external agent) or used directly (in-process);
/// the key itself is read from the environment by octos's provider, never
/// stored or forwarded by the launcher.
pub(crate) const PROVIDER_KEY_ENVS: &[(&str, &str)] = &[
    ("ANTHROPIC_API_KEY", "anthropic"),
    ("OPENAI_API_KEY", "openai"),
    ("GEMINI_API_KEY", "gemini"),
    ("OPENROUTER_API_KEY", "openrouter"),
    ("DEEPSEEK_API_KEY", "deepseek"),
    ("GROQ_API_KEY", "groq"),
    ("MOONSHOT_API_KEY", "moonshot"),
];

/// The provider implied by the environment, when no octos config chose one.
pub(crate) fn provider_from_env() -> Option<&'static str> {
    provider_from_lookup(|var| std::env::var(var).ok())
}

/// The provider implied by octos's auth store (`octos auth login` pastes a
/// key into `auth.json` without writing a config), so logged-in users need no
/// further setup either. Prefers anthropic, else the first stored provider.
pub(crate) fn provider_from_auth_store() -> Option<String> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Some(dir) = std::env::var_os("OCTOS_CONFIG_DIR") {
        candidates.push(std::path::PathBuf::from(dir).join("auth.json"));
    }
    if let Some(home) = std::env::var_os("HOME").map(std::path::PathBuf::from) {
        candidates.push(home.join(".config").join("octos").join("auth.json"));
        candidates.push(home.join(".octos").join("auth.json"));
    }
    for path in candidates {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let Some(creds) = value.get("credentials").and_then(|c| c.as_object()) else {
            continue;
        };
        if creds.contains_key("anthropic") {
            return Some("anthropic".to_string());
        }
        if let Some(name) = creds.keys().next() {
            return Some(name.clone());
        }
    }
    None
}

fn provider_from_lookup(get: impl Fn(&str) -> Option<String>) -> Option<&'static str> {
    PROVIDER_KEY_ENVS
        .iter()
        .find(|(var, _)| get(var).is_some_and(|v| !v.is_empty()))
        .map(|(_, provider)| *provider)
}

/// Fully local, no-key path: if an Ollama server is running on the default
/// port, use it (octos's `ollama` provider needs no key). Returns the model
/// to use, picked from what's actually pulled — coder-ish models first, since
/// the job is writing Splash code. Std-only probe: a short TCP connect + a
/// minimal HTTP/1.0 GET (which forces a non-chunked, close-delimited reply),
/// bounded to ~0.7s worst case — and it only runs when every other detection
/// came up empty.
pub(crate) fn ollama_model() -> Option<String> {
    ollama_model_at(11434)
}

fn ollama_model_at(port: u16) -> Option<String> {
    use std::io::{Read, Write};
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream =
        std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(150)).ok()?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_millis(500)))
        .ok()?;
    stream
        .write_all(b"GET /api/tags HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n")
        .ok()?;
    let mut response = Vec::new();
    let _ = stream.take(1 << 20).read_to_end(&mut response);
    let response = String::from_utf8_lossy(&response);
    let body = response.split_once("\r\n\r\n").map(|(_, b)| b)?;
    pick_ollama_model(body)
}

/// Chooses the best generation model from an Ollama `/api/tags` body.
fn pick_ollama_model(tags_json: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(tags_json).ok()?;
    let names: Vec<String> = value
        .get("models")?
        .as_array()?
        .iter()
        .filter_map(|m| m.get("name").and_then(|n| n.as_str()).map(str::to_string))
        .collect();
    // Splash generation is a coding task; prefer code-tuned models.
    for hint in ["coder", "qwen", "deepseek", "codellama", "llama"] {
        if let Some(name) = names.iter().find(|n| n.to_lowercase().contains(hint)) {
            return Some(name.clone());
        }
    }
    names.first().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_detection_prefers_anthropic_and_skips_empty() {
        let get = |var: &str| match var {
            "ANTHROPIC_API_KEY" => Some("sk-ant-x".to_string()),
            "OPENAI_API_KEY" => Some("sk-x".to_string()),
            _ => None,
        };
        assert_eq!(provider_from_lookup(get), Some("anthropic"));

        // An exported-but-empty key must not win.
        let get = |var: &str| match var {
            "ANTHROPIC_API_KEY" => Some(String::new()),
            "GROQ_API_KEY" => Some("gsk-x".to_string()),
            _ => None,
        };
        assert_eq!(provider_from_lookup(get), Some("groq"));

        assert_eq!(provider_from_lookup(|_| None), None);
    }

    #[test]
    fn ollama_model_pick_prefers_coder_models() {
        let tags = r#"{"models":[
            {"name":"llama3.2:latest"},
            {"name":"qwen2.5-coder:14b"},
            {"name":"mistral:7b"}
        ]}"#;
        assert_eq!(pick_ollama_model(tags).as_deref(), Some("qwen2.5-coder:14b"));

        let tags = r#"{"models":[{"name":"mistral:7b"},{"name":"llama3.1:8b"}]}"#;
        assert_eq!(pick_ollama_model(tags).as_deref(), Some("llama3.1:8b"));

        // No hint match: first model wins; empty list: none.
        let tags = r#"{"models":[{"name":"gemma:2b"}]}"#;
        assert_eq!(pick_ollama_model(tags).as_deref(), Some("gemma:2b"));
        assert_eq!(pick_ollama_model(r#"{"models":[]}"#), None);
        assert_eq!(pick_ollama_model("not json"), None);
    }

    /// The probe against a real (fake-Ollama) listener, and against a closed
    /// port — both must resolve promptly.
    #[test]
    fn ollama_probe_reads_tags_and_fails_fast() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((mut sock, _)) = listener.accept() {
                let mut buf = [0u8; 512];
                let _ = sock.read(&mut buf); // the GET
                let body = r#"{"models":[{"name":"qwen2.5-coder:14b"}]}"#;
                let _ = write!(
                    sock,
                    "HTTP/1.0 200 OK\r\nContent-Type: application/json\r\n\r\n{body}"
                );
            }
        });
        assert_eq!(ollama_model_at(port).as_deref(), Some("qwen2.5-coder:14b"));

        // Closed port: prompt None (localhost refuses immediately).
        let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let closed_port = closed.local_addr().unwrap().port();
        drop(closed);
        let start = std::time::Instant::now();
        assert_eq!(ollama_model_at(closed_port), None);
        assert!(start.elapsed() < std::time::Duration::from_secs(1));
    }
}

/// Whether any octos config file exists (same resolution order octos uses).
/// When one does, the user's own setup wins and no auto-detection happens.
pub(crate) fn octos_config_exists() -> bool {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Some(dir) = std::env::var_os("OCTOS_CONFIG_DIR") {
        candidates.push(std::path::PathBuf::from(dir).join("config.json"));
    }
    if let Some(home) = std::env::var_os("HOME").map(std::path::PathBuf::from) {
        candidates.push(home.join(".config").join("octos").join("config.json"));
        candidates.push(home.join(".octos").join("config.json"));
    }
    candidates.iter().any(|p| p.exists())
}

/// Picks and starts the agent backend for one generation.
///
/// Selection: `HOST_LAUNCHER_AGENT_CMD` always wins and always means "spawn
/// this external ACP command" — the explicit override, and how the offline
/// test agent (`fake_acp`) is injected even in `agent-octos` builds. With no
/// override, the in-process backend is used when compiled in, else an
/// external `octos acp` is spawned — with the provider auto-detected from
/// the environment when octos was never configured, so an exported API key
/// is the ONLY setup needed.
pub fn start_backend(workspace: &std::path::Path) -> Result<Box<dyn AgentTransport>, String> {
    if let Ok(cmd) = std::env::var("HOST_LAUNCHER_AGENT_CMD") {
        return Ok(Box::new(AcpClient::spawn(&cmd, workspace)?));
    }
    #[cfg(feature = "agent-octos")]
    {
        return Ok(Box::new(octos_inproc::InProcessOctos::start(workspace)?));
    }
    #[cfg(not(feature = "agent-octos"))]
    {
        let cmd = if !octos_config_exists() {
            if let Some(provider) = provider_from_env()
                .map(str::to_string)
                .or_else(provider_from_auth_store)
            {
                format!("octos acp --provider {provider}")
            } else if let Some(model) = ollama_model() {
                // Fully local fallback: a running Ollama needs no key at all.
                format!("octos acp --provider ollama --model {model}")
            } else {
                "octos acp".to_string() // fails with octos's own setup hint
            }
        } else {
            "octos acp".to_string()
        };
        Ok(Box::new(AcpClient::spawn(&cmd, workspace)?))
    }
}
