//! `agent-octos`: the octos agent running IN-PROCESS, no child process.
//!
//! Links the octos agent core (provider registry, tool registry + sandbox,
//! agent loop) directly and drives it on a dedicated thread that owns a tokio
//! runtime — the same shape `octos acp` itself uses, minus the process
//! boundary. This is the only local option on iOS, where `exec()` is
//! prohibited; on desktop it also removes the "install octos first" step
//! (only `~/.octos/config.json` — an `octos init` from any machine — is
//! needed for the provider).
//!
//! Faithfully mirrors `octos acp`'s ConfigAgentFactory in miniature:
//! provider from config.json via the octos-llm registry (+ RetryProvider),
//! builtin tools rooted at the workspace under the platform sandbox, an
//! episodic store on a launcher-private dir (degraded-open so a user's own
//! octos process never contends), the per-turn reporter with the
//! StreamChunk/Response dedupe, and octos's exact history-append rules.
//! Deliberately NOT replicated (use the external `octos acp` for these):
//! fallback-model routing, OAuth auth-store keys, keychain: markers, MCP,
//! plugins/skills, memory-bank tools.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::Arc;

use makepad_widgets::SignalToUI;

use crate::generate::acp_client::AcpEvent;
use crate::generate::AgentTransport;

/// The launcher-private octos data dir (episodes DB). Separate from ~/.octos
/// so an octos process the user runs themselves never fights us for the redb
/// lock (and we open degraded regardless).
fn octos_data_dir() -> PathBuf {
    let base = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join(".host_launcher").join("octos_data")
}

enum Cmd {
    Prompt(String),
}

/// In-process octos agent behind the same event interface as the ACP client.
pub struct InProcessOctos {
    events: Receiver<AcpEvent>,
    cmd_tx: Sender<Cmd>,
    shutdown: Arc<AtomicBool>,
}

impl InProcessOctos {
    pub fn start(workspace: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(workspace).ok();
        let (evt_tx, events) = std::sync::mpsc::channel();
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
        let shutdown = Arc::new(AtomicBool::new(false));
        let ws = workspace.to_path_buf();
        let sd = shutdown.clone();
        std::thread::spawn(move || agent_thread(ws, cmd_rx, evt_tx, sd));
        Ok(Self { events, cmd_tx, shutdown })
    }
}

impl AgentTransport for InProcessOctos {
    fn drain_events(&mut self) -> Vec<AcpEvent> {
        let mut out = Vec::new();
        loop {
            match self.events.try_recv() {
                Ok(e) => out.push(e),
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }
        out
    }

    fn send_prompt(&mut self, text: &str) {
        // Clear a stale cancel flag HERE (not inside the turn) so a cancel or
        // drop that lands after this queue-up still wins — mirroring octos
        // acp's own dispatch-loop reset ordering.
        self.shutdown.store(false, Ordering::Release);
        let _ = self.cmd_tx.send(Cmd::Prompt(text.to_string()));
    }

    fn cancel(&mut self) {
        // The agent loop checks this at each iteration/stream-chunk boundary.
        self.shutdown.store(true, Ordering::Release);
    }

    fn desc(&self) -> &str {
        "octos (in-process)"
    }
}

impl Drop for InProcessOctos {
    fn drop(&mut self) {
        // Without this, dropping the client mid-turn (stall watchdog, app
        // teardown) leaves the agent turn running detached — burning tokens
        // invisibly for up to max_iterations. The flag aborts it at the next
        // loop/stream checkpoint; the thread then sees the closed command
        // channel and exits, taking the runtime with it.
        self.shutdown.store(true, Ordering::Release);
    }
}

fn send(evt_tx: &Sender<AcpEvent>, event: AcpEvent) {
    let _ = evt_tx.send(event);
    SignalToUI::set_ui_signal();
}

/// The agent thread: build once, then serve prompt turns until the client
/// (and thus the command channel) is dropped.
fn agent_thread(
    workspace: PathBuf,
    cmd_rx: Receiver<Cmd>,
    evt_tx: Sender<AcpEvent>,
    shutdown: Arc<AtomicBool>,
) {
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        // Deep agent futures; octos's own entrypoints use an 8MB stack.
        .thread_stack_size(8 * 1024 * 1024)
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            send(&evt_tx, AcpEvent::ProcessGone(format!("tokio runtime: {e}")));
            return;
        }
    };

    let agent = match rt.block_on(build_agent(&workspace, shutdown.clone())) {
        Ok(agent) => agent,
        Err(e) => {
            send(&evt_tx, AcpEvent::ProcessGone(e));
            return;
        }
    };

    send(&evt_tx, AcpEvent::SessionReady);

    // Conversation history across turns (repair prompts rely on it), kept by
    // octos's exact append rules (see run_turn).
    let mut history: Vec<octos_core::Message> = Vec::new();

    // Blocking command loop OUTSIDE the runtime: recv() parks this thread;
    // each turn runs to completion on the runtime.
    while let Ok(Cmd::Prompt(text)) = cmd_rx.recv() {
        rt.block_on(run_turn(&agent, &shutdown, &evt_tx, &mut history, &text));
    }
}

