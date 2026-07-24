# The AI "create app" bar

The glass pill at the top of the home screen generates new mini-apps from a
one-line request: type "a pomodoro timer" and hit return, and a few seconds
later the finished app's icon lands on your home screen like any other install.

## How it works

```
create bar ──▶ spawn ACP agent (`octos acp`) ──▶ session/prompt
                                                      │  (dialect guide + request)
   streamed status ◀── session/update chunks ◀────────┘
                                                      ▼
                       extract the ```splash fence ──▶ validate with the REAL
                                                       Splash parser (throwaway
                                                       isolate, captured errors)
                                                      │
                             ok ◀─────────────────────┤ errors
                              │                       ▼
                              │            repair turn (errors fed back,
                              │            same session, max 2 attempts)
                              ▼
                    MiniAppManifest { builtin: false, allow_net: false }
                    → user_apps + registry + home-screen icon (persisted)
```

- **Transport**: the [Agent Client Protocol](https://agentclientprotocol.com)
  over stdio — newline-delimited JSON-RPC 2.0. host_launcher implements a
  minimal std-only client (`src/generate/acp_client.rs`, no async runtime);
  events reach the UI thread via a queue + `SignalToUI`, mirroring the pattern
  octos-one uses.
- **One process per generation**: the agent is spawned on submit and killed on
  completion/cancel. No reconnect state; Stop just kills it.
- **The agent is swappable**: anything that speaks ACP works. octos is the
  default; Claude Code or Gemini ACP adapters should work unmodified.
- **Prompting**: `src/generate/splash_guide.md` teaches THIS repo's Splash
  dialect (exemplar-driven, with hard prohibitions on other dialects' idioms
  like octos-one's `sys.*` helpers). The reply contract is one ```` ```splash ````
  fence starting with `// name:` / `// icon:` / `// tint:` header comments.
- **Validation is the real parser**, not a lint: makepad's
  `validate_splash_body` (widgets/src/splash.rs) dry-runs the source in a
  throwaway isolate with the exact prefix/instruction-limit the Splash widget
  uses and returns the captured errors, which are fed back verbatim in repair
  turns.

## Setup (once)

1. Build/install the octos CLI (the default `octos acp` needs no extra cargo
   features):

   ```bash
   git clone https://github.com/octos-org/octos && cd octos
   cargo install --path crates/octos-cli
   ```

2. Give it an LLM provider — pick whichever is least effort for you:

   - **Paste a key in the app**: tap the bar's ✨ (the modal also opens
     automatically when generation fails for lack of setup), paste an API
     key, Save. The provider is inferred from the key's prefix (`sk-ant-` →
     anthropic, `sk-or-` → openrouter, `gsk_` → groq, `AIza` → gemini,
     `sk-` → openai) and a minimal octos config is written to `~/.octos/`
     (0600; a config the launcher didn't write is never overwritten).
   - **Already exported in your shell?** Nothing to do — the launcher infers
     the provider from whichever of `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`,
     `GEMINI_API_KEY`, `OPENROUTER_API_KEY`, `DEEPSEEK_API_KEY`,
     `GROQ_API_KEY`, `MOONSHOT_API_KEY` is set (that order) and passes
     `--provider`; the key is read from the environment by octos itself.
   - **`octos auth login -p anthropic`** stores a pasted key in octos's auth
     store/keychain — the launcher detects that too.
   - **`octos init`** for everything else: models, custom endpoints,
     fallbacks. An existing config always wins over auto-detection.

   Note on "using your Claude account": octos has no Anthropic OAuth (its
   only account-login flow is OpenAI/ChatGPT device-code) — a Claude.ai
   Pro/Max subscription can't back it. You need an **API key** from
   console.anthropic.com, billed separately from the subscription.

3. Optional overrides via the agent command line:

   ```bash
   # a different provider/model, or a different ACP agent entirely
   export HOST_LAUNCHER_AGENT_CMD="octos acp --provider anthropic --model claude-sonnet-4-5"
   ```

   The default is plain `octos acp`. The agent's tools are rooted at
   `~/.host_launcher/agent_workspace/` (created on demand).

   Note the agent is a normal child process: it inherits the launcher's
   environment (that's how provider API keys reach it) and runs with your
   user's privileges. Only point `HOST_LAUNCHER_AGENT_CMD` at agents you
   trust. The *generated apps*, by contrast, run in a stripped Splash isolate:
   no filesystem, no subprocesses, no resource loader, no network.

## Refining an app

Long-press any non-builtin app (generated, or an installed sample) →
**Refine App…** → the create bar's hint flips to "Change {name}…" — type the
change ("add a reset button", "make it dark red") and hit return. Same
pipeline, but the prompt carries the app's current source and the result
replaces it **in place**: same id, same home placements, name/icon/tint may
update, and the app is force-stopped so the next open runs the new script.

## Cargo features (all off by default)

| Feature | Effect |
|---|---|
| `agent-octos` | Link the octos agent core **in-process** (no child process — required for iOS, where `exec()` is prohibited). Reads the same `~/.octos/config.json`; providers resolve through octos-llm's registry with retry. `HOST_LAUNCHER_AGENT_CMD` still wins when set, so the offline test agent keeps working. Not replicated from full octos (use the external agent for these): fallback-model routing, OAuth auth-store keys, `keychain:` markers, MCP/plugins. Costs ~300 extra crates and a fatter binary. |
| `agent-skills` | Deploy the dialect guide **persistently on the agent side** — for `octos acp`, as the `<workspace>/.octos/AGENTS.md` bootstrap file (appended to the system prompt on every `session/new`, uncapped); for the in-process backend, appended directly. Per-turn prompts shrink from ~6KB to one pointer line. Foreign ACP agents that ignore the file still work — the prompt falls back to inlining. |
| `agent-tools` | Allow the agent to research with its tools (web search/fetch) before answering, baking found data into the app as constants — "an app with the current F1 calendar" actually looks it up. Tool activity streams as bar status. The generated app itself still runs sandboxed and offline. |

## Remote agents (no feature needed)

The transport is stdio, and stdio composes: point the bar at an agent running
anywhere —

```bash
export HOST_LAUNCHER_AGENT_CMD="ssh myserver octos acp"
```

That's the whole thin-client story for a machine that can't (or shouldn't)
run the model locally: the launcher stays unchanged, ssh carries the NDJSON.

## Testing offline

`src/bin/fake_acp.rs` is a deterministic stand-in agent: it speaks just enough
ACP and picks its scenario from the prompt text (a valid app by default, an
invalid-then-repaired one for requests containing "broken", a refusal for
"refuse"). The UI tests run against it:

```bash
HOST_LAUNCHER_FRESH=1 HOST_LAUNCHER_AGENT_CMD=$PWD/target/debug/fake_acp \
    cargo test --test ui -- --test-threads=1
```

`HOST_LAUNCHER_DEBUG_STATE=genbusy` boots the launcher with the bar frozen in
its busy state for screenshots.
