# Design Decisions

The prompt said to auto-pick the top recommended answer for any open question and
present them at the end. Here they are, grouped, with the choice made and a short
rationale. None are hard to change.

## Dependencies & toolchain

1. **makepad via git or a local path?** → **Local path to `../makepad`.** The
   checkout is at the exact commit Robrix pins (`gl_sampling_fallback`), so versions
   match, and a path dep let me extend makepad locally (which the project needed).

2. **UI system?** → **The `script_mod!` Splash system** (not the deprecated
   `live_design!`), matching Robrix and all current makepad examples on this branch.

## Local makepad changes (uncommitted, for this project)

Hosting mutually-untrusting Splash mini-apps required extending the local makepad.
All changes are behavior-preserving for existing apps:

- **Isolate ambient authority removed** (`widgets/src/widget_async.rs`): each Splash
  isolate has `mod.fs` and `mod.run` stripped from its namespace, and
  `net.socket_stream` is gated on the net runtime (`platform/script/std/src/net.rs`)
  like `http_request` already was. An `allow_net: false` app now has no I/O escape.
- **`ui` handle confined to the app's subtree** (`widget_async.rs`): an isolate's
  `ui.<id>` / `ui.root` only resolves within that mini-app's own widgets, so it
  can't reach host or sibling-app widgets by name.
- **Isolate-safe script timers** (`platform/src/script/timer.rs` +
  `widget_async.rs`): `std.start_timeout`/`start_interval` callbacks are dispatched
  back into the owning isolate VM via a new timer-dispatch hook (they previously ran
  against the main VM's heap), and stale timers are torn down when an isolate is
  reclaimed.
- **Wall-clock API** (`timer.rs`): added `std.time_now()` and `std.local_time()` so
  pure-Splash apps (Clock, Calendar, Stopwatch) can read time; the host supplies the
  local UTC offset via `set_script_local_utc_offset_secs`.
- **`ui.<view>.set_visible(bool)` from Splash** (`widgets/src/view.rs`): lets a
  mini-app show/hide its own widgets, needed for list apps that use pre-allocated
  fixed slots (the reliable dynamic-list pattern).
- **Snapshot effective visibility** (`widgets/src/widget_tree.rs`): the widget
  snapshot now reports a widget as hidden if any ancestor is hidden (was: own flag
  only). General correctness fix; also makes a backgrounded mini-app's widgets test
  as hidden.
- **`gc_dead_splash_isolates` made public** + `macos_activate_app` / headless no-op,
  used by the launcher's force-stop and test harness.

## Mini-app hosting & isolation

3. **One isolate per app?** → **Yes**, one `Splash` widget (its own VM) per app.
   Makepad's per-entry 64ms budget + 200k instruction limit already contain runaway
   loops, so one app can't hang the launcher.

4. **Keep running when closed, or restart?** → **Keep running** (iOS-style). Closing
   hides the host but keeps the VM alive so state survives a reopen; "Force Stop"
   drops the host and its VM, restarting fresh next open.

5. **Network for mini-apps?** → **Off by default, opt-in per manifest** (`allow_net`).
   The bundled mock apps don't need it, so they all run sandboxed.

## Launcher UX

6. **Long-press style?** → **Android lift-and-drag.** Long-press lifts an icon;
   moving drags it (reorder, edge page-flips, drop-to-swap); releasing in place shows
   a context menu. Desktop has no long-press event, so it's implemented with a timer.

7. **Remove vs uninstall?** → **Remove = off the home screen** (stays in the drawer);
   **Uninstall = gone entirely** (user apps only). Two samples (Counter, Stopwatch)
   ship as user-installed so uninstall is testable.

8. **Dock (favorites row)?** → **Not in v1.** The paged grid + drawer + widgets cover
   the requested behavior.

9. **Open animation?** → **Animated rect-morph** of the host container from the icon
   rect to nearly-fullscreen (with matching corner radius), reversed on close. Cheaper
   than a render-to-texture zoom and reads correctly.

10. **Drawer trigger?** → **Swipe up anywhere on the home screen** (Android-style),
    plus a chevron handle for discoverability/mouse. Vertical vs horizontal is decided
    by the dominant axis of the first movement.

11. **Drawer sort?** → **One top-right button toggling Alphabetical ⇄ Recent.**
    "Recent" = most-recently-opened first (persisted).

## Widgets

12. **Home-screen widgets?** → **Android-style:** a Splash instance pinned to a
    grid-cell span, always running, movable/resizable in edit mode. The Clock and
    Weather widgets ship on the default home screen.

13. **Do Splash apps have timers/clock?** → They do now (see makepad changes). The
    Clock/Stopwatch use `start_interval`; Clock/Calendar use `local_time()`.

## Platform behavior

14. **Back-button order?** → context menu → mini-app → drawer → edit mode →
    non-first page → (unhandled, OS default). Escape mirrors it on desktop.

15. **Desktop reflow?** → **Fixed 4-column grid** whose cell size scales with window
    width; rows from height. Opens phone-shaped, resizes arbitrarily.

16. **Persistence?** → **JSON via serde** in the `robius-directories` data dir: home
    layout, recents, and user-installed apps (built-ins are constructed in code).

## Testing

17. **How to verify?** → **Headless `makepad_test` suite** (`tests/ui.rs`) that drives
    a real build with synthesized clicks/drags/keys and asserts widget state, plus
    live GPU-app screenshots (via an env-gated `capture_next_frame_to_file` hook) for
    the liquid-glass look. Two debug env hooks exist: `HOST_LAUNCHER_AUTOMATION_DIR`
    (file-triggered screenshot/tree-dump) and `HOST_LAUNCHER_DEBUG_STATE`
    (jump to open-app/drawer/edit on startup). `HOST_LAUNCHER_FRESH=1` makes tests
    start from the default layout and skip persistence.
