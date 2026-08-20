# host_launcher — agent guide

A Makepad app styled like a phone home-screen launcher that hosts isolated Splash
mini-apps. Read `README.md` for what it is and `docs/DESIGN_DECISIONS.md` for the
choices made.

## Build & run

- `cargo run` / `cargo run --release` — the live GPU app (a resizable, phone-shaped
  desktop window).
- Tracks `makepad/makepad` `dev` by git dep. The host-services bridge this
  project needed (makepad/makepad#1181) is merged upstream, so no local
  worktree is required. For local makepad hacking, point ALL THREE makepad
  deps (including `makepad-test` under `[dev-dependencies]`) at the same
  tree, or the widget/editor/test types won't match.

## Testing

- Headless UI suite: `HOST_LAUNCHER_FRESH=1 cargo test --test ui -- --test-threads=1`.
  It drives a real (offscreen) build of the app via `makepad_test` with synthesized
  input and widget-state assertions. `HOST_LAUNCHER_FRESH=1` starts every instance
  from the default layout and skips persistence so tests are order-independent.
- Do NOT edit any source (Rust or `.splash`) while a `cargo test --test ui` run is
  in progress: the harness rebuilds the app per session, and a mid-run edit yields
  spurious "app exited with code 0" failures. Let the run finish first.
- `timed out waiting for hub response` means SLOW FRAMES, not a hang. The harness
  allows a hard 10s per reply, and every `widget_snapshot()` first pumps 3 `Tick`
  messages — so one query costs ~4 rendered frames, and any state costing over
  ~2.5s a frame fails. Software rasterisation is what's expensive, so the lever is
  what gets composited (glass surfaces, overlay draw lists), not what's computed.
  A whole-suite run heats the box up enough that the heaviest tests tip over near
  the end; a lone retry of one is the way to tell a real failure from that.
- The suite is slow for reasons that live in makepad, not here, and two PRs fix
  most of it: makepad/makepad#1175 (the headless backend recompiled all 68 shaders
  with `rustc -O` on EVERY app start — 38.7s of the ~45s each test spends, plus a
  per-frame re-conversion of the glyph atlas) and makepad/makepad#1176 (test
  harness knobs). Until they land, point all three makepad deps at a local
  checkout with those branches to get it — see the comment in `Cargo.toml`; ALL
  THREE must be the same tree. With them: a steady frame goes 840ms -> 127ms, and
  the full suite goes from many hours to 67 min green.
- `MAKEPAD_HEADLESS_DPI=1` (needs #1175) renders at 1x instead of the hardcoded
  2x. The suite only asserts logical geometry, so it is a straight 3-4x on raster
  for nothing lost — and it is what gets the slowest tests off the 10s cliff.
- `MAKEPAD_TEST_PARALLEL=1 cargo test -- --test-threads=4` (needs #1176) runs the
  suite in ~11-20 min instead of 67, but 2-3 tests fail per run and the set
  CHANGES between runs. Fine for local iteration, not for a verdict: several tests
  wait by counting polls, and how much wall clock and how many frames a poll buys
  both collapse under that load. Verify serially.
- Long-press in headless: the desktop path emits no LongPress event, so forward raw
  `MouseDown` … poll `widget_snapshot()` past 0.5s … `MouseUp`. A bare `sleep` won't
  advance the app's timers; you must keep it pumping (see `tests/ui.rs`).
- Live GPU verification: run with `HOST_LAUNCHER_AUTOMATION_DIR=<dir>` and touch
  `<dir>/shot_request` (writes `<dir>/frame.png` via `capture_next_frame_to_file`),
  `<dir>/tree_request` (widget tree), or `<dir>/activate_request` (raise the window).
  `HOST_LAUNCHER_DEBUG_STATE=open:<app>|drawer|edit` jumps to a state on startup.

## Code conventions (follow Robrix's)

- One widget family per file; `script_mod!` UI at the top, then the `#[derive]`
  struct, `ScriptHook`, `Widget`, then the `Ref` ext and the file's action enum.
- Comments: 2-3 lines max, say *why* not *what*, no em-dashes, no fix-history.
- `AppState` is shared to widgets via `Scope::with_data`; read it with
  `scope.data.get::<AppState>()`.
- Apply `SAFE_INSET_PAD_*` in whichever widget draws into an edge.

## Writing a mini-app (`apps/*.splash`)

Pure Splash, no Rust. Key rules (see `../makepad/splash.md`):
- A plain `View{on_render}` does NOT auto-run on first draw — call `ui.<view>.render()`
  once at boot (`let _boot = start_timeout(0.05, || ui.x.render())`).
- Dynamic lists whose items have per-row `on_click` handlers don't re-render on
  array growth: use the pre-allocated fixed-slot pattern + `ui.<row>.set_visible()`.
- Isolation: `mod.fs` is a per-app jail, `mod.run`/`mod.res`/`cx.quit` are gone,
  and the network exists only when the user grants the declared `network`
  permission; `ui` only reaches this app's own widgets. `std.time_now()`/
  `std.local_time()` give the clock; `std.start_interval` ticks inside the isolate.
- Host services (location, clipboard, notifications, IPC, files, …) go through
  `host.request(service, args, cb)` behind per-app user grants — the full model,
  service catalog, and script API live in `docs/PERMISSIONS.md`. Apps declare
  permissions in `permissions_for` (`src/mini_apps/builtin.rs`) and MUST stay
  fully usable with zero grants (fallback content, never a blank state).
- Parser landmines for callback-heavy code: never end a `fn`/closure with an
  `if`/`else` (use early `return nil`s — a final if parses as an expression and
  errors); keep `} else {` on one line; the bridge result field is `r.is_ok`
  (`ok` is a keyword, `r.ok` never parses as field access); and `.to_chars()`
  yields char CODES, use `.split(...)` for string work.
- Service-broker debugging: `HOST_LAUNCHER_TRACE_SERVICES=1` appends every
  bridge dispatch/response (app, service, grants, outcome) to
  `/tmp/host_launcher_services.log` — works inside the UI test harness too.
- Apps must handle ANY host size (split-screen panes ~190w/~250h up to wide
  desktop windows). Cap+center the column with `width: Fill{max: N}` under an
  `align: Align{x: 0.5}` parent, and define `fn on_app_resize(w, h)` (called on
  open + every settled size change) to toggle pre-declared tier Views via
  `set_visible` — fonts/fixed sizes can't change at runtime. See
  `apps/calculator.splash` for the canonical shape.
- Compile-check every app's Splash from the CLI:
  `MAKEPAD=headless HOST_LAUNCHER_FRESH=1 HOST_LAUNCHER_DEBUG_STATE=validate cargo run`.
- Register a new app in `src/mini_apps/builtin.rs`.
