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

## Launcher-parity round (2026-07-16)

A second pass drove the shell much closer to a real iOS/Android launcher. Choices:

18. **Drag reflow model?** → **Swap, not global compaction.** Dropping an icon on an
    empty cell lands it there; dropping on another icon swaps that icon back into the
    dragged icon's old cell. An iOS-style "insert and flow everything to the top-left"
    was tried first but is wrong for this grid, which intentionally allows gaps around
    widgets. A live push-aside preview animates the swap partner during the drag, with
    a dashed drop-target outline and a lifted shadow/scale on the picked-up icon.

19. **Jiggle + menu together?** → Long-press (or right-click) an icon **both** starts
    the iOS jiggle (edit mode) **and** opens the Android-style shortcut menu, with the
    finger still down. Sliding the finger dismisses the menu and turns into a drag;
    tapping empty background exits edit mode. The pager keeps processing an in-flight
    gesture even while the menu modal is up so the slide-to-drag works.

20. **Menus?** → Two liquid-glass menus. Per-app (`LauncherContextMenu`): app glyph +
    name, up to four quick-action shortcuts (from `MiniAppManifest.shortcuts`), then
    Open / App info / Add-Remove / Jiggle & Edit / Force Stop / Uninstall. Background
    (`LauncherBackgroundMenu`, right-click empty space): Edit Home Screen / Search /
    All Apps / Change Wallpaper. Both are gauss-lensing glass panels in a `Modal`.

21. **Dock?** → A persistent bottom glass bar (`LauncherDock`) of label-less favorite
    icons, shown on every page, seeded with weather/notes/todo/music (kept off the
    grid). Custom-drawn; hit-rects are stored relative to the draw origin and rebased
    onto the resolved area, because a widget below a Fill sibling can't trust
    `turtle().rect()` at draw time.

22. **Search?** → Two entry points. Swipe **down** drops a Spotlight-style
    `SearchOverlay` (auto-focused field + live-filtered app grid, reusing drawer
    cells). The app drawer also grew its own **search field** at the top. Both filter
    by case-insensitive name substring.

23. **Interactive drawer?** → Swipe **up** now follows the finger: the pager drives
    the drawer's open fraction continuously (`DragDrawer`), and on release snaps open
    or closed by position + velocity (`ReleaseDrawer`). Chrome buttons are all glass
    (`GlassButton`); remove/resize badges are glass `LensSurface` discs top-left;
    multi-cell widgets get a `WIDGET_GAP` inset so they no longer touch.

24. **Widget content reflow on resize?** → A host→isolate size hook. makepad's
    `Splash` grew a generic `call_script_fn(cx, name, args)` (budgeted, silently
    a no-op if the script doesn't define the fn); the pager calls the script's
    optional `on_widget_resize(cols, rows, w, h)` on first draw and whenever the
    tile's span or pixel size changes (grid resize, window resize). Scripts adapt
    by toggling pre-declared id'd Views (`ui.<id>.set_visible` is View-only, and
    fonts/flow can't change at runtime, so alternate layouts are pre-declared):
    the clock swaps a 34pt face for a 48pt one at 3+ columns and reveals world
    clocks at 2+ rows (Tokyo needs 3), the weather widget goes compact-row at
    1-row height, full stack at 2+, and adds a 5-day forecast strip at 3+ columns
    (pairing it with the horizontal hero when only 2 rows tall). Tile content is
    now center-aligned so every layout sits nicely at any size.

25. **Dock v2** → full-width `glass.LensSurface` pill (real gauss lensing, like
    the menus) instead of the flat translucent bar, five favorites
    (weather/notes/todo/music/gallery), outer icons inset clear of the lens rim
    where refraction would warp them. Saved 4-slot docks are topped up to five,
    and an app promoted into the dock loses its grid icon so it isn't duplicated.

## Polish round (2026-07-16, later)

26. **Badge & grip rendering** → Remove badges are crisp RoundedView discs pinned
    to icon/widget tiles' top-left corners via dedicated holder views (the icon
    tile centers overlay children, so badges need their own align). Two shader
    facts drove this: the RoundedView SDF's visual corner radius is 2x its
    `border_radius` (and it degenerates into a diamond past half the box size),
    and the theme font has no resize-arrow glyph — so the widget resize grip is
    an SDF-drawn disc-plus-arrow (`DrawResizeGrip`) painted by the pager itself.

27. **Notification badges** → `NotifBadge` ports Robrix's UnreadBadge: fixed
    27x18, the red oval shrinks around short counts via `border_size`, caps at
    "99+", and fades its core through a warm tone for anti-aliasing. Counts live
    in `AppState.notifications` (demo-seeded: news 3, calendar 25, music 104);
    every ancestor view up the icon tile sets `clip_x/clip_y: false` so the
    corner overhang isn't cut (same as Robrix's rooms list), and the dock is
    tall enough to give its badges headroom.

28. **Glass widgets** → WidgetTile is a `glass.Card` styled like aichat's
    message bubbles (blue tint 0.14, lensing 0.6, soft shadow) instead of the
    old flat translucent RoundedView — real edge refraction and much better
    contrast against the backdrop.

29. **Clock scaling** → the time face (always including the seconds ticker)
    comes in three pre-declared tiers (25/34/48pt) picked by pixel width, since
    scripts can't change fonts at runtime. The date line requires a 2-row tile.
    Discovered along the way: chained `!var` boolean logic misbehaves in Splash
    scripts (and script `set_visible` defaults truthy on bad args, so broken
    logic *shows* things) — adaptive scripts use positive range tests only.

## Launcher-behavior round (2026-07-16, evening)

30. **Anchored popup menus** → Both the app shortcut menu and the background menu
    are now Android-style anchored popups: the Modal is top-left aligned with a
    faint scrim, and `place_popup` positions the panel next to its anchor rect
    (below if it fits, above otherwise, clamped to the window). Anchors flow
    through every path: grid icons/widgets, dock icons, drawer entries, and the
    right-click point for the background menu.

31. **iOS jiggle semantics** → Long-pressing an *item* opens its shortcut menu
    only (sliding into a drag still lifts it and starts jiggle); long-pressing
    *empty space* enters jiggle mode; in jiggle mode a stationary press-and-
    release on an item does nothing, tapping empty space (or Done) exits, and
    right-clicking an icon shows the menu without starting jiggle.

32. **Edit/management mode** → Jiggle mode shows a management bar: Done,
    ＋ Widget (opens a glass widget picker listing widget-capable apps),
    Wallpaper (cycles tints), ＋ Page, plus grid steppers.

33. **Adjustable grid** → The home grid is now per-layout state (persisted):
    columns 3–5, rows 4–8, stepped from the edit bar. Shrinking the grid clamps
    widget spans and re-places items that no longer fit (same page first, then
    later/new pages). All geometry (cells, drops, resizes, first-fit) derives
    from `LauncherLayout::grid()`.

34. **Splash argument gotcha (hard-won)** → An `&&`/`||` expression passed
    inline as a script-method argument mis-evaluates, and script `set_visible`
    treats a bad argument as `true` — which made hidden clock faces reappear.
    Adaptive widget scripts bind every boolean to a `let` local before passing
    it. Also: glass.Card tiles draw their lens overlay above pager-drawn quads,
    so the resize grip became a child widget (`LauncherGrip`) of the tile; and
    names defined in the same `script_mod!` must be referenced fully-qualified
    (`mod.widgets.X`) since `use mod.widgets.*` imports a snapshot.
