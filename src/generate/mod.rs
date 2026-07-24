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
const PROVIDER_KEY_ENVS: &[(&str, &str)] = &[
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

fn provider_from_lookup(get: impl Fn(&str) -> Option<String>) -> Option<&'static str> {
    PROVIDER_KEY_ENVS
        .iter()
        .find(|(var, _)| get(var).is_some_and(|v| !v.is_empty()))
        .map(|(_, provider)| *provider)
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
            match provider_from_env() {
                Some(provider) => format!("octos acp --provider {provider}"),
                None => "octos acp".to_string(), // fails with octos's own setup hint
            }
        } else {
            "octos acp".to_string()
        };
        Ok(Box::new(AcpClient::spawn(&cmd, workspace)?))
    }
}
