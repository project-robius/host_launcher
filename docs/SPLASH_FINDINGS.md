# Splash Correctness Deep Dive — Findings & Fixes

Everything below was found while building host_launcher. Items marked **[FIXED]**
have been implemented in the local makepad checkout with regression tests; items
marked **[RECOMMEND]** are proposals awaiting a decision.

## 1. Short-circuit `&&`/`||` loses its value in call-argument position — [FIXED]

**Symptom.** `ui.x.set_visible((w >= 190) && (w < 280))` acted as `true` when the
expression was false. Cost hours of "ghost clock face" debugging.

**Empirical signature** (15-case probe matrix, then reduced to a pure VM unit test):
the bug fires when (a) the short-circuit jump is taken, (b) the skipped RHS is a
multi-opcode expression (a bare variable is immune), and (c) the expression is a
call argument (widget method or user fn). `if` conditions, concat operands, and
`let` bindings were all immune. The argument arrives as **nil**.

**Root cause** (platform/script/src/parser.rs). Call arguments commit their value
to the args object via POP_TO_ME. When the argument's last opcode can carry it,
the parser **fuses** the commit as a flag onto that opcode (`set_pop_to_me`).
The short-circuit's jump target is patched to point *after* the RHS's last
opcode — so the fused commit sits **inside the skipped region**, and a taken
short-circuit jumps straight to CALL_EXEC without ever committing the value.
A bare-variable RHS is a raw value slot that can't carry the flag, so a
standalone POP_TO_ME lands *at* the jump target — which is why that case worked.

**Fix.** `ScriptParser` now tracks `last_short_circuit_target` (recorded at every
short-circuit jump-patch site); `set_pop_to_me` refuses to fuse onto that
boundary and emits a standalone POP_TO_ME instead, which both execution paths
reach. Regression tests: `platform/script/tests/short_circuit_args.rs` (8 cases:
parenthesized/unparenthesized/second-arg/or/host-called fn/controls).

## 2. A call as the final statement of a script never executes — [FIXED]

Found while writing the regression tests (it was making them pass *spuriously*).
After `foo(...)`'s closing `)`, the parser waits one token to rule out a trailing
`do {...}` block (`CallMaybeDo`). At end-of-source no token arrives, and both
end-of-parse unwind loops dropped the state via `_ => {}` — CALL_EXEC was never
emitted, the argument value leaked out as the script's return value. Splash
bodies rarely hit this (they end in a `View{...}` literal) but any probe/util
script ending in a call silently did nothing.

**Fix.** Both unwind loops (`parse()` and the streaming `auto_close`) now resolve
a pending `CallMaybeDo` as a plain call, and also patch a pending
`ShortCircuitEnd` (which would otherwise leave a **zero jump offset** — a third
latent bug that could cause an infinite loop on a trailing `a && b`). Streaming
is unaffected: an appended `do` block restores the checkpoint and re-parses.

## 3. `set_visible` treated a bad argument as `true` — [FIXED]

