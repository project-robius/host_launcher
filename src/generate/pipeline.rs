//! The create-app generation pipeline: prompt → streamed reply → extract the
//! fenced Splash source → validate it against the real parser → repair loop →
//! a ready-to-install `MiniAppManifest`.
//!
//! Modeled on octos-one's card pipeline (single fenced block as the ENTIRE
//! reply, client-side extraction, lint → one-shot repair), but the validator
//! here is the actual Splash parser/evaluator running in a throwaway isolate
//! with a captured-error sink — not a regex lint.

use makepad_widgets::*;

use crate::generate::acp_client::AcpEvent;
use crate::generate::prefs::AgentPrefs;
use crate::generate::AgentTransport;
use crate::mini_apps::registry::{MiniAppId, MiniAppManifest};

/// How many repair turns a generation may take after the first attempt.
const MAX_REPAIRS: u32 = 2;

/// Symbol glyphs the launcher's fonts are known to lack (they render as
/// tofu boxes). Generated apps must use emoji / words / × instead; seen in
/// the wild when models reach for ✕-style icons.
const TOFU_GLYPHS: &[char] = &['✕', '✗', '✘', '⤡', '⤢', '➜', '↻', '⟳'];

/// A generation with NO agent events for this long is declared stalled. LLM
/// turns legitimately take a while, so this is generous — it only catches an
/// agent that is alive but silent (hung provider, dead network) which would
/// otherwise leave the bar busy until the user hits Stop.
const STALL_SECS: u64 = 240;

/// Where the generation pipeline currently is; drives the create bar's status.
#[derive(Debug, Clone, PartialEq)]
pub enum GenPhase {
    /// Agent spawned, handshake in flight.
    Connecting,
    /// The prompt (or a repair prompt) is out; the reply is streaming in.
    /// `attempt` is 0 for the first try, 1.. for repairs.
    Generating { attempt: u32 },
    /// Nothing in flight (after success/failure the pipeline is dropped).
    Done,
}

/// The outcome the app layer acts on after feeding an event batch through.
pub enum GenOutcome {
    /// Still working; the status line may have changed.
    Working,
    /// Success. `refine_of` is set when this UPDATES an existing app (the
    /// manifest keeps that app's id) rather than installing a new one.
    Ready {
        manifest: Box<MiniAppManifest>,
        refine_of: Option<MiniAppId>,
    },
    /// Failure, with a short human-readable reason.
    Failed(String),
}

/// Whether this generation creates a new app or modifies an existing one.
enum GenMode {
    Create,
    /// Refine `base`: the prompt carries its current source, and the result
    /// keeps its id (name/icon/tint may change via the header).
    Refine { base: MiniAppManifest },
}

/// One in-flight "create app" (or "refine app") request.
pub struct Generation {
    client: Box<dyn AgentTransport>,
    /// The user's request, verbatim.
    request: String,
    mode: GenMode,
    phase: GenPhase,
    /// Repair turns used so far.
    repairs: u32,
    /// Live status line for the bar.
    status: String,
    /// Ids already taken (so a fresh id can be made unique against the registry).
    taken_ids: Vec<MiniAppId>,
    /// When the last agent event arrived, for the stall watchdog.
    last_event: std::time::Instant,
    /// Whether prompts may omit the inline guide (agent-skills active AND this
    /// generation's backend is one that carries the persistent copy — a
    /// foreign agent via HOST_LAUNCHER_AGENT_CMD may ignore AGENTS.md, so the
    /// env override always inlines).
    slim_prompts: bool,
    /// Human-readable activity trail for the drop-down panel: phase changes,
    /// tool calls, validation errors. Capped; newest last.
    activity: Vec<String>,
    /// The current turn's streamed reply text (the code being written),
    /// accumulated for the panel's live tail. Cleared per turn.
    stream: String,
}

impl Generation {
    /// Starts the agent backend and kicks off a generation for `request`.
    /// `taken_ids` is the current set of installed app ids.
    pub fn start(
        request: String,
        taken_ids: Vec<MiniAppId>,
        prefs: AgentPrefs,
    ) -> Result<Self, String> {
        Self::start_with_mode(request, taken_ids, GenMode::Create, prefs)
    }

