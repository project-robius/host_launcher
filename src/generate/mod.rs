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

/// Picks and starts the agent backend for one generation.
///
/// Selection: `HOST_LAUNCHER_AGENT_CMD` always wins and always means "spawn
/// this external ACP command" — the explicit override, and how the offline
/// test agent (`fake_acp`) is injected even in `agent-octos` builds. With no
/// override, the in-process backend is used when compiled in, else an
/// external `octos acp` is spawned.
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
        Ok(Box::new(AcpClient::spawn("octos acp", workspace)?))
    }
}
