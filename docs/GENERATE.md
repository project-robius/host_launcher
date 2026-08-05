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
   cargo install --git https://github.com/octos-org/octos octos-cli
   ```

   (The launcher tells you this itself: with octos missing, the Providers
   page leads with a banner naming it and a **Copy install command** button,
   rather than letting a run die on a spawn errno.)

2. Give it an LLM provider — pick whichever is least effort for you:

   - **The AI Providers page** (see below): tap **＋ Providers** under the
     create bar, or just click the prompt — with nothing set up, that's what
     it offers. Pick the provider, paste its key, Save.
   - **Already exported in your shell?** Nothing to do — the launcher infers
     the provider from whichever of `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`,
     `GEMINI_API_KEY`, `OPENROUTER_API_KEY`, `DEEPSEEK_API_KEY`,
     `GROQ_API_KEY`, `MOONSHOT_API_KEY` is set (that order) and passes
     `--provider`; the key is read from the environment by octos itself.
   - **`octos auth login -p anthropic`** stores a pasted key in octos's auth
     store/keychain — the launcher detects that too.
   - **`octos init`** for everything else: models, custom endpoints,
     fallbacks. An existing config always wins over auto-detection.

   **Claude Pro/Max subscription (no API key)**: your subscription can't
   back *octos* (it has no Anthropic OAuth, and subscription tokens are
   Claude-Code-only) — but Claude Code itself speaks ACP, and the bar is a
   generic ACP client:

   ```bash
   ./scripts/run_with_claude.sh      # installs the adapter if needed, then runs
   ```

   or by hand:

   ```bash
   npm install -g @zed-industries/claude-code-acp
   HOST_LAUNCHER_AGENT_CMD="claude-code-acp" cargo run
   ```

   That generates on your existing `claude` login (run `claude` → `/login`
   once if you never have), billed to the subscription like any Claude Code
   session. Put the export in your shell profile to make it the default.
   Don't export `ANTHROPIC_API_KEY` alongside it, or Claude Code may bill
   the API instead (the script unsets it for the run). For an octos backend
   specifically you'd need an API key from console.anthropic.com (billed
   separately).

   **No API key at all?** Three options:

   - **Fully local (private, free)**: install [Ollama](https://ollama.com)
     and pull a code model — `ollama pull qwen2.5-coder:14b`. A running
     Ollama on the default port is auto-detected as the last-resort
     provider (no key, no account); the launcher picks the best pulled
     model, preferring code-tuned ones. Expect lower dialect accuracy than
     the frontier models — the repair loop earns its keep here.
   - **ChatGPT account**: `octos auth login -p openai` runs octos's OAuth
     flow (browser or `--device-code`) — no API key; the launcher detects
     the stored login automatically.
   - **Free-tier keys**: Gemini and Groq offer no-payment API keys, and
     OpenRouter has `:free` models — those go through the normal paste-a-key
     modal.

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

## Modifying an app

Two ways in, both landing in the same place:

- **From the app**: long-press (or right-click) any app → **✏️ Modify App…**.
  The create bar prefills with `✏️ Weather: ` and takes focus, so you just
  type the change and hit return.
- **Just ask**: type it into the bar and the launcher works out that you mean
  an app you already have — "make the weather app show animations", "add a
  reset button to the pomodoro". Naming an installed app *and* phrasing it
  like an edit is what triggers it; anything that sounds like a new app
  ("create a weather app") still creates one. See `src/generate/intent.rs`.

Either way the prompt carries the app's current source and the result replaces
it **in place**: same id, same home placements, name/icon/tint may update, the
app restarts on next open, and the previous version is archived first.

Built-ins can be modified too — the result is saved as a user override that
shadows the stock app. It stays non-uninstallable; version history is how you
get the original back.

## App Info & version history

Long-press an app → **App Info & History…** opens its settings page: what kind
of app it is, where it's placed, whether it may use the network, how much it
has stored (with **Clear**), its code size, and the destructive actions
(Force Stop, Uninstall). The long-press menu itself stays short — it only
carries what you do *to the home screen*.

Version history lives on that page. Every modification (and every restore)
archives what it replaced, so edits are undoable: snapshots are listed
newest-first with the date and the request that superseded each one, and
**Restore** puts one back (snapshotting the current state first, so the restore
is itself undoable).

On disk they're plain files beside the app, timestamped in local time:

```
apps/<id>/versions/20260727-105412.splash    the source as it was
apps/<id>/versions/20260727-105412.json      when, why, and its name/icon/tint
```

The newest 20 are kept per app; older ones are pruned.

## Reading an app's source

**App code → View** on that page opens the Splash source in a popup over App
Info: makepad's `CodeView` (read-only, syntax-highlighted, no gutter), scrolling
in both directions because generated lines run long and word wrap is off. It
reads the source out of the *registry*, not off disk — a built-in has no file
under `apps/`, and a modified app's current source is what's loaded, not what's
archived. Its × goes back to App Info rather than to the home screen, and so
does Escape/Back: the viewer is a layer on top, not a replacement.

## Sharing apps: export & import

A generated app is a Splash script plus a scrap of metadata, so it travels as
one file. **Export** (App Info) writes a `.splashapp` bundle — flat JSON:
format, id, name, icon, tint, source, and the widget script if there is one —
and copies the same text to the clipboard. Both at once on purpose: the file is
for handing over a folder, the clipboard is for pasting into a chat.

```
<data_dir>/exchange/<id>.splashapp
```

**Import App…** (right-click empty home-screen space) is the other direction.
It lists every bundle in that folder — makepad has no native file dialog yet,
so the folder *is* the picker, and **Open Folder** reveals it — plus a paste box
for a bundle that arrived as text. A bare `.splash` script imports too: the
`// name:` / `// icon:` / `// tint:` header the generator writes is all a
manifest needs, so an app the agent printed into a chat installs the same way
as one you exported.