    /// Starts a refine of an existing app: same machinery, but the prompt
    /// includes the app's current source and the result keeps its id.
    pub fn start_refine(
        request: String,
        base: MiniAppManifest,
        prefs: AgentPrefs,
    ) -> Result<Self, String> {
        Self::start_with_mode(request, Vec::new(), GenMode::Refine { base }, prefs)
    }

    fn start_with_mode(
        request: String,
        taken_ids: Vec<MiniAppId>,
        mode: GenMode,
        prefs: AgentPrefs,
    ) -> Result<Self, String> {
        let workspace = agent_workspace_dir();
        // Persist the dialect guide on the agent side so prompts can go slim.
        #[cfg(feature = "agent-skills")]
        crate::generate::skills::deploy_guide(&workspace);
        let client = crate::generate::start_backend(&workspace, &prefs)?;
        // Slim prompts only for backends known to carry the persistent guide:
        // the default octos spawn / the in-process agent. An explicit
        // HOST_LAUNCHER_AGENT_CMD may be any ACP agent, which likely ignores
        // our AGENTS.md — inline the guide for those.
        #[cfg(feature = "agent-skills")]
        let slim_prompts = std::env::var("HOST_LAUNCHER_AGENT_CMD").is_err()
            && crate::generate::skills::guide_is_deployed();
        #[cfg(not(feature = "agent-skills"))]
        let slim_prompts = false;
        Ok(Self {
            client,
            request,
            mode,
            phase: GenPhase::Connecting,
            repairs: 0,
            status: "Contacting agent…".to_string(),
            taken_ids,
            last_event: std::time::Instant::now(),
            slim_prompts,
            activity: vec!["Starting agent…".to_string()],
            stream: String::new(),
        })
    }

    /// The user's request, verbatim — recorded as the version-history note
    /// when a modification lands.
    pub fn request(&self) -> &str {
        &self.request
    }

    /// The run's full activity trail, oldest first — what the console shows.
    pub fn activity(&self) -> &[String] {
        &self.activity
    }

    /// The tail of the current turn's streamed reply — the code being written,
    /// live. Capped to what the panel can usefully show.
    pub fn stream_tail(&self) -> &str {
        // Last ~700 bytes, snapped to a char boundary, so the panel shows the
        // most recent lines without ever holding the whole reply twice.
        let start = self.stream.len().saturating_sub(700);
        let start = (start..self.stream.len())
            .find(|&i| self.stream.is_char_boundary(i))
            .unwrap_or(0);
        &self.stream[start..]
    }

    /// Appends to the run's trail. Nothing is ever dropped: the console shows
    /// the whole history and scrolls, and a pipeline lives only as long as the
    /// generation it's driving, so the trail is bounded by the run itself.
    fn log(&mut self, line: impl Into<String>) {
        self.activity.push(line.into());
    }

    /// The current status line for the create bar.
    pub fn status(&self) -> &str {
        &self.status
    }

    /// Asks the agent to abandon the in-flight turn. The caller drops the
    /// pipeline right after; this just lets the agent stop burning tokens.
    pub fn cancel(&mut self) {
        self.client.cancel();
    }

    /// Whether the agent has gone silent past the stall budget. Checked from a
    /// periodic timer, since a silent agent by definition produces no events
    /// that would otherwise wake the pipeline.
    pub fn is_stalled(&self) -> bool {
        self.phase != GenPhase::Done && self.last_event.elapsed().as_secs() > STALL_SECS
    }