/// Mirrors `octos acp`'s ConfigAgentFactory::build, minimally.
async fn build_agent(
    workspace: &Path,
    shutdown: Arc<AtomicBool>,
) -> Result<Arc<octos_agent::Agent>, String> {
    let (llm, _model) = build_provider()?;

    let cwd = std::fs::canonicalize(workspace).unwrap_or_else(|_| workspace.to_path_buf());
    // Tools only when generation is allowed to research (`agent-tools`);
    // otherwise the agent is text-only — matching what the prompt promises,
    // instead of merely asking the model not to use a full builtin toolset.
    #[cfg(feature = "agent-tools")]
    let tools = octos_agent::ToolRegistry::with_builtins_and_sandbox(
        &cwd,
        octos_agent::create_sandbox(&octos_agent::SandboxConfig::default()),
    );
    #[cfg(not(feature = "agent-tools"))]
    let tools = {
        let _ = &cwd;
        octos_agent::ToolRegistry::new()
    };

    // Degraded open: if anything holds the redb lock the store becomes an
    // in-memory no-op — fine, since save_episodes is off anyway. Any OTHER
    // error (corrupt/truncated episodes.redb from a crash mid-init, bad
    // permissions) would otherwise brick every future generation — the dir is
    // launcher-private scratch, so wipe it and retry once before giving up.
    let data_dir = octos_data_dir();
    let memory = match octos_memory::EpisodeStore::open_or_degraded(&data_dir).await {
        Ok(store) => Arc::new(store),
        Err(_) => {
            let _ = std::fs::remove_dir_all(&data_dir);
            Arc::new(
                octos_memory::EpisodeStore::open_or_degraded(&data_dir)
                    .await
                    .map_err(|e| format!("episode store: {e}"))?,
            )
        }
    };

    let agent = octos_agent::Agent::new(
        octos_core::AgentId::new("host_launcher"),
        llm,
        tools,
        memory,
    )
    .with_config(octos_agent::AgentConfig {
        max_iterations: 12,
        save_episodes: false,
        ..Default::default()
    })
    .with_shutdown(shutdown);

    // With agent-skills, the guide lives in the system prompt for the whole
    // session (the in-process analogue of the .octos/AGENTS.md bootstrap
    // file), and per-turn prompts go slim.
    #[cfg(feature = "agent-skills")]
    {
        agent.append_system_prompt(crate::generate::SPLASH_GUIDE);
        crate::generate::skills::mark_deployed();
    }

    Ok(Arc::new(agent))
}

/// Minimal ~/.octos/config.json: the fields the provider build needs.
#[derive(serde::Deserialize, Default)]
struct OctosConfig {
    provider: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
    api_key_env: Option<String>,
    api_type: Option<String>,
    #[serde(default)]
    env_vars: HashMap<String, String>,
}