Imported source is a stranger's code, so it goes through the same compile check
a generated app does before it's allowed onto the home screen (the sandbox
contains it at runtime, but a script that doesn't even parse should fail at the
door, with a reason). Ids are re-uniqued against what's installed and
`builtin` is always cleared, so an import can neither overwrite an app you have
nor mint a protected one.

## What the console shows while it works

A reasoning model can think for a minute before it writes a byte, so the console
reports more than "connected": the agent's **plan** as it publishes it (Claude
Code's TodoWrite arrives as ACP `plan` updates, diffed so a republished list
doesn't print twice), a **💭 Thinking…** line with the thinking text streaming
streaming in below, tool calls, then the code itself as it's written. A spinner
sits in the status line, because a status that never changes is
indistinguishable from a hung one. No run clock: a ticking `m:ss` invited you to
watch a number instead of the work, and said nothing about progress.

Thinking is kept out of the reply buffer — the fenced-block extractor must never
see code the model merely mused about mid-thought.

When the run ends the output stays up to be read. **New prompt** (bottom right)
puts the composer back, clears the field and hands the caret straight back —
you pressed it to type the next one. On success **Open** (top right) puts the
composer back too and goes straight into the app that was just built.

Pressing outside the bar **collapses** it to one line rather than dismissing
it: the log, the error and the Retry/Open offers all survive, and the chevron
brings them back. Nothing the user typed is ever discarded by a press outside,
by losing focus, or by Open — only **New prompt** clears the field.