`View::script_call` parsed its argument with `.unwrap_or(true)` — the worst
possible default, since corrupted logic *shows* hidden widgets (this is what
amplified finding #1 from "wrong value" to "invisible views appearing").
Now a non-bool/non-number argument keeps the current visibility and returns a
proper script error (`set_visible expects a bool...`), which lands in the
standard error log like other `[E]` script errors.

## 4. `Sdf2d.box` degenerates into a diamond — [FIXED (clamp)]

The box SDF doubles its radius parameter internally (visual radius = `2*r`), and
when `2*r` exceeds the half-size the field degenerates into a rotated diamond —
the badge-rendering artifact we chased twice (both RoundedView and the gauss
LensSurface build on this SDF). The radius is now clamped to the half-size, so
oversized radii saturate at a circle/capsule instead. No correctly-sized usage
changes appearance. **[RECOMMEND]** the doubled-radius parameterization itself is
surprising (`border_radius: 5` → 10px visual) but changing it would re-skin every
existing makepad UI; document it instead. `box_x`/`box_y`/`box_all` have the same
degeneration and could get the same clamp.

## 5. Diagnostics added — [FIXED]

- `vm.bx.debug_trace = true` now traces every executed opcode (body/ip/stack/op)
  in `run_core` — the tool that cracked finding #1. Off by default.
- `ScriptParser::dump_opcodes()` already existed; the trace complements it.

## 6. Follow-up recommendations — [ALL IMPLEMENTED on makepad `splash_improvements`]

1. **Gauss/LensSurface at small sizes — [FIXED]** The refraction band
   (`lensing_width`) is now capped at 35% of the surface's smaller side, with
   `lensing_strength` scaled proportionally (`eff_lensing_width/scale` in
   gauss_view.rs). Surfaces ≥ ~63px are pixel-identical; small discs degrade
   gracefully instead of becoming smeared blobs. Applied to both the static and
   ripple lens shaders (shared edge function).
2. **Glass overlay draw order — [DOCUMENTED]** Module-level comment in
   gauss_view.rs states the rule: chrome above a glass surface must be a child
   of it; parent-drawn quads are covered by the later lens-overlay pass.
3. **Missing-glyph visibility — [FIXED]** The text shaper now logs one warning
   per unique codepoint that no loaded font can render
   ("no loaded font has a glyph for '⤡' (U+2921); rendering .notdef"),
   emitted at the fallback-exhausted point in shape_recursive.
4. **`use x.*` snapshot semantics — [DOCUMENTED]** Comment at the parser's
   `use` handling explains that glob imports copy the names existing at that
   point of evaluation, so same-block later registrations need full paths.
5. **Host→isolate API — [FIXED]** `Splash` now caches its script body id at
   eval (no more per-call pointer-identity scans, with a pointer fallback);
   `call_script_fn` returns `bool` (fn found or not) so hosts can distinguish
   optional hooks from typos; new `set_script_global(cx, key, value)` on
   `Splash`/`SplashRef` injects host-provided globals without re-evaluating.
6. **`text_style: {..}` type errors** are runtime-log-only; fine, but easy to
   miss — grep app logs for `[E]` after DSL edits. (Unchanged by design.)

Also completed alongside: the `Sdf2d.box` clamp was extended to `box_x`,
`box_y`, and `box_all` in BOTH shader libraries (the script-system
`draw/src/shader/sdf.rs` — the one Splash widgets actually use — and the
legacy live_design `std.rs`).

## 7. A `(` or `[` on the next line silently glued onto the previous value — [FIXED]

**Symptom.** The calendar's `(h + 6) % 7` computed inside a `fn` produced
`variable h not found` and `call target is not a function (got u40)`. The two
lines
```
    let base = weekday_of(first) % 7
    (h + 6) % 7
```
parsed as `... % 7(h + 6) % 7` — i.e. the grouped `(h + 6)` on the next line was
read as a **call** applied to the `7` ending the previous line. Splash statements
are newline-delimited, but the parser was continuing the expression greedily
across the newline (the JS "automatic-semicolon-insertion" footgun).

**The ambiguity.** Postfix `(` (call) and `[` (index) can *both* legitimately
begin a new statement (a grouped expression / an array literal) **and** act as
postfix operators on the previous value. So a leading `(`/`[` after a newline is
genuinely ambiguous. Infix operators (`+ - * / % && ||`) and `.` **cannot** start
a statement, so a leading one is unambiguously a continuation — makepad's own
shader DSL relies on this to wrap long math like `let c = a * 0.125\n + (...)`.

**Fix** (platform/script — tokenizer.rs + parser.rs). The tokenizer now records a
`preceded_by_newline` flag per token (set on the token *after* a `\n`, via a
`newline_pending` latch cleared at each emit). At statement level, a leading
`(`/`[` that is `preceded_by_newline` starts a **new statement** instead of
gluing as a postfix call/index; leading infix operators and `.` still continue
the expression. Inside `( )`/`[ ]` groupings newlines are insignificant
(Python/Swift-style), so the divert is suppressed there
(`inside_round_square_bracket()` walks the parser state stack). Regression tests:
`platform/script/tests/newline_call.rs` (8 cases: leading-paren-is-not-a-call,
same-line calls still work, multiline args, leading `[`, leading/trailing binary
operators continue, field access, operator-inside-parens). No mini-app scripts
had to change — the calendar's `(h + 6) % 7` now parses as intended.

## 8. `script_apply_eval!` on a custom widget couldn't resolve any names — [FIXED]

**Symptom.** `place_popup()` positions the context/background menus with
```
script_apply_eval!(cx, popup, { margin: Inset{left: #(x), top: #(y)} });
```
which failed with `__script_source__ not found` **and** `variable Inset not found
in scope`. The menus silently fell back to the Modal's top-left alignment instead
of anchoring to their trigger. This was pre-existing (reproduced on pristine
makepad), just never noticed because the fallback still drew *something*.

**Root cause 1 — no source (makepad derive).** `script_apply_eval` sets
`__script_source__` on the eval scope to `self.script_source()`, which is what
lets `+:` proto-inherit and name lookup resolve. The `#[derive(Script)]` macro
only generated `script_source()` for a struct with its own `#[source]` field. A
custom widget like `LauncherContextMenu` holds `#[deref] view: View` (and `View`
*has* the `#[source]`) but declared none itself, so its `script_source()` fell
back to `ScriptObject::ZERO` → `__script_source__` unset → every eval against it
failed. **Fix:** `derive_scriptable.rs` now forwards `script_source()` to the
`#[deref]` base when the struct has no `#[source]` of its own — so eval on any
custom widget that derefs to `View` behaves like eval on the bare `View`.

**Root cause 2 — eval scope has no `use`s (host_launcher).** Even with a valid
source, an eval body only sees the minimal scope (`mod`, `ui`,
`__script_source__`); the module-level `use mod.prelude...` imports that make bare
`Inset` resolve in normal DSL are **not** in eval scope, so the type has to be
named fully (`mod.turtle.Inset{...}`, since Inset is registered in `mod.turtle`).

**But `script_apply_eval!` was the wrong tool here anyway.** Spinning up the
script VM (tokenize → parse → allocate script objects → run) just to write two
numbers is wasteful, and makepad's own `CalloutTooltip` positions itself by
writing `view.walk.margin` directly. So `place_popup` now sets the margin on the
menu widget's `walk` directly (`Inset` is a plain Rust type via `makepad_widgets`;
the menu widgets are custom types that deref to `View`, reached through their
concrete type with `borrow_mut::<T>()`). Same treatment for the one other custom-
widget eval — the Splash `allow_net: true` set became a typed `Splash::set_allow_net`
setter. Verified headless: the menus still land at their computed anchor
(`LauncherBackgroundMenu 8 268 232 271`) with a clean log.