    /// Feeds queued agent events through the state machine. Call on every UI
    /// event (cheap when idle); `cx` is needed to run the validator.
    pub fn advance(&mut self, cx: &mut Cx) -> GenOutcome {
        let events = self.client.drain_events();
        if !events.is_empty() {
            self.last_event = std::time::Instant::now();
        }
        for event in events {
            match event {
                AcpEvent::SessionReady => {
                    let prompt = match &self.mode {
                        GenMode::Create => build_initial_prompt(&self.request, self.slim_prompts),
                        GenMode::Refine { base } => {
                            build_refine_prompt(&self.request, base, self.slim_prompts)
                        }
                    };
                    self.client.send_prompt(&prompt);
                    self.phase = GenPhase::Generating { attempt: 0 };
                    self.status = "Generating your app…".to_string();
                    self.log("Agent connected — writing the app");
                    self.stream.clear();
                }
                AcpEvent::Chunk(text) => {
                    self.stream.push_str(&text);
                    // Streaming progress. Length is a decent proxy for life.
                    if let GenPhase::Generating { attempt } = self.phase {
                        self.status = if attempt == 0 {
                            "Writing the app…".to_string()
                        } else {
                            format!("Fixing the app (try {attempt})…")
                        };
                    }
                }
                AcpEvent::ToolCall(title) => {
                    self.status = format!("Agent: {title}…");
                    self.log(format!("🔧 {title}"));
                }
                AcpEvent::TurnDone { stop_reason, text } => {
                    if stop_reason == "cancelled" {
                        return GenOutcome::Failed("Cancelled".to_string());
                    }
                    if stop_reason == "refusal" {
                        return GenOutcome::Failed("The agent declined that request".to_string());
                    }
                    match self.finish_turn(cx, &text) {
                        TurnVerdict::Installed(manifest) => {
                            self.phase = GenPhase::Done;
                            let refine_of = match &self.mode {
                                GenMode::Create => None,
                                GenMode::Refine { base } => Some(base.id.clone()),
                            };
                            return GenOutcome::Ready { manifest, refine_of };
                        }
                        TurnVerdict::NeedsRepair(errors) => {
                            self.repairs += 1;
                            if self.repairs > MAX_REPAIRS {
                                return GenOutcome::Failed(
                                    "The generated app kept failing to compile".to_string(),
                                );
                            }
                            for e in errors.iter().take(3) {
                                self.log(format!("⚠ {e}"));
                            }
                            let attempt = self.repairs;
                            self.log(format!("Sending errors back (repair {attempt})"));
                            self.client.send_prompt(&build_repair_prompt(&errors));
                            self.phase = GenPhase::Generating { attempt };
                            self.status = format!("Fixing the app (try {attempt})…");
                            self.stream.clear();
                        }
                        TurnVerdict::Malformed(reason) => {
                            // No fenced code at all — one retry with a nudge,
                            // sharing the repair budget so this can't loop.
                            self.repairs += 1;
                            if self.repairs > MAX_REPAIRS {
                                return GenOutcome::Failed(reason);
                            }
                            let attempt = self.repairs;
                            self.log("Reply had no code block — asking again");
                            self.client.send_prompt(&build_nudge_prompt());
                            self.phase = GenPhase::Generating { attempt };
                            self.status = "Asking for the code again…".to_string();
                            self.stream.clear();
                        }
                    }
                }
                AcpEvent::Error(msg) => {
                    return GenOutcome::Failed(short_reason(&msg));
                }
                AcpEvent::ProcessGone(msg) => {
                    return GenOutcome::Failed(process_gone_reason(&msg, self.client.desc()));
                }
            }
        }
        GenOutcome::Working
    }