That fold is a **height clamp on the field**, not a re-layout of its text. The
laid-out text is what maps a click to a caret position, and it is only rebuilt
at draw time — so folding by `max_lines` (which is what this used to do) left
the press that re-focused the composer resolving against the folded layout
while the expanded one was on screen, putting the caret and any drag-selection
on the wrong text. Handing focus back also has to go through
`TextInput::take_key_focus` rather than the generic setter: the field usually
never lost focus in the first place, and re-setting focus it already holds
dispatches no event, so the caret's animators would stay switched off. Both
traps are written up in `docs/SPLASH_FINDINGS.md` (#9, #10).

## Retrying a failed generation

When the repair budget runs out — or the agent errors — the bar keeps the
console and puts **Retry** where Stop was. It re-runs the same request without
it being retyped, and where the backend still has an unused rung on its effort
ladder it raises the setting first (the pick persists, and the options row
updates to match) — the button just says **Retry** either way, because a
button that renames itself to describe its own internals is a puzzle, not a
label. Cancels and refusals get no retry button: a second identical run
doesn't fix either. A missing provider gets the Providers page instead.

### Saying what actually went wrong

A failure arrives buried three layers deep, and every layer wants to replace
the useful sentence with a generic one:

```
{"code":-32603,"message":"Internal error",          ← JSON-RPC's own boilerplate
 "data":"prompt turn failed: API error (moonshot-coding@api/k3):
         provider quota exhausted — HTTP 403 -      ← octos's rendering
         {\"error\":{\"message\":\"You've reached your usage limit…   ← the answer
```

So `pipeline::short_reason` ranks candidates by **depth** — the transport wraps
the provider wraps the API, so the further down a string sits the more specific
it is — and treats `"Internal error"` and friends as noise that never wins.
`data` arrives as a *string of JSON*, one parse deeper than the envelope, and
octos caps a provider's response body at 200 chars (`octos-llm/src/error.rs`),
so that inner document is routinely **unterminated**. Nothing in the extraction
may assume it parses; the well-formed path uses `serde_json` and the broken
path hand-scans. `src/generate/pipeline.rs` tests both against a frame copied
verbatim off the wire.

The one-line status strip is the only thing that gets shortened
(`App::headline_of`, on a word boundary). The console log always holds the
whole message — truncating before the log is how the actionable half of an API
error got thrown away.

## AI Providers

Credentials live in one place with one way in. Reaching for the prompt with
nothing configured opens the **AI Providers** page rather than letting you type
a request that could only fail; afterwards it's the **＋ Providers** button in
the create bar's options row (the ✨ opens it too, and so does a
setup-class generation failure).

The page is one list, every provider showing its own state:

| state | button | what it does |
|---|---|---|
| in use | **Replace** | ask for a new key for it |
| set up | **Use** | switch to it |
| not set up | **Add** | ask for its key |

plus **×** to forget a key — offered only for keys this app saved, since an
exported variable belongs to your shell and an `octos auth login` belongs to
octos. The key field is masked with an eye toggle, always reopens masked, and
is only ever shown *attached to a named provider*: a bare "paste a key" box is
what made this guessy before.

**Where the keys go.** All of them into octos's own `config.json`, each under
its provider's variable name in `env_vars`, with `provider` naming the one in
use. octos resolves a key by looking up *that provider's* variable
(`Config::resolve_api_key`) and never exports the map, so keys for the others
are inert — switching provider is a one-field edit and no second key store has
to exist. The launcher writes the file `0600` (octos itself writes with the
default umask). Every write is surgical: settings this app knows nothing about
survive untouched, and there are tests for that.

Two things the page does on your behalf:

- **A pinned model is dropped when you switch.** A model chosen for one
  provider is meaningless to the next, and `octos acp --provider moonshot
  --model claude-haiku-4-5` fails in a way that looks like the launcher is
  broken.
- **An unmistakable key on the wrong row is refused.** `sk-ant-`, `gsk_`,
  `AIza`, `sk-or-` and `sk-kimi-` name their provider exactly, so pasting one
  onto another row is a slip; you get told which row it belongs to instead of
  an auth error later. A plain `sk-…` is genuinely shared by
  OpenAI/Moonshot/DeepSeek and is never second-guessed.

