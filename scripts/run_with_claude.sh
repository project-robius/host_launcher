#!/bin/zsh
# One-line launch of host_launcher with AI app generation backed by your
# Claude Pro/Max subscription (via Claude Code's official ACP adapter).
#
#   ./scripts/run_with_claude.sh          # sets everything up, then runs
#   ./scripts/run_with_claude.sh --check  # just verify the setup, don't run
#
# What it does:
#   1. Ensures the claude-code-acp adapter is installed (via npm).
#   2. Warns about login / API-key pitfalls that would surprise you later.
#   3. Runs the launcher with the bar pointed at Claude Code.
#
# No octos, no API key, no config files — generation is billed to your
# existing `claude` login like any Claude Code session.
set -e
cd "$(dirname "$0")/.."

say()  { print -P "%F{cyan}[claude-setup]%f $1"; }
fail() { print -P "%F{red}[claude-setup]%f $1"; exit 1; }

# 1. Node/npm (needed for the adapter). We don't install node ourselves —
#    that's a bigger system change than a helper script should make.
command -v npm >/dev/null 2>&1 || fail "npm not found — install node first (e.g. \`brew install node\`), then re-run."

# 2. The Claude Code CLI itself.
command -v claude >/dev/null 2>&1 || fail "the \`claude\` CLI isn't installed — see https://claude.com/claude-code, then re-run."

# 3. The ACP adapter (auto-installed; it's a small npm package).
if ! command -v claude-code-acp >/dev/null 2>&1; then
  say "installing @zed-industries/claude-code-acp…"
  npm install -g @zed-industries/claude-code-acp
fi
say "adapter: $(command -v claude-code-acp)"

# 4. Pitfall warnings.
if [[ -n "$ANTHROPIC_API_KEY" ]]; then
  say "note: ANTHROPIC_API_KEY is exported — unsetting it FOR THIS RUN so"
  say "      generation bills your subscription, not the API."
fi
say "if generation fails with an auth error, run \`claude\` once and /login."

# 4b. Model / effort / thinking. These are Claude Code's own env knobs; the
#     create bar reads them at startup to seed its option row, and whatever you
#     pick there afterwards wins (and is remembered).
[[ -n "$ANTHROPIC_MODEL" ]]           && say "model: $ANTHROPIC_MODEL"
[[ -n "$CLAUDE_CODE_EFFORT_LEVEL" ]]  && say "effort: $CLAUDE_CODE_EFFORT_LEVEL"
[[ -n "$MAX_THINKING_TOKENS" ]]       && say "thinking budget: $MAX_THINKING_TOKENS"
say "tip: ANTHROPIC_MODEL / CLAUDE_CODE_EFFORT_LEVEL (low|medium|high|max) /"
say "     MAX_THINKING_TOKENS preset the bar's options; set them there instead."

if [[ "$1" == "--check" ]]; then
  say "setup looks good — run without --check to launch."
  exit 0
fi

# 5. Launch. CLAUDECODE is dropped by the launcher itself (nested-session
#    guard bypass); the API key is dropped here (subscription billing).
say "launching host_launcher (create bar → Claude Code)…"
HOST_LAUNCHER_AGENT_CMD="claude-code-acp" \
  env -u ANTHROPIC_API_KEY cargo run --release --bin host_launcher "$@"