    /// Extract + validate a finished turn. On success, returns the manifest.
    fn finish_turn(&mut self, cx: &mut Cx, reply: &str) -> TurnVerdict {
        let Some(block) = extract_splash_block(reply) else {
            return TurnVerdict::Malformed(
                "The agent's reply had no ```splash code block".to_string(),
            );
        };
        let (header, source) = parse_header(&block);

        if let Some(banned) = forbidden_construct(&source) {
            return TurnVerdict::NeedsRepair(vec![format!(
                "the script uses `{banned}`, which is not available in this dialect \
                 — remove it and use only constructs from the guide"
            )]);
        }
        // Runtime-invisible but user-visible: glyphs the app fonts don't have
        // render as tofu boxes. Caught here (the parser can't) so the repair
        // turn swaps them out.
        if let Some(c) = source.chars().find(|c| TOFU_GLYPHS.contains(c)) {
            return TurnVerdict::NeedsRepair(vec![format!(
                "the app font has no glyph for '{c}' (renders as an empty box) — \
                 use an emoji, a plain word, or × (U+00D7) instead"
            )]);
        }

        self.status = "Checking the app…".to_string();
        self.log("Validating with the Splash parser");
        let errors = validate_splash(cx, &source);
        if !errors.is_empty() {
            return TurnVerdict::NeedsRepair(errors);
        }
        self.log("Compiles clean — installing");

        // A refine keeps the app's identity; the header may restyle it. A
        // create mints everything fresh.
        let manifest = match &self.mode {
            GenMode::Create => {
                let name = header.name.unwrap_or_else(|| default_name(&self.request));
                let id = unique_id(&name, &self.taken_ids);
                MiniAppManifest {
                    id,
                    name,
                    icon: header.icon.unwrap_or_else(|| "✨".to_string()),
                    tint: header.tint.unwrap_or(0x7c6cf0),
                    source,
                    allow_net: false,
                    builtin: false,
                    widget: None,
                    shortcuts: Vec::new(),
                }
            }
            GenMode::Refine { base } => MiniAppManifest {
                id: base.id.clone(),
                name: header.name.unwrap_or_else(|| base.name.clone()),
                icon: header.icon.unwrap_or_else(|| base.icon.clone()),
                tint: header.tint.unwrap_or(base.tint),
                source,
                allow_net: base.allow_net,
                // Keep the flag: a modified BUILT-IN stays built-in (its
                // override just shadows the stock app). Dropping it here would
                // make it uninstallable-then-resurrectable — and would strip
                // the protection the menu relies on.
                builtin: base.builtin,
                // The widget is a SEPARATE script; refining the app's main
                // script doesn't invalidate it (and dropping it would break
                // any placed instances). Keep it.
                widget: base.widget.clone(),
                shortcuts: base.shortcuts.clone(),
            },
        };
        TurnVerdict::Installed(Box::new(manifest))
    }
}

enum TurnVerdict {
    Installed(Box<MiniAppManifest>),
    NeedsRepair(Vec<String>),
    Malformed(String),
}

/// `~/.host_launcher/agent_workspace`, the harmless directory the agent's
/// tools are rooted at (octos insists on a cwd for its sandbox).
fn agent_workspace_dir() -> std::path::PathBuf {
    let base = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join(".host_launcher").join("agent_workspace")
}

// ---------------------------------------------------------------------------
// Prompts
// ---------------------------------------------------------------------------

use crate::generate::SPLASH_GUIDE;

/// With `agent-tools`, the agent may research before answering (web search /
/// fetch — octos's own tools), baking what it finds into the app as
/// constants. Without it, generation is a single pure text turn.
#[cfg(feature = "agent-tools")]
const TOOL_POLICY: &str = "You MAY use your tools first (e.g. web search/fetch) to look up real \
     data the app should carry — exchange rates, schedules, trivia — and bake \
     the results into the script as plain constants (the app itself has no \
     network access). Your FINAL message must still be exactly the one fenced \
     block, nothing else.";
#[cfg(not(feature = "agent-tools"))]
const TOOL_POLICY: &str = "Do not use tools; reply directly.";

/// The reply contract shared by create, refine, and repair turns.
fn reply_contract() -> String {
    format!(
        "## Reply format (MANDATORY)\n\
         \n\
         Reply with EXACTLY ONE fenced code block and nothing else — no prose \
         before or after, no extra fences. {TOOL_POLICY}\n\
         \n\
         ```splash\n\
         // name: <Short App Name, max 18 chars>\n\
         // icon: <one emoji>\n\
         // tint: <hex color like #4A90D9 that suits the app>\n\
         <the complete script>\n\
         ```"
    )
}

/// The dialect guide, unless this generation's backend carries a persistent
/// copy (`agent-skills` deploys one — see `super::skills`), in which case a
/// one-line pointer replaces the ~6KB text.
fn guide_section(slim: bool) -> String {
    if slim {
        return "Follow the Splash dialect guide in your workspace's AGENTS.md \
                (also in your system prompt) EXACTLY — only constructs shown \
                there exist."
            .to_string();
    }
    SPLASH_GUIDE.to_string()
}

fn build_initial_prompt(request: &str, slim: bool) -> String {
    format!(
        "You are the app generator for a phone launcher. Build a small, polished, \
         self-contained mini-app in the Makepad Splash dialect described below.\n\
         \n\
         {}\n\
         \n\
         {}\n\
         \n\
         User request: {request}",
        guide_section(slim),
        reply_contract(),
    )
}

