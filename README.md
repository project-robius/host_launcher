# Host Launcher

A home-screen launcher app, built in pure Rust with [Makepad](https://github.com/makepad/makepad)
and the [Project Robius](https://github.com/project-robius) app-dev framework.

It looks and behaves like an iOS/Android home screen — paged grids of app icons,
swipe navigation, long-press-to-rearrange, an app drawer, and live home-screen
widgets — but every "app" is actually a **Splash mini-app**: a chunk of Makepad's
Splash UI-scripting DSL that runs inside its own isolated Splash VM, hosted by the
single `host_launcher` process.

![the home screen](docs/screenshots/home.png)

## What it is

The launcher is one regular Makepad desktop/mobile app that merely *looks* like a
phone launcher. Each icon is a shortcut to a mini-app written entirely in Splash
(no per-app Rust). Tapping an icon zooms it up into a nearly-fullscreen view; a
back/close button (or the Android back gesture) zooms it back into its icon.

Mini-apps are strongly isolated: each runs in its own Splash VM (`ScriptHeap` +
`ScriptStd`), can't reach the host UI or a sibling app's widgets, can't touch the
filesystem or spawn processes, and can't hang the host (a runaway loop is cut off
by a per-entry time budget). See [Isolation](#isolation).

## Features

- **AI "create app" bar**: a Google-style pill on the home screen; type what
  you want and an ACP agent (octos by default) writes a Splash mini-app, the
  launcher validates it with the real parser (with automatic repair turns),
  and the finished app installs like any other. See
  [docs/GENERATE.md](docs/GENERATE.md).
- **Paged icon grid** with iOS-style horizontal swipe, flick velocity, page
  snapping, and rubber-banding at the ends. A dot indicator tracks the position.
- **App drawer** (Android-style): swipe up (or tap the chevron) for a scrollable
  grid of every installed app, with a sort toggle for **Alphabetical ⇄ Recent**.
- **Long-press editing**: hold an icon to lift it, then drag to rearrange (with
  edge-of-screen page flips and drop-to-swap). Release in place for a context
  menu (Open / Remove from Home / Add Widget / Force Stop / Uninstall / …).
- **Home-screen widgets**: an always-running Splash mini-app pinned to a resizable
  grid-cell span, exactly like Android widgets. The bundled Clock and Weather
  widgets ship on the default home screen.
- **Icon → fullscreen animation** on open, and the inverse on close.
- **Keep-alive**: closing a mini-app keeps its VM running (state survives a
  reopen); "Force Stop" tears the VM down and restarts it fresh next open.
- **Back-button / Escape** navigation that unwinds context menu → mini-app →
  drawer → edit mode → non-first page, matching platform expectations.
- **Liquid-glass theme**: a live animated vector backdrop with translucent glass
  surfaces that refract it, following the `makepad-example-aichat` look.
- **Safe-area insets**: content pads itself out of the device's unsafe areas
  while the backdrop bleeds under the system bars.
- **Reflows** from a phone-shaped window to any desktop window size.
- **Persistence**: home layout, recents, and user-installed apps are saved as
  JSON under the platform data dir (via `robius-directories`).

## Pre-installed mini-apps

All written in pure Splash (`apps/*.splash`). The built-ins can't be uninstalled;
two samples (Counter, Stopwatch) are "user-installed" so uninstall is testable.

| App | What it demonstrates |
|-----|----------------------|
| Weather | Tap-a-day forecast with hourly bar chart; also provides a home widget |
| Clock | Live ticking time + world clocks; also a home widget (uses the timer + wall-clock APIs) |
| News | Headline list with tap-to-read detail |
| To-Do | Add / toggle / clear tasks (struct-array state, dynamic list rendering) |
| Notes | Write and save notes |
| Calculator | Numeric glass calculator |
| Settings | Quick-settings mockup with glass toggles |
| Calendar | Month grid with prev/next navigation and day selection (date math) |
| Music | Mock player with a working queue and play/pause state |
| Gallery | Photo grid with a lightbox detail view |
| Counter | Minimal counter (deletable sample) |
| Stopwatch | Lap timer (deletable sample; uses interval timers) |

Editing an `apps/*.splash` file and reopening the app picks up the change without
a rebuild (the sources are read from disk in a dev checkout, and baked into the
binary for release).

## Running

```sh
cargo run                 # desktop (opens a phone-proportioned, resizable window)
cargo run --release       # optimized
```

Requires the sibling `../makepad` checkout (this crate uses it as a path
dependency; it's pinned to the same commit Robrix uses).

## Testing

Headless UI tests drive a real build of the app through Makepad's `makepad_test`
harness (synthesized clicks/drags/keys + widget-state assertions), rendered
offscreen via the software rasterizer:

```sh
HOST_LAUNCHER_FRESH=1 cargo test --test ui -- --test-threads=1
```

`HOST_LAUNCHER_FRESH=1` starts each app instance from the default layout and skips
persistence, so tests are order-independent and don't touch your real launcher
state. Failure artifacts land in `target/makepad_test/host_launcher/<test>/`.

For visual verification of the live GPU-rendered glass, the app supports two
env-gated debug hooks (see `src/app.rs`): `HOST_LAUNCHER_AUTOMATION_DIR=<dir>`
enables a file-triggered `capture_next_frame_to_file` screenshot + widget-tree
dump, and `HOST_LAUNCHER_DEBUG_STATE=open:<app>|drawer|edit` jumps straight to a
UI state on startup.

## Architecture

```
src/
  app.rs                     top-level App: state, action routing, back-nav, persistence
  persistence.rs             JSON save/load of layout + window geometry
  shared/styles.rs           glass backdrop + icon-tile styles
  launcher/
    home_screen.rs           home composition (pager + indicator + drawer handle)
    home_pager.rs            paged grid: swipe, long-press, drag-reorder, widget tiles
    page_indicator.rs        the page dots (custom SDF shader)
    app_drawer.rs            swipe-up drawer with sort toggle
    context_menu.rs          long-press context menu (in a Modal)
  mini_apps/
    registry.rs              app manifests + persistable layout model
    builtin.rs               the pre-installed app manifests
    mini_app_screen.rs       fullscreen Splash host + open/close animation + keep-alive
apps/*.splash                the pure-Splash mini-app sources
docs/DESIGN_DECISIONS.md     design questions and the answers chosen
```

The launcher follows Robrix's conventions: `script_mod!` UI, one widget family per
file, `AppState` shared to widgets via `Scope`, `robius-directories` for paths,
and `SAFE_INSET_PAD_*` applied by whichever widget draws into an edge.

## Isolation

Each mini-app is hosted by a `Splash` widget, which allocates a dedicated Splash
VM. To make these safe to host mutually-untrusting apps, the local `makepad`
checkout was extended (uncommitted, for this project):

- **Ambient authority removed**: `mod.fs` (filesystem) and `mod.run` (process
  spawn) are stripped from every isolate's module namespace, and raw
  `net.socket_stream` is gated behind the same net-runtime check as HTTP — so a
  mini-app with `allow_net: false` has no I/O escape hatch at all.
- **UI reach confined**: an isolate's `ui` handle (and `ui.root`) only resolves
  widgets within that mini-app's own subtree, so it can't find or drive host or
  sibling-app widgets by name.
- **Isolate-safe timers**: `std.start_timeout` / `start_interval` callbacks are
  dispatched back into the owning isolate VM (they previously ran against the main
  VM's heap), and stale timers are torn down when an isolate is reclaimed.
- **Wall-clock API**: `std.time_now()` and `std.local_time()` were added so pure
  Splash apps (the Clock, Calendar) can read the time; the host supplies the local
  UTC offset.

Runaway scripts are already contained by Makepad's per-entry 64 ms time budget and
200k-instruction limit, so an infinite loop in an `on_click` cannot hang the
launcher or starve other apps.