/// Config file resolution, matching octos: $OCTOS_CONFIG_DIR, XDG config dir,
/// then legacy ~/.octos.
fn load_config() -> Option<OctosConfig> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(dir) = std::env::var_os("OCTOS_CONFIG_DIR") {
        candidates.push(PathBuf::from(dir).join("config.json"));
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        candidates.push(home.join(".config").join("octos").join("config.json"));
        candidates.push(home.join(".octos").join("config.json"));
    }
    for path in candidates {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(config) = serde_json::from_str::<OctosConfig>(&text) {
                return Some(config);
            }
        }
    }
    None
}

/// Resolves an API key: the named env var, read from config env_vars (literal
/// values only) else the process environment. Not replicated from octos:
/// OAuth auth-store tokens and `keychain:` markers — use the external
/// `octos acp` for those setups.
fn resolve_key(config: &OctosConfig, names: &[String]) -> Option<String> {
    for name in names {
        let found = config
            .env_vars
            .get(name)
            .filter(|v| !v.is_empty() && !v.starts_with("keychain:"))
            .cloned()
            .or_else(|| std::env::var(name).ok().filter(|v| !v.is_empty()));
        if found.is_some() {
            return found;
        }
    }
    None
}

/// config.json → a retry-wrapped provider. Registry-driven (the same factory
/// `octos chat`/`octos acp` use), with octos's two escape hatches replicated:
/// `provider: "custom"` (endpoint fully described by config, no registry
/// entry) and an `api_type` override forcing the wire protocol.
fn build_provider() -> Result<(Arc<dyn octos_llm::LlmProvider>, String), String> {
    const NO_PROVIDER: &str =
        "no LLM provider configured. Run `octos init` or set provider in config.json";

    let config = load_config().unwrap_or_default();
    let provider_name = config
        .provider
        .clone()
        .or_else(|| {
            config
                .model
                .as_deref()
                .and_then(octos_llm::registry::detect_provider)
                .map(str::to_string)
        })
        // Zero-config path: no octos config at all, but a well-known provider
        // key sits in the environment — infer the provider from it.
        .or_else(|| crate::generate::provider_from_env().map(str::to_string))
        .ok_or(NO_PROVIDER)?;

    let is_custom = provider_name == "custom";
    let entry = if is_custom {
        None
    } else {
        Some(
            octos_llm::registry::lookup(&provider_name)
                .ok_or_else(|| format!("unknown LLM provider `{provider_name}` in config.json"))?,
        )
    };

    // Key-name chain: the configured name, else the registry's (plus its
    // sibling aliases, e.g. MOONSHOT_API_KEY/KIMI_API_KEY), else a
    // {PROVIDER}_API_KEY guess.
    let mut key_names: Vec<String> = Vec::new();
    if let Some(name) = &config.api_key_env {
        key_names.push(name.clone());
    } else if let Some(entry) = entry {
        key_names.extend(entry.api_key_env.map(str::to_string));
        key_names.extend(entry.key_env_aliases.iter().map(|s| s.to_string()));
    }
    if key_names.is_empty() {
        key_names.push(format!(
            "{}_API_KEY",
            provider_name.to_uppercase().replace('-', "_")
        ));
    }
    let api_key = resolve_key(&config, &key_names);

    let model = config
        .model
        .clone()
        .or_else(|| entry.and_then(|e| e.default_model.map(str::to_string)));

    // An explicit api_type (or provider "custom") bypasses the registry
    // factory and picks the wire protocol directly — silently ignoring it
    // would talk the wrong protocol to anthropic-compatible proxies.
    let provider: Arc<dyn octos_llm::LlmProvider> = if is_custom || config.api_type.is_some() {
        let model = model.clone().ok_or("config.json needs a model for this provider setup")?;
        let key = api_key.clone().unwrap_or_default();
        match config.api_type.as_deref().unwrap_or("openai") {
            "anthropic" => {
                let mut p = octos_llm::anthropic::AnthropicProvider::new(&key, &model);
                if let Some(url) = &config.base_url {
                    p = p.with_base_url(url);
                }
                Arc::new(p)
            }
            "responses" => {
                let mut p = octos_llm::openai_responses::OpenAIResponsesProvider::new(&key, &model);
                if let Some(url) = &config.base_url {
                    p = p.with_base_url(url);
                }
                Arc::new(p)
            }
            _ => {
                let mut p = octos_llm::openai::OpenAIProvider::new(&key, &model);
                if let Some(url) = &config.base_url {
                    p = p.with_base_url(url);
                }
                Arc::new(p)
            }
        }
    } else {
        let entry = entry.expect("non-custom provider has a registry entry");
        if api_key.is_none() && entry.requires_api_key {
            return Err(NO_PROVIDER.to_string());
        }
        (entry.create)(octos_llm::registry::CreateParams {
            api_key,
            model: model.clone(),
            base_url: config.base_url.clone(),
            model_hints: None,
            llm_timeout_secs: None,
            llm_connect_timeout_secs: None,
        })
        .map_err(|e| format!("LLM provider: {e}"))?
    };

    let model = provider.model_id().to_string();
    Ok((Arc::new(octos_llm::RetryProvider::new(provider)), model))
}