fn build_refine_prompt(request: &str, base: &MiniAppManifest, slim: bool) -> String {
    format!(
        "You are the app generator for a phone launcher. MODIFY an existing \
         mini-app written in the Makepad Splash dialect described below. Keep \
         everything the user didn't ask to change.\n\
         \n\
         {}\n\
         \n\
         ## The app's current source ({})\n\
         \n\
         ````splash\n{}\n````\n\
         \n\
         {}\n\
         \n\
         User's change request: {request}",
        guide_section(slim),
        base.name,
        base.source,
        reply_contract(),
    )
}

fn build_repair_prompt(errors: &[String]) -> String {
    let list = errors
        .iter()
        .map(|e| format!("- {e}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Your script failed to compile. Errors:\n{list}\n\n\
         Re-read the guide's rules and reply again with the corrected COMPLETE \
         script as EXACTLY ONE ```splash fenced block (same // name/icon/tint \
         header), nothing else."
    )
}

fn build_nudge_prompt() -> String {
    "Your reply did not contain a ```splash fenced code block. Reply with \
     EXACTLY ONE ```splash fenced block containing the complete script \
     (starting with the // name / // icon / // tint header lines), and \
     nothing else."
        .to_string()
}

// ---------------------------------------------------------------------------
// Extraction
// ---------------------------------------------------------------------------

/// Pulls the body of the ```splash fence out of the reply. The agent is asked
/// for exactly one, but replies sometimes grow prose that *mentions* the fence
/// (before or after the real one) — so every tagged fence is scanned and the
/// longest body wins, which is robust against both a quoted-format preamble
/// and a trailing "as requested, one ```splash block" remark. Falls back to a
/// bare ``` fence if no tagged one exists. CRLF endings are normalized.
fn extract_splash_block(reply: &str) -> Option<String> {
    fn body_after(reply: &str, open_idx: usize) -> Option<&str> {
        let after = &reply[open_idx..];
        let body_start = after.find('\n')? + 1;
        let body = &after[body_start..];
        let end = body.find("```").unwrap_or(body.len());
        Some(body[..end].trim_end())
    }

    let mut best: Option<&str> = None;
    let mut from = 0;
    while let Some(rel) = reply[from..].find("```splash") {
        let idx = from + rel;
        if let Some(body) = body_after(reply, idx) {
            if best.is_none_or(|b| body.len() > b.len()) {
                best = Some(body);
            }
        }
        from = idx + "```splash".len();
    }
    let body = match best {
        Some(b) => b,
        // Fallback: a single untagged fence.
        None => body_after(reply, reply.find("```")?)?,
    };
    if body.trim().is_empty() {
        return None;
    }
    Some(body.replace("\r\n", "\n"))
}

/// The `// name:` / `// icon:` / `// tint:` header comments, all optional.
#[derive(Default)]
struct Header {
    name: Option<String>,
    icon: Option<String>,
    tint: Option<u32>,
}

/// Parses the header comments off the top of the block. They are left in the
/// source (this dialect allows `//` comments), so the installed app keeps its
/// provenance visible.
fn parse_header(block: &str) -> (Header, String) {
    let mut header = Header::default();
    for line in block.lines().take(6) {
        let Some(rest) = line.trim().strip_prefix("//") else {
            continue;
        };
        let rest = rest.trim();
        if let Some(v) = rest.strip_prefix("name:") {
            let v = v.trim();
            if !v.is_empty() {
                header.name = Some(v.chars().take(18).collect::<String>().trim().to_string());
            }
        } else if let Some(v) = rest.strip_prefix("icon:") {
            // Take the first whitespace-separated token, capped — this keeps
            // multi-codepoint emoji (ZWJ sequences like 👨‍👩‍👧, flags, keycaps)
            // intact instead of truncating them to a broken first scalar.
            if let Some(tok) = v.split_whitespace().next() {
                header.icon = Some(tok.chars().take(12).collect());
            }
        } else if let Some(v) = rest.strip_prefix("tint:") {
            header.tint = parse_hex_color(v.trim());
        }
    }
    (header, block.to_string())
}

/// `#4A90D9`, `0x4A90D9`, or `4A90D9` → 0xRRGGBB.
fn parse_hex_color(v: &str) -> Option<u32> {
    let v = v
        .trim_start_matches('#')
        .trim_start_matches("0x")
        .trim_start_matches('x');
    if v.len() != 6 {
        return None;
    }
    u32::from_str_radix(v, 16).ok()
}

/// Cheap pre-parse ban list: constructs from OTHER dialects (octos-one's
/// `sys.*` helpers, web substrates, raw module imports) that would either
/// parse-fail confusingly or signal the agent drifted off-dialect — caught
/// here with a clearer message than the parser would give. This is a
/// steering aid, NOT the sandbox: actual containment is the isolate itself
/// (no net runtime, and `fs`/`run`/`res` stripped from its namespace).
/// String literals are blanked before scanning so an app whose *label text*
/// mentions e.g. "import " isn't rejected.
fn forbidden_construct(source: &str) -> Option<&'static str> {
    const BANNED: &[&str] = &[
        "sys.", "live_design", "Root{", "Root {", "Window{", "Window {",
        "use mod", "import ", "<script", "<div",
    ];
    let bare = blank_string_literals(source);
    BANNED.iter().copied().find(|b| bare.contains(b))
}