**Seen again (agent console, later).** Holding the console's height at its
high-water mark was first written as `script_apply_eval!(cx, out, { height:
Fit{min: FitBound.Abs(#(h)), ...} })` — and every name in it failed to resolve
for exactly the reason above, so the height never applied and the view drew at
zero height (the console simply never appeared). Fully qualifying to
`mod.turtle.Fit{...}` fixed it, but the right answer was again the typed route:
`view.walk.height = Size::Fit{ min: Some(FitBound::Abs(h)), .. }`, which is what
`apply_edit_bar_height` next door already does — and this one runs per event,
not once per tile. **Rule of thumb: if a Rust type exists for the field, write
the walk; reach for the eval only for shader instance vars.**

The root-cause-1 derive fix **stays** regardless: it's a general latent-bug fix
(a `script_apply_eval!` on any custom widget silently no-op'd instead of resolving
names), even though `place_popup` no longer relies on it. The remaining
`script_apply_eval!` sites all target plain `View`s (their `draw_bg` tint/border
are shader **instance** vars with no typed Rust setter — `color: instance(...)` —
so the eval is the sanctioned path there, and they run once per tile creation, not
per frame).

## 9. Re-focusing a field that never lost focus leaves it with no caret — [FIXED]

**Symptom.** After the create bar's **New prompt**, the composer accepted
typing but drew no caret and no selection highlight. It came right on its own
after collapsing and expanding the bar a few times, which is what made it look
like a rendering glitch rather than a state bug.

**Root cause.** `TextInput` draws its caret as
`mix(hidden, color, (1.0 - self.blink) * self.focus)`. Both `blink` and `focus`
are animator instances, and the only thing that turns them on is
`Hit::KeyFocus`:

```rust
Hit::KeyFocus(_) => {
    self.animator_play(cx, ids!(focus.on));
    self.reset_blink_timer(cx);
    ...
```

A run hides the composer behind the console — and **hiding a widget does not
clear `Cx`'s key focus**, so the field still holds it the whole time. When
"New prompt" put the composer back and asked for focus, the platform saw the
field already had it and dispatched no hit at all. The animators stayed parked
where the previous focus-lost had left them: `focus.off`, `blink.on`. Typing
worked throughout, because typing only needs `Cx`'s key focus — the caret is
purely animator state. The "fixes itself eventually" part was a real click
outside (a genuine `KeyFocusLost`) followed by a real click inside (a genuine
`KeyFocus`), which put the animators back.

**Fix.** `TextInput::take_key_focus` on makepad `splash_improvements`: sets key
focus **and** plays `focus.on` + resets the blink, unconditionally. Repairing
the case where focus did *not* change is the whole point, so it must not be
gated on the focus having moved. Use it in place of the generic
`Widget::set_key_focus` anywhere the app hands focus to a field itself.

