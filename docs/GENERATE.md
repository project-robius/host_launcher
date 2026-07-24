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

2. Configure an LLM provider (interactive; writes `~/.octos/config.json`):

   ```bash
   octos init          # or: octos auth
   ```

   Without this the bar reports: *"octos has no LLM provider — run `octos
   init` in a terminal first"*.

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
