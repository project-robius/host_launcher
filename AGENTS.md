# host_launcher — agent guide

A Makepad app styled like a phone home-screen launcher that hosts isolated Splash
mini-apps. Read `README.md` for what it is and `docs/DESIGN_DECISIONS.md` for the
choices made.

## Build & run

- `cargo run` / `cargo run --release` — the live GPU app (a resizable, phone-shaped
  desktop window).
- Depends on the sibling `../makepad` checkout as a path dependency (same commit
  Robrix pins). This project made local, uncommitted changes to that checkout; see
  `docs/DESIGN_DECISIONS.md` "Local makepad changes". Do not commit/push them.

## Testing

- Headless UI suite: `HOST_LAUNCHER_FRESH=1 cargo test --test ui -- --test-threads=1`.
  It drives a real (offscreen) build of the app via `makepad_test` with synthesized
  input and widget-state assertions. `HOST_LAUNCHER_FRESH=1` starts every instance
  from the default layout and skips persistence so tests are order-independent.
- Do NOT edit any source (Rust or `.splash`) while a `cargo test --test ui` run is
  in progress: the harness rebuilds the app per session, and a mid-run edit yields
  spurious "app exited with code 0" failures. Let the run finish first.
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
- Isolation: `mod.fs`, `mod.run`, and (without `allow_net`) the network are blocked;
  `ui` only reaches this app's own widgets. `std.time_now()`/`std.local_time()` give
  the clock; `std.start_interval` ticks inside the isolate.
- Register a new app in `src/mini_apps/builtin.rs`.