`OCTOS_CONFIG_DIR`, when set, is **authoritative** — it names the config, with
no fallback to `~/.config/octos` or `~/.octos`. That matches how octos treats
an explicit config context, and it is what keeps a test fixture pointed at a
scratch directory from ever resolving to your real config.

## Using a Kimi / Moonshot key

octos ships **two** Kimi providers, and which one your key belongs to decides
everything else (verified against the pinned octos rev,
`crates/octos-llm/src/registry/`):

| octos provider | aliases | endpoint | default model | key env |
|---|---|---|---|---|
| `moonshot` | `kimi` | `https://api.moonshot.ai/v1` | `kimi-k2.5` | `MOONSHOT_API_KEY` (or `KIMI_API_KEY`) |
| `moonshot-coding` | `kimi-coding` | `https://api.kimi.com/coding/v1` | `k3` | `KIMI_CODING_API_KEY` (or `KIMI_API_KEY` / `MOONSHOT_API_KEY`) |

A **Coding Plan** subscription key looks like `sk-kimi-…` and is *rejected* by
the regular Moonshot endpoints — it needs `moonshot-coding`. That prefix is
recognized, so a coding-plan key just goes into the ✨ **AI Providers** page —
pick *Kimi (Coding Plan)*, paste, save — and nothing else is needed.

A platform key from the Moonshot console is a plain `sk-…`, indistinguishable
from an OpenAI key, so nothing can detect that one from the key alone; pick the
provider by name in the Providers page, or write the config yourself:

```json
{
  "version": 1,
  "provider": "moonshot",
  "model": "kimi-k2.5",
  "env_vars": { "MOONSHOT_API_KEY": "sk-…" }
}
```

octos reads, in order: `<cwd>/.octos/config.json`, then
`~/.config/octos/config.json`, then legacy `~/.octos/config.json`
(`crates/octos-cli/src/config.rs`, `load_resolved`). The Providers page writes
the legacy path, so put yours there unless one of the earlier two
already exists — whichever comes first wins, and a stale `provider: "anthropic"`
config will quietly ignore a Kimi key you exported into the environment.

Models for `moonshot`: `kimi-k2.5` (fast), `kimi-k2.6` and `kimi-k3` (strong).
`k3` and `kimi-for-coding-highspeed` belong to `moonshot-coding` — sending
those to `moonshot` routes a coding-plan model at the wrong host.

The bar reads the provider out of octos's config (not just the environment), so
it reports the backend that will actually run — `octos · moonshot-coding` — and
offers only the controls that provider can honour:

- **`moonshot-coding` (k3):** Effort as **Low / High / Max**. octos emits
  `reasoning_effort` for the k3 family and clamps a Medium pick up to `"high"`
  (octos-llm `openai.rs`, `ReasoningStyle::EffortLowHighMax`), so Medium isn't
  offered — two rungs that did the same thing would be a control that lies.
  Thinking is always on for k3 and its `thinking` object is rejected, so there
  is no Thinking row either.
- **`moonshot` (kimi-k2.x):** no Effort row at all. Those models resolve to
  `ReasoningStyle::None` and octos emits nothing, so the knob would be a no-op.
- Neither gets a Model row (that one is Claude-only today), so the provider's
  default model applies unless you pin one in the config.

A saved model is never passed to a backend with no Model control. It used to
be: a model picked while Claude was configured leaked into `octos acp
--provider moonshot --model claude-haiku-4-5`, which fails in a way that looks
like the launcher is broken.

## Cargo features (all off by default)