/// Per-turn reporter: octos's own StreamChunk/Response dedupe, reduced to the
/// pipeline's event vocabulary.
struct Reporter {
    evt_tx: Sender<AcpEvent>,
    streamed: AtomicBool,
}

impl octos_agent::ProgressReporter for Reporter {
    fn report(&self, event: octos_agent::ProgressEvent) {
        use octos_agent::ProgressEvent as E;
        match event {
            E::StreamChunk { text, .. } => {
                self.streamed.store(true, Ordering::Release);
                send(&self.evt_tx, AcpEvent::Chunk(text));
            }
            // The loop emits streaming deltas AND a final full Response with
            // the same text; forward the Response only when nothing streamed
            // (non-streaming providers).
            E::Response { content, .. } => {
                if !self.streamed.load(Ordering::Acquire) {
                    send(&self.evt_tx, AcpEvent::Chunk(content));
                }
            }
            E::ToolStarted { name, .. } => {
                send(&self.evt_tx, AcpEvent::ToolCall(name));
            }
            _ => {}
        }
    }
}

/// One prompt turn, following `octos acp`'s run_prompt_turn to the letter:
/// stale-cancel reset before the turn, cancelled-Err mapped to a cancel (not
/// an error), and the two-guard history append.
async fn run_turn(
    agent: &Arc<octos_agent::Agent>,
    shutdown: &Arc<AtomicBool>,
    evt_tx: &Sender<AcpEvent>,
    history: &mut Vec<octos_core::Message>,
    text: &str,
) {
    // NOTE: the stale-cancel reset happens in send_prompt (UI side), BEFORE
    // the command is queued — so a cancel/drop arriving while the turn waits
    // in the queue is never clobbered here.
    agent.set_reporter(Arc::new(Reporter {
        evt_tx: evt_tx.clone(),
        streamed: AtomicBool::new(false),
    }));

    let snapshot = history.clone();
    let outcome = agent.process_message(text, &snapshot, vec![]).await;
    let cancelled = shutdown.load(Ordering::Acquire);

    match outcome {
        Ok(resp) => {
            let assistant_reply = resp.content.clone();
            history.extend(resp.messages);
            let already = matches!(
                history.last(),
                Some(last) if last.role == octos_core::MessageRole::Assistant
                    && last.content == assistant_reply
            );
            if !cancelled && !assistant_reply.is_empty() && !already {
                history.push(octos_core::Message {
                    role: octos_core::MessageRole::Assistant,
                    content: assistant_reply.clone(),
                    media: vec![],
                    tool_calls: None,
                    tool_call_id: None,
                    reasoning_content: None,
                    client_message_id: None,
                    thread_id: None,
                    timestamp: chrono::Utc::now(),
                });
            }
            let stop_reason = if cancelled { "cancelled" } else { "end_turn" };
            send(
                evt_tx,
                AcpEvent::TurnDone {
                    stop_reason: stop_reason.to_string(),
                    text: if cancelled { String::new() } else { assistant_reply },
                },
            );
        }
        Err(e) => {
            if cancelled {
                send(
                    evt_tx,
                    AcpEvent::TurnDone { stop_reason: "cancelled".to_string(), text: String::new() },
                );
            } else {
                send(evt_tx, AcpEvent::Error(format!("agent turn failed: {e}")));
            }
        }
    }
}