**Rule of thumb: `set_key_focus` is a no-op when the target already has focus,
so anything that rides on the resulting event — visuals, actions, IME sync —
silently doesn't happen.** The same no-op is why the composer's options row
had to be opened explicitly by "New prompt" rather than left to the field's
own `KeyFocus` action.

## 10. Folding a field by `max_lines` breaks click-to-place-caret — [FIXED]

**Symptom.** After the composer collapsed (on blur) and expanded again (on
focus), clicking into the text put the caret in the wrong place and dragging
selected the wrong span.

**Root cause.** The fold was `max_lines` 1 ⇄ 0, which re-lays the text out into
a different shape. That laid-out text is also what maps a click to a caret
position (`point_in_lpxs_to_cursor`), and it is only rebuilt at **draw** time
(`layout_text`, keyed on width + `max_lines`). So the very press that
re-focused the composer was resolved against the *folded* one-line layout while
the expanded one was already on screen. No ordering fixes this: within an event
pass there is no `Cx2d`, so the field cannot be re-laid-out before the click is
handled.

Clearing `laidout_text` in the setter is worse, not better — every cursor
operation for the rest of that event batch then bails out with "can't move
cursor because layout was invalidated by an earlier event" and silently returns,
so the click places no caret at all.

**Fix.** Don't re-lay out to fold. Pin the field's **height** to one line
(`TextInput::set_height`) and let it clip its own overflow: the layout is
identical folded or open, so a click always means what it looks like.

Two things this cost on the way, both found by measuring rather than reasoning:

- Clamp the **field**, not a clipping parent. The field's `max` is `Rel` to the
  space its parent offers, so clamping the parent shrank the field to 75% of one
  line and sliced the text through the middle of its glyphs.
- The clamp must be a **whole number of lines**. It clips, so anything in
  between cuts the last line in half. 44 = one line + the field's 10/10 padding,
  which also leaves the idle face at `create_head`'s 50 and the bar at 404×64,
  matching the busy face so it doesn't resize when a run starts.

## 11. `ButtonFlat`'s defaults push an icon off-centre in an `Overlay` — [FIXED]

**Symptom.** The Providers page's trash button drew its icon 3px below the
centre of its disc, and slightly left of it.

**Root cause.** `ButtonFlat` is built for a text button with an optional
leading icon, and three of its defaults move the glyph:

- `margin: theme.mspace_v_1` — a 3px vertical inset. In a `flow: Overlay`
  holder, a child that already fills the holder has no slack to absorb a margin,
  so it is simply placed 3px down. Measured: the button box at y=157 inside a
  wrapper at y=154.
- `spacing: theme.space_2` — the icon/label gap, still applied when the label is
  empty, so centring the [icon + gap + nothing] row leaves the icon left of true.
- `padding` — the text button's left/right breathing room.

**Fix.** Zero all three (`margin: 0, spacing: 0, padding: 0`) plus
`align: Align{x: 0.5, y: 0.5}` on any icon-only `ButtonFlatter`. Note this
matters even when the button is invisible: the create bar's send button is a
transparent hit target over a `RoundedView` disc, and the same 3px drop put its
tap area off the disc it belongs to.

## 12. `on_render` widget emission is fragile around branches — [FIXED upstream 2026-08-07]

Found 2026-08-06 while making every mini app responsive. Symptom: an
`on_render` closure runs to completion (no `[E]` log, probes at the top of the
closure render) yet some or all of its widgets silently never appear.

Bisected rules, each verified with A/B probe rows in a live app:

- **A widget literal emitted from an `if`/`else` branch is unreliable** — the
  executed branch's widgets are often (not always) dropped. This includes the
  common `if narrow { TierA{...} } else { TierB{...} }` shape and a bare
  `if cond { View{...} }` at closure level. Whether it survives seems to
  depend on surrounding opcodes, reminiscent of the fixed short-circuit
  POP_TO_ME parser bugs (§ "VM bugs" in the project notes) — do not rely on it.
- **`for x in xs` inside a widget literal emits nothing** (a `while` with an
  index works everywhere).
- **An `Inset{...}` property with an INTEGER field kills the literal**
  (`margin: Inset{top: 6}` = dead row; `Inset{top: 6.0}` or a float variable
  is fine). Scalar integer props (`height: 30`) are fine.
- **glass container prototypes (`glass.Card`/`Group`/`ListRow`) with computed
  content are unreliable as emission roots**; plain `View`/`RoundedView` roots
  with glass/Label prototypes as descendants (variable props included) work.