| Feature | Effect |
|---|---|
| `agent-embedded` | Link the octos agent core **in-process** (no child process — required for iOS, where `exec()` is prohibited). Reads the same `~/.octos/config.json`; providers resolve through octos-llm's registry with retry. `HOST_LAUNCHER_AGENT_CMD` still wins when set, so the offline test agent keeps working. Not replicated from full octos (use the external agent for these): fallback-model routing, OAuth auth-store keys, `keychain:` markers, MCP/plugins. Costs ~300 extra crates and a fatter binary. |
| `agent-persistent-guide` | Deploy the dialect guide **persistently on the agent side** — for `octos acp`, as the `<workspace>/.octos/AGENTS.md` bootstrap file (appended to the system prompt on every `session/new`, uncapped); for the in-process backend, appended directly. Per-turn prompts shrink from ~6KB to one pointer line. Foreign ACP agents that ignore the file still work — the prompt falls back to inlining. |
| `agent-research` | Allow the agent to research with its tools (web search/fetch) before answering, baking found data into the app as constants — "an app with the current F1 calendar" actually looks it up. Tool activity streams as bar status. The generated app itself still runs sandboxed and offline. |

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
invalid-then-repaired one for requests containing "broken", one that never
compiles for "hopeless" — the Retry path — and a refusal for "refuse"). The UI
tests run against it:

```bash
HOST_LAUNCHER_FRESH=1 OCTOS_CONFIG_DIR=$(mktemp -d) \
    HOST_LAUNCHER_AGENT_CMD=$PWD/target/debug/fake_acp \
    cargo test --test ui -- --test-threads=1
```

(`OCTOS_CONFIG_DIR` sandboxes the setup-modal test's config write; that test
fails fast rather than ever touching a real `~/.octos`.)

## The bar: composer, then console

The bar is one glass pill that floats *over* the home screen — it grows down
across your icons and widgets rather than reflowing them (a fixed-height slot
below it reserves the resting position so the grid still starts underneath).

Idle, it's a multi-line composer: the prompt grows with what you type, up to
75% of the screen, and scrolls past that. Return submits **only when a
physical keyboard is attached** — on a phone, Return has to be able to type a
newline — so a Send arrow appears beside the field as soon as it's non-empty
(beside, not under: its own row doubled the bar's height the moment you typed
a character). That's
also how the UI tests submit (`submit_prompt()` in `tests/ui.rs`); a headless
harness reports no physical keyboard.

Once you submit, the same space becomes the agent's console: a status line
with Stop, over the run's log — connection and phase changes, tool calls,
validation errors being sent back for repair — and under it everything the
agent has emitted.

**Everything**, literally: all the thinking and all the code, across every
repair turn, kept for the life of the run and scrollable (wheel or drag). It
used to be a rolling 700-byte window of the *current* turn, which meant the
attempt that failed to compile — the thing worth reading — was gone by the time
the repair finished. Thinking and code get a heading when they alternate, since
they arrive interleaved, and each repair turn gets a marker.

The box goes to full height the moment its content outgrows it: all the way
down to just above the dock, in one jump. Growing a line at a time behind the
output left it permanently one line short, with the text you wanted scrolling
past the bottom edge and reflowing on every chunk. It never shrinks under you
either — only the cap can pull it back, which it must, because the dock's
position isn't known until the dock has drawn. It follows the tail like a
terminal until you scroll or press inside it, then leaves you where you are.

The console is a **virtualized list**, so only the lines on screen are widgets:
a 5,000-line run costs the same to scroll as a 20-line one. It follows the
newest line while you are at the bottom, stops the moment you scroll up, and
picks the tail back up when you scroll down again — `PortalList`'s own
`auto_tail`, not something the launcher reimplements.

The transcript is handed to that list on a ~120ms clock rather than per chunk.
That was once load-bearing (the console was one `Label`, and a Label re-lays
out *all* of its text on every change); now it just avoids redoing O(run) work
— clone, split into lines, diff — for an update nobody perceives at more than a
few a second. The final flush ignores the throttle, or the last stretch, the
part that says how the run turned out, would never be painted.

**Stop asks first.** It throws away a turn's work — the tokens are already
spent — and it sits in the slot that Retry and Open take over moments later, so
a mis-timed tap on a nearly-finished run used to destroy it. It only cancels if
the run is still going: a run can finish while the sheet is up.