/// Replaces the contents of double-quoted string literals with spaces
/// (honoring `\"` escapes), so bans only match actual code.
fn blank_string_literals(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut in_str = false;
    let mut escaped = false;
    for c in source.chars() {
        if in_str {
            if escaped {
                escaped = false;
                out.push(' ');
            } else if c == '\\' {
                escaped = true;
                out.push(' ');
            } else if c == '"' {
                in_str = false;
                out.push('"');
            } else {
                out.push(' ');
            }
        } else {
            if c == '"' {
                in_str = true;
            }
            out.push(c);
        }
    }
    out
}

/// A short display name derived from the request when the header lacks one.
fn default_name(request: &str) -> String {
    let mut words = request.split_whitespace().collect::<Vec<_>>();
    words.retain(|w| !matches!(w.to_ascii_lowercase().as_str(),
        "a" | "an" | "the" | "app" | "make" | "create" | "build" | "me" | "for" | "with"));
    let mut name = words.into_iter().take(2).collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        name = "Generated".to_string();
    }
    // Title-case each word.
    name.split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(18)
        .collect()
}

/// Kebab-case id from the name, made unique against the installed set.
/// Also used by the install path to re-unique against the LIVE state, since
/// the snapshot taken when the generation started can go stale (apps
/// installed from the store, or a second generation racing this one).
pub(crate) fn unique_id(name: &str, taken: &[MiniAppId]) -> MiniAppId {
    let mut base: String = name
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if base.is_empty() {
        base = "generated".to_string();
    }
    if !taken.iter().any(|t| t == &base) {
        return base;
    }
    for n in 2.. {
        let candidate = format!("{base}-{n}");
        if !taken.iter().any(|t| t == &candidate) {
            return candidate;
        }
    }
    unreachable!()
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Evaluates `source` the exact way the Splash widget will (same prelude
/// prefix, no network) in a throwaway isolate, and returns the formatted
/// errors. Empty == the script parses and its root evaluates. Runtime errors
/// inside handlers can still happen later — this catches the compile/eval
/// class that octos-one's lint+repair loop targets, but with the real parser.
pub fn validate_splash(cx: &mut Cx, source: &str) -> Vec<String> {
    makepad_widgets::splash::validate_splash_body(cx, source, false)
}

// ---------------------------------------------------------------------------
// Error copy
// ---------------------------------------------------------------------------

/// First line, trimmed, capped — status-bar sized.
fn short_reason(msg: &str) -> String {
    let line = msg.lines().next().unwrap_or("agent error").trim();
    let mut s: String = line.chars().take(120).collect();
    if s.is_empty() {
        s = "agent error".to_string();
    }
    s
}

/// Friendlier copy for the two setup failure modes everyone will hit first:
/// the binary missing, and the provider not configured.
fn process_gone_reason(msg: &str, cmd: &str) -> String {
    if msg.contains("no LLM provider configured") {
        "No LLM provider — export an API key (e.g. ANTHROPIC_API_KEY) or run `octos init`"
            .to_string()
    } else if msg.contains("couldn't start") {
        format!("`{cmd}` isn't runnable — install octos or set HOST_LAUNCHER_AGENT_CMD")
    } else {
        short_reason(msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_tagged_fence() {
        let reply = "sure!\n```splash\n// name: X\nView{}\n```\nthanks";
        assert_eq!(extract_splash_block(reply).unwrap(), "// name: X\nView{}");
    }

    #[test]
    fn extracts_longest_tagged_fence() {
        // Trailing prose that merely MENTIONS the fence must not beat the
        // real (longer) block, wherever each appears.
        let reply = "```splash\nlet a = 1\nView{}\n```\nas requested, one ```splash block";
        assert_eq!(extract_splash_block(reply).unwrap(), "let a = 1\nView{}");
        let reply2 = "a ```splash\nstub\n``` first, then\n```splash\nlet real = 1\nView{}\n```";
        assert_eq!(extract_splash_block(reply2).unwrap(), "let real = 1\nView{}");
    }

    #[test]
    fn extraction_normalizes_crlf() {
        let reply = "```splash\r\nView{}\r\n```";
        assert_eq!(extract_splash_block(reply).unwrap(), "View{}");
    }

    #[test]
    fn extracts_untagged_fallback_and_rejects_none() {
        assert_eq!(extract_splash_block("```\nBody\n```").unwrap(), "Body");
        assert!(extract_splash_block("no code here").is_none());
    }

    #[test]
    fn unterminated_fence_still_extracts() {
        let reply = "```splash\nView{}";
        assert_eq!(extract_splash_block(reply).unwrap(), "View{}");
    }

    #[test]
    fn parses_header_and_leaves_it_in_source() {
        let block = "// name: Tip Calc\n// icon: 💰\n// tint: #4A90D9\nView{}";
        let (h, source) = parse_header(block);
        assert_eq!(h.name.as_deref(), Some("Tip Calc"));
        assert_eq!(h.icon.as_deref(), Some("💰"));
        assert_eq!(h.tint, Some(0x4A90D9));
        assert_eq!(source, block);
    }

    #[test]
    fn header_variants_parse() {
        assert_eq!(parse_hex_color("#aabbcc"), Some(0xaabbcc));
        assert_eq!(parse_hex_color("0xAABBCC"), Some(0xAABBCC));
        assert_eq!(parse_hex_color("nope"), None);
    }

    #[test]
    fn bans_foreign_dialects() {
        assert_eq!(forbidden_construct("View{ sys.weather() }"), Some("sys."));
        assert_eq!(forbidden_construct("let x = 1\nView{}"), None);
    }

    #[test]
    fn ban_scan_ignores_string_literals() {
        // A label whose TEXT mentions a banned token is a legit app.
        assert_eq!(
            forbidden_construct("View{ Label{text: \"how to import  sys. stuff\"} }"),
            None
        );
        // ...but the same token as code still trips.
        assert_eq!(
            forbidden_construct("Label{text: \"hi\"}\nsys.weather()"),
            Some("sys.")
        );
        // Escaped quotes don't desync the scanner.
        assert_eq!(
            forbidden_construct("Label{text: \"a \\\" quote\"}\nimport x"),
            Some("import ")
        );
    }

    #[test]
    fn icon_header_keeps_multi_codepoint_emoji() {
        let (h, _) = parse_header("// icon: 👨‍👩‍👧\nView{}");
        assert_eq!(h.icon.as_deref(), Some("👨‍👩‍👧"));
    }

    #[test]
    fn ids_unique_against_taken() {
        let taken = vec!["tip-calc".to_string(), "tip-calc-2".to_string()];
        assert_eq!(unique_id("Tip Calc", &taken), "tip-calc-3");
        assert_eq!(unique_id("Fresh", &taken), "fresh");
        assert_eq!(unique_id("!!!", &[]), "generated");
    }

    #[test]
    fn default_names_from_request() {
        assert_eq!(default_name("make me a pomodoro timer app"), "Pomodoro Timer");
        assert_eq!(default_name(""), "Generated");
    }
}