Safe patterns (all verified): emit PLAIN `View`/`RoundedView` literals at
closure statement level or from `while` loops; chunk/pad with count-driven
`while` loops (compute `lead`/`fit`/`trail` with value-only `if`s); pick
prototypes by VALUE (`let B = DayBtn  if compact { B = DayBtnSm }  B{...}`);
split alternative layouts (grid vs lightbox) into SEPARATE `on_render` views
toggled with `set_visible` instead of branching inside one closure.
`apps/calendar.splash`, `apps/weather.splash`, `apps/gallery.splash`, and
`apps/news.splash` are the reference implementations.

UPDATE 2026-08-07 — fixed in makepad (`splash_improvements`), with pure-VM
regression tests in `platform/script/tests/on_render_emission.rs`:

- Branch joins (if/else, elif, match, try/err) are now recorded like
  short-circuit targets, so the statement's POP_TO_ME lands standalone at the
  join and every taken path commits. `elif` itself was also rewritten (it
  previously left its IF_ELSE jump unpatched = an interpreter spin that
  silently burned the isolate's whole instruction budget).
- `for x in xs` inside literals works; a for over a NON-iterable (string,
  bool) now raises "for loop source is not iterable" instead of silently
  skipping the body (nil / empty stay silent). `for k v in <number>` binds
  correctly on every iteration.
- A line-leading `{` after a value line starts a new statement (same divert
  rule as `(` and `[`).
- Numeric subtypes (int literals etc.) collapse into the number bucket:
  `x.is_number()` works on ints, and int args no longer fail float-default
  checks with "expected number, got number". NOTE: the "integer `Inset`
  field kills the literal" rule reported here earlier was a misattribution —
  that probe's real killer was being the closure's FINAL statement (below).
- An `on_render` closure's FINAL statement used to become its (discarded)
  return value; the host now commits a returned widget object as the last
  child, so ending a closure with a widget literal works. This was also why
  wrapping everything in one extra `View{}` emitted nothing.
- `View::script_result` now LOGS a failed render closure instead of silently
  discarding its output — the reason this whole family was invisible.

The safe patterns above remain good style (and the apps keep them), but new
code no longer needs to avoid branch emission, for-in, or final-statement
widgets. Known quirks still open (pre-existing, out of that fix's scope): a
bare ident line directly after a `}`/`;`-ending line can glue onto the
previous statement (write `let out = r` on a fresh line instead), and
overlapping `match` arms may run more than one body.

## 13. Phantom "variable <raw hex id> not found" on every app open/resize — [FIXED upstream 2026-08-07]

The log filled with `variable 00001e93e419c77c not found in scope` errors,
attributed to lines like `let _boot = start_timeout(0.05, || refresh())` in
apps that don't define `fn on_app_resize`. Both parts of the message were
misleading: the raw hex is `id!(on_app_resize)` printed without a name (Rust
`id!()` hashes aren't in the reverse-lookup table), and the line:col is the
STALE instruction pointer from the end of the module eval (the last closure
compiled), not where anything failed. Root cause: `Splash::call_script_fn`
probed for optional hooks with a trapping `scope_value` — the miss was
handled, but the trap had already queued a NotFound into the error log. Fixed
upstream by probing with `NoTrap`. The deferred-timer-closure path itself was
verified healthy in pure-VM tests.

## 14. `let c = <lambda>` as a module's FINAL statement never bound — [FIXED upstream 2026-08-07]

The parser's end-of-source auto-close dropped still-open `EndFnExpr` /
`EndFnBlock` / `EmitLetDyn` states via a catch-all: the lambda's jump-over
stayed 0 (so its body ran INLINE at eval time, ending the module early with
"FN_BODY_DYN: me stack is empty" queued) and the `let` opcode was never
emitted (the binding silently didn't exist). Both drivers (parse + streamed
auto_close) now close those states like the live handlers do. Side effect
worth knowing: the old test read-out idiom `let out = r\nout` only ever
worked BECAUSE the trailing let was dropped (RETURN popped the naked value);
with the let now binding, end scripts with a CALL instead — `echo(r)` with
`fn echo(x){ x }`. Regression tests: makepad
`platform/script/tests/auto_close_eof.rs`.

## Verification

- `cargo test` in platform/script: all pass (8 newline + 8 short-circuit + reload
  + eval regression tests).
- makepad-widgets builds clean; launcher rebuilt against the fixed VM and derive.
- host_launcher: full 17-test headless suite re-run green; home/bigwidgets/edit
  states visually re-verified (widgets reflow correctly with the parser fix in
  place, badges/grips/dock unchanged); bgmenu/ctxmenu states now render anchored
  with a clean log.