When the run finishes the output **stays**, with the result appended as its
last line. A press outside only *collapses* it; **New prompt** is the one thing
that throws it away. (It used to erase itself a few seconds after finishing,
which took the explanation with it.) While the agent works
the ✨ gives up its slot to a triangle that hides the output — it rotates
between pointing right (hidden) and down (showing), and brings the console
back at the size it had; the sticky choice lasts the session. Presses on the bar
never fall through to the icons underneath, and it reverts to the composer
when you dismiss it.

### Agent options

Focus the prompt (or start typing) and a row of controls appears under it:
**Model**, **Effort**, **Thinking**. They're glass segmented controls, and your
picks persist across launches.

Which controls appear depends on the **active backend**, because there's no
cross-provider standard for any of this:

| Backend | Model | Effort | Thinking | How it's delivered |
|---|:---:|:---:|:---:|---|
| Claude Code (`claude-code-acp`) | ✅ | ✅ | ✅ | `ANTHROPIC_MODEL`, `CLAUDE_CODE_EFFORT_LEVEL`, `MAX_THINKING_TOKENS` — env vars the CLI reads itself |
| octos (anthropic) | ✅ | ✅ | — | model via `octos acp --model …`; effort via `gateway.reasoning_effort` |
| octos (openai, gemini, groq, …) | — | ✅ | — | effort via `gateway.reasoning_effort` |
| any other ACP agent | — | — | — | unknown binary, unknown knobs |

Every control maps to a setting the agent itself has. There's deliberately no
invented "thorough mode" knob: a control that only appends encouragement to the
prompt looks like a capability and isn't one.

A knob nobody reads is worse than a missing one — it looks like it works — so
the bar only shows what the live backend can actually honour, and names that
backend under the controls.

Two wrinkles worth knowing:

- **Effort on octos edits octos's config.** `octos acp` has no `--effort` flag
  (that's `octos chat`); it reads `gateway.reasoning_effort` from
  `config.json`, and octos itself treats that as a persistent setting applied
  to every turn. So the control writes that one field — everything else in the
  file is preserved — and octos maps it per provider: `reasoning_effort` for
  OpenAI and Grok, a thinking budget for Gemini, a thinking block for
  Anthropic. Models with no reasoning style ignore it.
- **The effort ladder is probed, not assumed.** `xhigh` exists in the API and
  in current Claude Code, but *not* in the runtime `claude-code-acp@0.16.2`
  bundles (`@anthropic-ai/claude-agent-sdk@0.2.44`, whose ladder is
  `low`/`medium`/`high`/`max`). An unknown level there is **silently dropped**,
  falling back to `high` — so the bar reads the runtime and only offers
  `X-High` when it will actually be honoured. Point `CLAUDE_CODE_EXECUTABLE`
  at a newer `claude` and the level appears by itself.

  That version pin is also why there's no Workflow tool: the adapter is already
  at its latest (0.16.2) but pins an old SDK, so upgrading the adapter doesn't
  help — `CLAUDE_CODE_EXECUTABLE` is the escape hatch until it bumps.

There's no model picker for non-Anthropic providers because we'd have to
invent model ids we can't verify, and a wrong one just errors at generation
time; Ollama already auto-picks the best model you have installed.

Exporting `ANTHROPIC_MODEL` / `CLAUDE_CODE_EFFORT_LEVEL` / `MAX_THINKING_TOKENS`
before launch still works — those seed the controls on a first run, and
whatever you pick in the bar afterwards wins and is remembered
(`agent_prefs.json`).

![the expanded prompt](screenshots/create_prompt.png)
![the agent console](screenshots/agent_console.png)

`HOST_LAUNCHER_DEBUG_STATE=longprompt` boots straight into a tall multi-line
prompt; `=genbusy` into the console; `=genlog` into a console filled past its
cap (for checking the scroll and the ceiling).
