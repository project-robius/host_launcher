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

## makepad changes this project drove (all upstream now)

Hosting mutually-untrusting Splash mini-apps required extending makepad. These
started as local patches and are all merged into `dev` — the host-services
bridge and its VM fixes via makepad/makepad#1181 — so the launcher builds
against upstream with no worktree. All are behavior-preserving for existing
apps:

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
    All Apps / Get More Apps / Import App / Change Wallpaper. Both are
    gauss-lensing glass panels in a `Modal`.

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

35. **Per-app storage jail + modular persistence** → Mini-apps get an
    OS-style private data directory, sandboxed in the Splash layer. Inside an
    app "the filesystem" (`mod.fs`, shadowing the stripped real one) is rooted
    at `/`, and that root IS the app's `app_data/<id>/` dir. Containment is
    entirely host-side (makepad `widgets/src/splash_storage.rs`): lexical path
    resolution (`..`-above-root / NUL / over-deep are errors), symlink defense
    on every component, quotas (1MB/file, 16MB/jail, 256 entries, mkdir
    charged too), and a per-heap root map script can't read or retarget. The
    host assigns it via `Splash::set_sandbox_dir` before eval; a widget shares
    its app's jail; previews/validation get a throwaway one. Apps persist with
    `fs.write("/x.json", v.to_json())` + `"...".parse_json()`; `load()` runs at
    top level so a deferred handler can't clobber it.

    On disk, state is no longer one giant JSON. `layout.json` holds only
    placements/grid/dock/recents/tombstones; each user app is
    `apps/<id>/{manifest.json,app.splash,widget.splash}` (real editable files,
    written atomically at install/refine, sources first + manifest last);
    private data is `app_data/<id>/`. A corrupt app is skipped, not fatal; a
    lost `layout.json` recovers apps from their dirs. The legacy single-file
    format migrates once (all-or-nothing). App ids are sanitized at every fs
    boundary. `HOST_LAUNCHER_FRESH=1` OR `MAKEPAD=headless` redirects the whole
    data root to a per-process temp dir, so tests never touch a real profile.
    Uninstall deletes both the code dir and the data dir (killing live widget
    isolates first), matching the OS convention.

36. **Modifying apps, and undoing it** → An app can be rewritten by AI two
    ways: long-press → **✏️ Modify App…**, which prefills the create bar with
    `✏️ <Name>: ` and focuses it (the prefix is stripped on submit, and is only
    a hint — delete it and the text alone decides); or by just typing, where
    `generate::intent` classifies the request. That classifier is deliberately
    high-precision: a modification needs BOTH a named installed app AND edit
    phrasing, any "create/build/make a" wins for creation, and anything
    ambiguous falls back to creating — a surprise duplicate icon is obvious and
    harmless, a surprise rewrite is neither.

    Built-ins are modifiable too; the result is persisted as a user override
    (`builtin: true` is preserved, so it stays non-uninstallable) that shadows
    the stock manifest at load.

    Every modification — and every restore — snapshots the PREVIOUS state first
    into `apps/<id>/versions/<local-timestamp>.{splash,json}` (source first,
    metadata last, so a crash leaves an ignored orphan rather than a phantom
    version). Restore swaps one back through the same write-through path a
    modification uses (registry + user_apps + disk + force-stop + icon/tile
    rebuild). Newest 20 kept per app.

37. **App Info page** (full-screen since) → The long-press menu was becoming a junk drawer, so it
    now keeps only home-screen verbs (open, place, remove, modify) and hands
    everything *about* the app to an App Info page, the way a phone's per-app
    settings screen works: type, placement, network access, storage used (with
    Clear data), code size, version history with Restore, Force Stop, and
    Uninstall. That also removed the per-menu-open disk scan the history entry
    needed to decide its own visibility.

38. **The create bar is an expanding overlay, not a row in the column** → A
    prompt worth writing is more than one line, and generation progress is
    worth more than a spinner, but neither should shove the home screen around.
    So the home screen is two layers: the icon/widget column, and the bar
    floating above it. The bar grows down over the grid (capped at 75% of the
    screen, then scrolls); a fixed 58pt slot in the column reserves its resting
    height, and both hide together in edit mode, so the layers stay aligned
    without any positioning math. Overlay siblings both get events, so the
    pager fences out presses landing inside the bar's (growing) rect.

    Submitting turns that same space into the agent console instead of adding a
    second surface — the thing you asked for and the thing being done occupy
    one place. And it needs a Send button: makepad only submits a multi-line
    input on plain Return when `has_physical_keyboard()`, which is exactly
    right (a soft keyboard's Return must type a newline) and exactly why a
    touch-only user would otherwise be stuck.

39. **The console only grows, and keeps everything** → Two things made the
    progress panel unreadable. It was a last-6-lines window, so a run with a
    repair loop scrolled its own history into the bin; and the box was `Fit`
    around a rolling code tail, so it changed height under you while you read.
    Now the trail is never truncated (a pipeline lives one generation, so the
    log is bounded by the run), the output scrolls with wheel and drag, and
    App holds the view at its high-water mark — it grows to the 62% cap and
    stops, never shrinking. It follows the tail like a terminal until you
    scroll or press inside it, which is taken as "I'm reading" and stops the
    auto-follow for the rest of the run.

    A scrolling view can't size itself, which shapes the whole thing: give it
    `Fit` and it takes the entire cap however little it has to say. So the
    viewport's height is written from Rust each event — the content's height,
    clamped to the cap, ratcheted so it only ever rises — and the content sits
    in an inner `Fit` view that's free to overrun. Measuring the *labels*
    instead reads back the clip, and the box can never grow past its own
    starting height.

    Three self-inflicted traps on the way, all worth remembering. The height
    was first set with `script_apply_eval!`, whose scope has none of the DSL's
    imports, so `Fit`/`FitBound` resolved to nothing and the console silently
    drew at zero height (SPLASH_FINDINGS #8, hit for the second time — it now
    writes `walk.height` directly). "Scroll to the bottom" was written as
    `set_scroll_pos(f64::MAX)`, which clamps once the view has scroll bars but
    before the first draw falls through to writing `layout.scroll` raw — an
    enormous offset there doesn't mean "the end", it throws the log a million
    points off screen; the target is computed now. And reading `.rect()` off an
    area that hasn't drawn logs a mark/sweep error per call — twenty of those
    per generation slowed the test harness's snapshot queries enough to lose
    the race against a 20s window, which is what made the failure look
    intermittent. All rect reads go through a `drawn_height` guard.

    The show/hide control moved into the ✨'s slot, and the sparkle hides
    while the agent works: two glyphs that mean different things never share
    the bar, and the status row is left to the status. The control is robrix's
    `ExpandArrow` (ported into `shared/`) — an SDF triangle that rotates
    between right and down — because every text option was bad: ˄/˅ are tofu
    in these fonts, the CJK compat chevrons that do render (︿﹀) are wide and
    mismatched, and no glyph can animate between states. Since makepad's
    Button draws its own text/icon and takes no children, the button is an
    Overlay hit target with the arrow drawn over it. Re-showing restores the
    height it had, because "hide" should be reversible, not a reset.

40. **App Info fills the screen and scrolls** → It started as a centred card,
    which was wrong on both counts: it's a page you go *into*, not a popover,
    and its content (version history especially) outgrows any fixed height. It
    now fills the screen like an open mini-app, with the header pinned and
    everything below it in a scrolling body. The sides keep the app window's
    4pt hairline; top and bottom give up 28pt so a strip of dimmed backdrop
    stays tappable — tap-outside-to-dismiss only works if there IS an outside. Pinning the header is the whole point of the × living
    there: a close button that scrolls off screen is worse than none, and with
    the page covering the screen there's no longer any backdrop left to tap.

41. **Agent options are per-backend, not a fixed set** → Model, effort and
    thinking are worth exposing, but there is no cross-provider standard for
    any of them: Claude Code reads three environment variables of its own,
    octos takes `--model` and offers nothing else we can drive, and a foreign
    ACP agent is a black box. So the bar asks the ACTIVE backend what it
    supports and renders that — three controls on Claude Code, two on octos +
    anthropic, effort alone on other octos providers, none for a foreign agent
    — and names the backend underneath, since the controls mean nothing without
    it. A knob nobody reads is worse than a missing one: it looks like it
    works. Adding one is a table entry in `Backend::knobs` plus a delivery
    rule, and the delivery mechanism has to be *checked*, not assumed: octos's
    in-process provider setup only passes a model, which is what made effort
    look undeliverable, but `octos acp` reads `gateway.reasoning_effort` from
    its config and maps it per provider (OpenAI/Grok `reasoning_effort`, Gemini
    thinking budget, Anthropic thinking block). Delivering it therefore means
    writing that one field into octos's own config — invasive-looking, but it
    IS the mechanism: `--effort` exists on `octos chat`, not `acp`, and octos
    treats the value as a persistent per-turn setting.

    The values are stored as what gets sent, so a model chosen in a build that
    listed it survives one that doesn't — the control falls back to "Default"
    while the stored value is still delivered. "Default" everywhere means *say
    nothing*, which is deliberately not the same as picking the agent's default
    value: it leaves `~/.claude/settings.json` in charge.

    The controls are `glass.GlassSegmented` rather than dropdowns — the choices
    are few and ordered, so showing them inline beats hiding them behind a
    popup, and a stock dropdown looked like a system widget dropped onto the
    glass. The catch is that its labels come from the DSL (makepad exposes no
    setter), so they're duplicated in create_bar.rs and pinned by a test.

42. **The effort ladder is probed; "Deep" is prompt text** → Two follow-ons
    from #41, both about not shipping controls that quietly do nothing.

    `xhigh` is real in the API and in current Claude Code, but the runtime our
    ACP adapter bundles is older and *silently drops* an unrecognized level
    rather than erroring — the worst failure mode, since the control looks like
    it worked. So the bar reads the runtime it will actually spawn
    (`CLAUDE_CODE_EXECUTABLE`, else the SDK bundled beside `claude-code-acp` on
    PATH), looks for the level, and offers it only if present — failing closed
    when nothing is found. The row therefore declares two segmented controls
    and shows whichever matches, since GlassSegmented's labels are fixed in the
    DSL. Selection is by CONTENT, not segment count: octos's ladder is also
    four levels and has no `xhigh`, and offering it there would write a value
    octos's own enum rejects.

    A "Deep" control (plan, delegate to subagents, verify) was built and then
    removed: it only appended encouragement to the prompt, and no such setting
    exists in any agent's own interface. It read as a capability while being a
    suggestion — the same objection as an unbacked knob, one level up. If the
    bundled runtime gains the Workflow tool, the control can map to that.

    The row opens on prompt focus and closes on a press outside the bar — not
    on the prompt losing focus, which had it vanishing out from under the very
    click that was trying to use it.

    Rows are addressed by `KnobId`, never by position. A backend offering
    effort but no model (octos + openai) must still land in the effort row, or
    the effort setting renders with the model's labels — which is exactly what
    happened the first time.

43. **Two makepad traps this codebase keeps hitting** → Worth stating plainly,
    because each cost a debugging round and neither announces itself.

    *A glass surface's lens renders OVER later siblings.* It is not a normal
    z-ordered fill. This is why the home screen has to be hidden the instant a
    mini-app starts opening (not when it finishes — widgets floated on top of
    the app for the whole zoom), and why the send button's icon is layered on a
    plain `RoundedView` disc rather than a `glass.GlassButtonProminent`: on the
    glass button the icon drew correctly and was invisible. If something you
    drew is missing and the log is clean, look for glass underneath it.

    *Typed widget accessors fail silently on a type mismatch.* Changing the
    send button from `glass.GlassButtonProminent` to `ButtonFlatter` left
    `glass_button(ids!(create_send)).clicked(actions)` matching nothing — the
    button rendered perfectly and did nothing, taking every generation path
    with it. No error, no warning; a screenshot can't catch it. Changing a
    widget's type is a change to every handler that reads it, and the UI suite
    is what proves the click still lands.


44. **How does an app leave the launcher?** → **One file, two ways to move it.**
    An app is a Splash script plus a scrap of metadata, so a bundle is flat
    JSON (`.splashapp`) — small enough to paste into a chat, self-contained
    enough to hand over in a folder. Export writes the file *and* copies the
    text to the clipboard, because there's no way to know which you meant.

    Import reads that same folder. Makepad's file dialogs are stubs on macOS
    (`open_save_file_dialog` prints and returns), so a native picker wasn't an
    option — the exchange folder *is* the picker, with **Open Folder** to reveal
    it and a paste box for text that arrived some other way. It also accepts a
    bare `.splash` script: the generator's `// name:` header is already a
    manifest, so it reuses `parse_header` rather than growing a second parser.

    Two invariants on the way in: the id is re-uniqued against what's installed
    (an import can never overwrite an app you have — you get a second copy),
    and `builtin` is always cleared (an import can never mint a protected app).
    The source is compiled before it's allowed onto the home screen, which is a
    *quality* gate, not the security boundary — the isolate is that.

45. **What happens after a generation fails?** → **Retry takes Stop's place in
    the bar.** The request is still known, so re-running it beats retyping it.
    Where the backend has an unused rung on its effort ladder it raises the
    setting first — escalating to the TOP rung, not the next one up, because
    "next" isn't defined from Default (which means *the agent's own setting*,
    not a position on the ladder) and after a failure the useful second try is
    the hardest one available. The button still just says "Retry": the
    escalation is the button doing its job, not a second thing to explain, and
    a control that renames itself to narrate its internals reads as a puzzle.

    No retry for a cancel or a refusal: neither is fixed by an identical second
    run. A missing provider gets the Providers page, which is the actual fix.

46. **Viewing generated source** → `makepad_code_editor`'s `CodeView`, the same
    choice robrix makes for its event-source modal: read-only, syntax
    highlighted, no gutter. It's a popup *over* App Info rather than a page that
    replaces it — you open it from there and go back to it, so × and Escape peel
    off one layer. The source comes from the registry, not disk: a built-in has
    no file under `apps/`, and a modified app's current source is what's loaded
    rather than what's archived.

47. **Why a home widget drew ON TOP of the create-bar console** → Because glass
    z-order is a **first-come slot table**, not draw order.

    Every glass surface (`glass.Card`, `glass.Group`, … — all
    `GaussRoundedView`) opens its own overlay draw-list and registers it with
    `CxDrawList::store_sub_list`, which hands out *the first free slot* and then
    **keeps it forever**; renderers walk the table in index order, so higher
    index = painted later = on top. Draw order within a frame is irrelevant once
    slots are assigned. The whole non-glass UI is flattened into a single quad
    beneath that table, which is the already-known "a lens renders over later
    siblings" rule seen from the inside.

    So the bar loses to a widget whenever the widget's tile registered *later* —
    which is exactly what installing a generated app does (the layout changes,
    `prune_children` drops the tiles, the rebuilt tile appends past the bar's
    slot). Cold-start order is the other way round, which is why this only
    showed up after a generation, and why it looked intermittent.

    The engine does expose `DrawList2d::begin_overlay_last` (used by makepad's
    dock ghost-tab and the Linux cursor), and exposing it as an `overlay_last`
    flag on `GaussRoundedView` *did* fix the bug — but it broke the bar's own
    buttons: `glass.GlassButton`/`glass.GlassSegmented` call
    `begin_overlay_reuse` unconditionally rather than checking
    `is_drawing_overlay()` the way `GaussRoundedView` does, so they hold their
    own slots and dropped into the hole the pill vacated each frame. Stop
    disappeared. Verified by A/B on one binary, then reverted.

    The fix is host-side and can't destabilize the renderer: **the pager skips
    drawing a widget tile the create bar overlaps** (`rects_overlap` against
    `AppState::create_rect`). Widgets are background content, like app icons —
    and icons are plain SDF quads in the flattened layer, so they still paint
    correctly under the bar and are left alone. At rest (a one-line composer)
    nothing overlaps and nothing changes. `HOST_LAUNCHER_DEBUG_STATE=zorder`
    reproduces the original bug on demand: it fills the console, then re-creates
    the clock tile on a timer, which is what an install does.

48. **A generation that looks hung** → It wasn't: it was thinking, and the
    client was throwing that away. `acp_client.rs` handled `agent_message_chunk`
    and `tool_call` and dropped everything else, but the 45–75s of silence at
    the start of a run is exactly when `claude-code-acp` is emitting
    `agent_thought_chunk` (verified in the installed adapter's
    `dist/acp-agent.js`, from Claude's `thinking`/`thinking_delta` blocks).

    Now surfaced: thinking (one "💭 Thinking…" line in the trail, the text
    itself in the transcript below — kept apart from `stream` so it can never
    reach the fenced-block extractor), the agent's `plan` (Claude Code's
    TodoWrite, diffed against the last one so a republished five-step plan
    doesn't print five times), and a `Tick` for updates with no content — those
    still prove the agent is alive, and the stall watchdog measures the gap
    since the last event. Plus makepad's `LoadingSpinner` (its shader reads
    `draw_pass.time`, which the platform detects and turns into a per-frame
    repaint, so it animates with no timer to start or stop).

    No run clock. A ticking `m:ss` next to the status invited you to watch a
    number instead of the work and said nothing about progress; the spinner
    already carries "still going", and the status text changes as the run moves
    through its phases.

49. **Finishing a run** → The console keeps its output until you clear it, so
    it needs to say how. A finished run puts **New prompt** bottom-right (put
    the composer back) and, on success, **Open** top-right in Stop's slot (go
    straight into what was just built, zooming out of the bar itself rather
    than hunting for the new icon). Failure puts Retry there instead — the two
    can't both apply. A press outside only *collapses* the console, keeping the
    log and the Retry/Open offers; **New prompt** is the only thing that
    discards them.

50. **Where do API keys live, and how do you get at them?** → In octos's own
    `config.json`, all of them, and through one page.

    The old flow was a modal behind the ✨ — a sparkle you had to *know* to tap,
    which is not discoverability. Now: clicking the prompt with nothing
    configured opens the Providers page (you cannot generate anything anyway,
    so asking is strictly better than failing later), and once set up there's a
    **＋ Providers** button in the options row next to the backend name that was
    already there. The ✨ and setup-class failures land in the same place.

    Storage is octos's `env_vars` map, one entry per provider under its own
    variable name, with `provider` naming the active one. Verified in octos:
    `resolve_api_key` looks up only *that provider's* variable (plus registry
    aliases) and the map is never exported to a subprocess environment, so
    keys for other providers are inert. That makes "several configured, one
    selected" a one-field edit and means no second key store — which would
    otherwise be a second place to leak from. We write `0600`; octos writes
    with the default umask.

    One list, not two ("configured" above "available"): every row shows its own
    state, so selecting, adding and replacing are the same gesture in the same
    place. The key field is pinned above the list rather than inline — while
    you're typing a key that IS the task, and below eight provider rows its
    Save button falls off the screen (found by a UI test clicking a button that
    wasn't there).

51. **A test wrote to the developer's real config.** `octos_config_candidates`
    listed `OCTOS_CONFIG_DIR` *first* but still returned `~/.config/octos` and
    `~/.octos` behind it, and every consumer picked "the first that exists". A
    fixture pointing at an empty scratch directory therefore resolved straight
    past it to the real `~/.octos/config.json` — and the provider tests
    overwrote a live API key with test values.

    `OCTOS_CONFIG_DIR` is now authoritative: set, it is the only candidate.
    That also matches octos, which treats an explicit config dir as its own
    context and never falls back from it. The test fixture additionally asserts
    the resolved path is inside the fixture before running a body.

    The general rule this cost us: **a "first existing candidate" search is a
    footgun for anything that also writes.** Reads degrade gracefully; writes
    escape. Resolve the write target explicitly.

52. **Glass didn't obey z-order at all — the actual fix.** #47 diagnosed this
    and then worked around it twice (skip widget tiles the create bar covers;
    hide the home behind full-screen modals). Both were wrong: a widget that
    silently disappears reads as a bug, and the problem kept resurfacing
    somewhere new because the cause was untouched.

    The cause: every glass surface opens its own `DrawList2d` and registers it
    into the single window-wide `Overlay` via `CxDrawList::store_sub_list`,
    which hands out **the first free slot and keeps it for the life of the
    process**. Renderers walk that table in index order, so the paint order of
    every glass surface in the app is the order they were first *created* —
    creation order, permanently, with holes reused by whatever registers next.
    Draw order never entered into it. That is why a rebuilt widget tile, or a
    panel opened later, could land on top of anything.

    The fix is that the engine already had the hook and nothing wrote it:
    `CxDrawList::draw_item_reorder`, honoured by every backend
    (`draw_item_order_len` / `draw_item_id_at_order_index` in metal, d3d11,
    opengl, vulkan, web_gl and the headless rasteriser). So:

    - `Cx2d::overlay_seq` counts overlay sub-lists begun this frame, reset in
      `Overlay::begin`.
    - `DrawList2d::begin_overlay_inner` stamps the sub-list with its sequence
      number (`CxDrawList::overlay_order`).
    - `Overlay::end` stable-sorts the overlay's items by that stamp into
      `draw_item_reorder`.

    Glass now composites in draw order like everything else. Both workarounds
    were reverted, and the two cases that motivated them are fixed with the
    home fully drawn behind: the console draws over a widget tile that stays
    visible, and a full-screen page covers the create bar's own controls.

    Note this also fixes it for *children*: `glass.GlassButton` and
    `glass.GlassSegmented` call `begin_overlay_reuse` unconditionally rather
    than checking `is_drawing_overlay()`, so they hold their own slots — which
    is why the earlier `overlay_last`-on-one-surface experiment made the create
    bar's Stop button vanish. Ordering by draw position fixes parent-then-child
    without special cases.

53. **Segmented controls size to their labels.** `width / count` gave "Max" the
    same room as "Default", so the long word was crowded and the short one
    floated. Each segment is now measured (`DrawText::layout(..).size_in_lpxs`)
    and gets its text plus padding, with leftover width shared equally so every
    label keeps the same margin; if the labels don't fit, the padding shrinks
    (never the text) to a floor. The selection pill's x/width are computed in
    Rust and passed to the shader as uniforms, since they can no longer be
    derived from a segment count — and hit-testing became a boundary lookup
    rather than a division. The travel easing was slowed from 0.30 to 0.16:
    the pill is the only confirmation a tap registered, and it used to arrive
    before the eye could follow it.

54. **How much of a run does the console keep?** → **All of it.** It showed a
    rolling 700-byte window of the current turn, and each repair turn cleared
    that — so the reasoning, and the attempt that failed to compile, were gone
    before anyone could read them. That is precisely the output worth reading.

    `stream`/`thought` stay per-turn working buffers, because the fenced-block
    extractor consumes `stream` and it has to be empty at each boundary. A
    separate `transcript` accumulates everything and is never cleared. Thinking
    and code get a heading whenever they alternate (they arrive interleaved and
    read as gibberish run together), and each repair turn gets a marker.

    Retention had a cost the windowed version didn't: a `Label` re-lays out ALL
    of its text on every change, so painting a tens-of-KB transcript per
    streamed token ate the frame budget. A ~120ms repaint clock bought time,
    and this entry said that if it ever proved slow to lay out *at all*, the
    fix was windowing what's rendered rather than tuning the interval.

    That is what happened — see #56. The clock stayed, demoted: it no longer
    guards a layout, only the handover (clone the transcript, split it into
    lines, diff it), which is O(run) for an update nobody perceives at more
    than a few a second. The final flush still ignores it, or the last stretch
    — the part that says how the run turned out — would never be painted.

55. **How does the console size itself?** → **One jump to full height.** It
    grew a line at a time behind its own output, which left it permanently one
    line short: the text you wanted scrolled past the bottom edge, and the box
    reflowed on every chunk. Now the moment content doesn't fit, it goes
    straight to the cap — a hair above the dock.

    The cap clamps the floor back DOWN as well as up, which the ratchet
    originally didn't. It has to: the cap is derived from the dock's position,
    and that isn't known until the dock has drawn. A console filled before then
    (a run started at launch) sized itself against the fallback fraction —
    measured 620 against a real cap of 588, which put the box 2px INTO the dock
    and left it there.

56. **A console that keeps everything gets slower the more it says** →
    **Virtualize it.** Predicted in #54 and duly arrived: two `Label`s holding
    a whole run re-laid out every byte on every change, and again on every
    frame while scrolling, since a Label has no partial layout to fall back on.

    A `PortalList` pays for what's on screen — a 5,000-line run costs the same
    to scroll as a 20-line one. The run is kept as lines; only visible ones
    become widgets.

    Tailing came free with it, and that is the lesson. A hand-rolled version
    hit three ordering bugs, all the same mistake — asking the list questions
    it can't answer yet. `is_at_end` before a draw describes the PREVIOUS
    extent; `scroll_to_end` before the first draw lands past the content and
    blanks the console; `set_first_id_and_scroll` from inside draw aborts the
    process. `PortalList` already has `auto_tail`
    (`tail_range = at_end && auto_tail`, evaluated inside the draw cycle):
    follows the newest line at the bottom, stops when you scroll up, re-arms
    when you come back. **Read the widget's API before reimplementing its
    behaviour** — all three bugs were self-inflicted.

    Virtualizing costs testability: off-screen lines aren't widgets, so "is the
    whole run kept?" is answered by scrolling to a line rather than asserting
    every line at once. That is the feature, so the test scrolls.

57. **Stop is destructive and sits where Open lands** → **Confirm it.** Stop
    throws away a turn's work and the tokens are already spent — and it
    occupies the slot that Retry and Open take over moments later, so a
    mis-timed tap on a nearly-finished run destroyed it. It asks first, and
    only cancels if the run is *still* going: it can finish while the sheet is
    up, and cancelling then would tear down a console being read.

58. **Uninstalling an app you MADE destroyed it** → **The store keeps it.** A
    catalog app can always be fetched again, so the gap was invisible until you
    uninstalled something generated or imported — those exist nowhere else, and
    dropping the manifest was the only copy gone. A prompt you can't reproduce,
    deleted by a menu item.

    Uninstall archives the manifest when the catalog can't supply it, the store
    lists archived apps alongside the catalog, and Get sources from either.
    Deliberately NOT the version-history snapshots: those are per-app undo, and
    an uninstalled app has no page left to undo from.

59. **Which backend is actually running?** → **The Providers page says so.**
    The rows name the service and where its key came from; none of that
    distinguishes a child `octos acp` from an agent compiled into this binary
    from something `HOST_LAUNCHER_AGENT_CMD` chose. `pgrep` was the only honest
    way to tell, and the two behave differently enough that "why is it doing X"
    was unanswerable from inside the app.

    The line is built by `generate::runtime`, which shares its command
    construction with `start_backend` — two copies would let the page be
    confidently wrong about the one thing it exists to explain.

## Resizable app hosts + split screen (2026-08-06)

60. **How does an app learn its size?** → **`fn on_app_resize(w, h)`, the app
    twin of the widgets' `on_widget_resize`.** Splash scripts can't read their
    own rect and can't change fonts/fixed sizes at runtime, so the host tells
    them: `MiniAppScreen` caches each host's content box (0.5px epsilon),
    queues a notification when it changes during draw, and delivers it on the
    next event via `call_script_fn` — draw-time delivery would silently no-op
    against a not-yet-rebuilt widget tree, the exact trap the pager's queue
    already documents. Notifications are gated on "settled": mid-zoom/morph
    every frame differs and would burn the script's instruction budget on
    throwaway layouts; a divider drag (no anim) reflows live on purpose.

    Apps adapt with two tools: `width: Fill{max: N}` + parent `align` caps and
    centers the column on wide hosts with zero script work, and pre-declared
    tier Views toggled by the hook (`set_visible`/`set_text`) cover narrow and
    short panes. `HOST_LAUNCHER_DEBUG_STATE=validate` compile-checks every
    installed + catalog app's Splash source and exits — the only way to
    "build" `.splash` edits from the command line.

61. **Split screen behaves like Android (OnePlus Open flavor).** One mode
    machine in `MiniAppScreen` — `Hidden | Single | Pick | Split` with an
    axis (chosen from window aspect, rotatable) and a divider ratio. Entry is
    Android's: the header split button (or the context menu's "Split Screen")
    docks the app to one pane and leaves the home screen LIVE in the rest of
    the window to pick the partner; every open funnel (icons, dock, drawer,
    search) then routes into the free pane. The divider drags continuously
    (both panes resize live, min pane 140), magnets onto 1/3 · 1/2 · 2/3, and
    released near an edge closes the smaller side and fullscreens the other;
    tapping it opens a Swap / Rotate / Full menu. Closing a pane (×, back,
    force stop, uninstall) leaves the survivor fullscreen; a window too small
    for two panes exits the split rather than showing two slivers.

    Pick mode is the one state where home is interactive UNDER a shown app, so
    it publishes `split_block_rect` through `AppState` and the pager, dock and
    `DrawerItem` cells (drawer + search) refuse presses inside it — overlay
    siblings all see every event (#38), and without the fence a tap on the
    docked pane also activated whatever it covered. The create bar hides for
    the same reason (`composer_suppressed`), and edit mode is refused while
    picking. Split state is deliberately not persisted (Android drops it too).

    Two hit-system lessons, learned the hard way: `event.hits()` CAPTURES the
    digit on FingerDown even when the caller ignores the result, so the
    divider must not hit-test presses that land on its own open menu (the
    menu's buttons would never click); and widgets inserted into the tree
    during DRAW never reach the snapshot tests select against, so the split
    chrome is instantiated at event time and selected by an inner `:=` id
    (`divider_pill`).

62. **Widget tiles never draw while a mini-app pane exists.** A glass tile
    (`glass.Card` → `GaussRoundedView`) renders its ENTIRE subtree into its
    own overlay draw list so the glass can sample the scene; overlays
    composite after the whole main pass (sorted by per-frame begin order), so
    a visible tile floats in front of an app pane no matter the widget-tree
    order — the weather widget literally sat on top of a docked app in pick
    mode. Rather than restructure the pane into overlay drawing (which would
    put it above modals and every later main-pass sibling), the launcher
    extends the existing rule "widgets are background content, never in front
    of an open app" to every pane state: `AppState.hide_widget_tiles` is true
    whenever `mini_app_screen.is_showing()` (fullscreen, split, pick, and all
    animations), and the pager skips drawing tiles, forwarding them finger
    events, and hit-testing their cells. Flips pair with `redraw_all()` — a
    tile that stops drawing leaves a stale overlay draw list until a full
    pass flushes it. App icons render inline (cheap SDF plates, no overlay),
    so they stay visible and pickable, which is exactly what pick mode needs.

63. **The split icon shows the divider you'll actually get.** `best_axis()`
    stacks panes top/bottom in a tall window and side-by-side in a wide one,
    so a static two-panes-side-by-side glyph lies on a portrait phone (and
    the original filled-bars version read as a pause button anyway). The
    header button's SDF takes a `horizontal` uniform — synced per event from
    the live axis while split (Rotate flips it) or from the container aspect
    otherwise — and the context menu carries two MenuButtons with opposite
    icons (`split.svg` / `split_h.svg`), showing whichever matches the
    window, because swapping `crate_resource(...)` through a runtime eval
    would hit the no-DSL-uses-in-eval-scope trap (SPLASH_FINDINGS #8).
    Macro footgun found on the way, fixed upstream: `script_apply_eval!`'s
    generated values-block used to bind `let mut v`, so interpolating a
    caller variable named `v` via `#(v)` captured the macro's own Vec.

64. **Pick mode parks the app as an offscreen sliver, and yields to the
    drawer.** Docking the waiting app into half the screen made picking
    cramped: half the grid sat under the pane. Now the app keeps its
    fullscreen SIZE (no resize, so its layout doesn't churn — and no
    on_app_resize fires) and slides along the split axis until only
    PICK_PEEK points peek in at the pane-A edge; the whole home screen is
    free for picking, and tapping the sliver cancels (its header, split
    button included, hangs offscreen). The input fence shrinks to the
    sliver's on-screen intersection. The drawer/search overlay draw BELOW
    the mini-app screen by design (an app opened from the drawer must zoom
    up on top of the closing drawer), so "drawer in front while picking" is
    implemented by yielding instead of reordering: while either is open
    during a pick, the sliver stops drawing and fencing entirely — the
    covering layer is effectively frontmost, every app in it is pickable,
    and the chosen one zooms up on top exactly like a normal drawer open.

65. **Zooms play over the real launcher, and snap once icon-sized.** Hiding
    home during open/close zooms (the old glass-artifact workaround) made a
    closing app shrink into a black void, with home popping in afterwards.
    Now the zooming host draws into its own overlay draw list — begun after
    the pager's tile overlays each frame, so it composites ABOVE their glass
    — which lets the full home screen, widget tiles included, sit behind the
    animation the whole way (`is_zooming()` gates both the home-hide and the
    tile-hide). In-place split animations are not zooms and keep home hidden.
    And a closing zoom finishes early: once the shrinking window is within
    16pt of the icon's size it is visually gone, so the tail of the glide
    (which read as lag) snaps to done.

65. **An app icon is just an app at 1x1; grow it and the app runs there.**
    Now that mini-apps reflow to any size, the line between icon, widget and
    running app is only a span. `PlacedKind::App` carries `cols`/`rows`
    (serde-defaulted to 1 so old layouts load): at 1x1 it draws the familiar
    icon+label, and at ANY larger span the pager hosts the real app live in
    those cells, with a compact title bar (name, ⤢ expand, shrink). The
    resize plumbing that already existed for widgets was made kind-agnostic
    rather than duplicated — long-pressing an icon arms the same Android
    resize frame, and dragging it back to 1x1 turns the app off again.

    The load-bearing decision is that expanding does NOT start a second
    instance. `mod.widgets.AppHost` moved into shared styles so the home grid
    and the app screen instantiate the SAME template with two chromes
    (`header` fullscreen, `tile_bar` in a cell), and expanding LENDS the
    running widget: the pager keeps its reference (so the isolate can never
    be dropped by the borrower), the app screen adopts it and zooms it up
    mid-state, and the cells hold the spot with a "Running full screen"
    stand-in offering Bring back. Closing the app returns it to its cells
    still running. Tapping an icon whose app already runs on home expands
    that tile for the same reason: one app, one isolate, several
    presentations.

    Only the title bar is a grab handle — a press anywhere else is the app's,
    which is why the pager takes no gesture at all on the body. Teardown had
    to grow with it: force stop, uninstall, clear-data and AI rewrites all
    drop live home hosts alongside widget tiles, or their isolates would tick
    on against a jail that may no longer exist.

    Where the stand-in actually earns its keep is split screen: pick mode
    puts the home screen back on show while the app keeps running as a
    pane, so its old cells need to say where it went and offer the way
    back. Two things follow. The stand-in is checked BEFORE the
    hide-widget-tiles rule (it is a plain card with no glass overlay, and
    that state is exactly when it should draw), and its tap is handled
    before that gate too — otherwise the card renders in the one case it
    exists for and ignores every press. Handing a pane back to its tile
    also has to fix up a split: the other pane goes fullscreen rather
    than being left pointing at a host that just left.

## Mini-app permissions + host services (2026-08-14)

66. **Deny-by-default hybrid permission model** (full design: docs/PERMISSIONS.md).
    Containers give the baseline (an isolate starts with NOTHING: fs jailed,
    run/res/cx.quit gone, no net runtime), Android gives declaration (a
    manifest `permissions` list; undeclared = ungrantable, no prompt ever),
    iOS gives the runtime prompt (first use asks, the answer persists, the
    user can flip any grant in App Info at any time). Chosen over any single
    model because each covers a hole in the others: pure prompting nags,
    pure declaration over-grants, pure sandboxing does nothing.

67. **One generic bridge in makepad, all policy in the launcher.** makepad's
    `splash_host.rs` (upstream via makepad/makepad#1181) only queues
    `host.request(...)` calls with a HOST-ASSIGNED app tag and calls the
    callback with whatever answer arrives; the launcher's broker
    (src/services/) owns the permission checks and the robius platform work.
    Rejected: teaching makepad about permissions (couples an engine to one
    launcher's policy), and per-service script modules (N doorways to audit
    instead of one).

68. **`network` is enforced at the VM, not brokered, and grant changes
    restart the app.** The net runtime is baked in at isolate alloc, so the
    launch prompt fires as a declared-network app opens (it opens NETLESS
    immediately, prompt floats above, Allow restarts it connected — Android's
    revoke-kills-the-process semantics, in both directions). Rejected:
    proxying all HTTP through the broker (slower, duplicates a stack makepad
    already has) and deferring the app open until the prompt is answered
    (punishes the common case).

69. **Grants are launcher state, never app state.** permissions.json lives
    beside layout.json (atomic writes, corruption costs re-asking, never the
    home screen); an app's own jail can't reach it; exported bundles carry
    DECLARATIONS only. Uninstall deletes grants — reinstall re-asks.

70. **IPC consent is asymmetric.** Sending is the privileged act (runtime
    prompt, sender side); receiving is opted into by declaring `ipc` +
    defining `on_ipc_message`, and remains user-blockable by denying the
    receiver's `ipc`. Same-app messaging (`to: "self"`, app <-> its widget)
    is permission-free: one app, one sandbox. Requiring a grant on BOTH ends
    was tried first and made the receiver's grant a prompt nobody ever sees
    (receivers don't request, so App Info was the only path to a working
    pair).

71. **Tests never touch the internet.** Headless/fresh runs auto-start a
    tiny local HTTP server (src/services/fake_net.rs) with deterministic
    geo/weather/news/timezone bodies, and the permission-free `env` service
    is the ONLY place apps get endpoint URLs — so pointing the whole fleet
    at 127.0.0.1 needs no app changes, and the real script->net->platform->
    isolate response path still gets exercised end to end. CoreLocation is
    skipped in these runs (its delegate needs the NSRunLoop the headless
    harness never pumps); the IP-geolocation fallback is the tested path.

72. **Validation now fails on parse errors** (makepad fix, same branch).
    Three freshly-generated apps shipped dangling-`else` parse errors
    straight through `validate` — the parser recovers, logs, and produces a
    runnable module, and nothing reached the captured-error sink. Parser
    errors are now recorded on the parser and drained into
    `captured_errors` by both eval paths.

## Abuse control for hostile mini-apps (2026-08-19)

73. **Containment and availability are separate problems, and only the first
    was solved.** An isolate already cannot escape: fs jailed, run/res/cx.quit
    gone, `app_tag`/`may_prompt` host-assigned, grants stored outside the
    app's reach. None of that stops an app from *asking* forever, and every
    refusal still costs the launcher work. So the bridge now prices requests
    (per-app token bucket, cost by what the service actually does) instead of
    only judging them.

74. **The escalation ends in a stop, because nothing else terminates.**
    Refuse → cooldown → strike → stop. An app that spends four cooldowns
    ignoring the limit is not going to be talked round, and "refuse forever"
    is a state the launcher pays for indefinitely. Force stop, mark it
    restricted, tell the user what happened.

75. **A restriction is persisted and only the user lifts it.** Otherwise
    relaunching is a free reset and the escalation means nothing. It is NOT
    cleared by "reset all permissions" either: that is about grants, and
    quietly freeing a stopped app as a side effect of tidying up is the kind
    of surprise a security control cannot afford. Strikes and budgets, by
    contrast, belong to a run and are dropped on every stop.

76. **Restriction is enforced at `effective()`, not at each call site.** Every
    capability decision already funnels through it, so a restricted app
    reports zero capabilities everywhere — App Info, `host.capabilities()`,
    the manager — without twelve separate checks that could each be forgotten.
    Launch paths (`open_app`, home tiles) check `is_restricted` directly,
    since "may it run at all" is not a capability question.

77. **OS dialogs are foreground-only and one-at-a-time.** A home-screen widget
    raising a file picker is never legitimate, and stacked pickers are a way
    to trap the user. The in-flight flag is set where the dialog is actually
    raised, not at the permission check — a request parked behind a prompt has
    not opened anything, and flagging it there made the post-grant retry
    refuse itself.

78. **The probe app misbehaves on purpose.** `sandbox_probe` gets a "Flood the
    host with requests" button that fires 80 requests and reports how many
    came back refused. A defense nobody can see is a defense nobody trusts,
    and one burst is a single strike out of four, so the demo cannot restrict
    the app that runs it.

79. **A condemned app's remaining requests are dropped, not refused.** Every
    answer re-enters the isolate synchronously, and the app is being torn down
    in that same event pass; a script that answers by touching its UI leaves
    paused threads and queued widget calls behind it. Dropping is what the
    bridge already does with a request nobody drains. This turned out to
    matter more than tidiness: before it, force-stopping a flooding app
    panicked the launcher in makepad's script GC, because a dead isolate's
    widgets were still being routed into the app VM (fixed upstream — a
    reclaimed heap is now told apart from the app VM's and its calls dropped).
    The launcher keeps its own drop anyway, so the kill path is safe on a
    makepad that predates that fix.

## Per-isolate resource limits (2026-08-20)

80. **makepad only bounded ONE ENTRY, so nothing bounded an app.** The VM
    applies 64ms and 200k instructions per entry into script — but a script
    that arranges to be entered often got a fresh full allowance every time.
    Ten timers is ten times the budget, and nothing capped timers. The fix is
    a cumulative allowance per isolate per window, on top of the per-entry
    one: an entry gets what is LEFT of the window rather than a fresh slice.

81. **Mechanism in makepad, policy in the launcher.** Only the VM can meter
    its own execution, and only the launcher knows that this app is a tile and
    that one is on screen. `SplashLimits` is a plain struct of numbers the
    host sets per isolate; `src/resources.rs` decides what those numbers are.
    Same split as the host-services bridge, for the same reason.

82. **Numbers, not levels.** A "strict/relaxed" switch cannot express "this
    app may use half the processor but hold four timers", which is exactly the
    kind of thing a user wants after an app misbehaves in one specific way.
    Every resource is an exact amount, chosen from presets — presets rather
    than a text field because these are real units with real consequences, and
    a mistyped one makes an app look broken with no way to tell why.

83. **A refused timer returns nil rather than raising.** The embedder that set
    the cap already knows it was hit — it is what refused it — and a script
    that asks for one timer too many should be able to check and cope instead
    of dying mid-function. The same reasoning as `r.is_ok` on brokered calls.

84. **A crossing is coalesced per app per kind per pass.** Found by the probe
    test: asking for thirty timers past the cap produced thirty refusal
    events, thirty strikes, and an instant stop for what is one greedy loop.
    An app crossed a limit or it didn't; how many times it repeated the ask
    inside one pass is not extra evidence.

85. **Timer bugs found on the way, all fixed upstream.**
    `std.stop_timer` removed its bookkeeping entry but never stopped the OS
    timer, so a stopped timer kept firing for the life of the process and was
    invisible to every teardown path (they all filter that same list). It also
    matched by id alone, with no ownership check. And `start_interval(-1.0)`
    reached `Duration::from_secs_f64` on several backends, which panics — one
    statement from a mini-app took the whole process down.

## Resource limits become shares (2026-08-21)

86. **A fixed per-app quota is a tax with no beneficiary.** The first version
    gave each app 25% of each second whether or not anything else wanted the
    time. One mini-app on an idle machine has nobody to be fair to, and the
    frame it gave up went nowhere. Replaced with the cgroup v2 split: a
    WEIGHT that decides who yields when apps actually compete and does
    nothing when they don't, plus MAX ceilings that ship off.

87. **One weight for every contended resource, not one per resource.** An app
    is important or it isn't. Asking a user to rank it separately for
    processor, memory and downloads is asking them to invent numbers they
    have no basis for.

88. **Space-multiplexed resources use the same rule as CPU.** Memory, timers
    and in-flight requests are pools, so "is the pool full AND is this
    isolate over its slice" is one function every resource calls. Only the
    units differ. Storage is the deliberate exception: disk is not handed
    back when pressure passes, so a share of it would be a share of something
    nobody returns — it stays a quota.

89. **Memory gets pressure before a verdict.** Over its share of a full
    system, an isolate is collected harder (cgroup `memory.high`) and only
    stopped after three collections that fail to bring it back down
    (`memory.max`). An app that frees what it grabbed never reaches the stop.

90. **A trimmed entry stays weighted.** Trimming everyone to the same sliver
    threw the weights away exactly where they mattered most — the moment apps
    compete. A throttled slice is now the app's fraction of a full one, with
    a floor so no entry is cut so small it bails part-way through its own
    work.

91. **The per-app memory backstop must not sit below the pool.** Found by a
    test: an 8M backstop under a 24M pool meant a lone app hit its own
    ceiling long before it could use memory nobody wanted, which quietly made
    the sharing unreachable. The backstop is the pool size.

92. **"Fastest timer" was an arbitrary rule, and it is gone.** A floor on how
    often an app may tick applied on a completely idle system, varied by
    surface for no defensible reason (16ms foreground, 100ms tile), and
    silently changed an app's behaviour without telling it. What an app pays
    now is the WAKEUP: every timer fire is charged to its processor share,
    because a fire costs a dispatch pass whether or not the callback does
    anything. Fast timers are therefore expensive exactly when the machine is
    busy and free when it is not. What survives at creation is a validity
    clamp — negative, NaN and infinite intervals panic several platform
    backends — which is a crash fix, not a policy.

93. **The launcher does not get a reserved slice either.** The first cut gave
    mini-apps 800ms of each second and kept 200ms back, which is the same
    arbitrary-quota mistake one level up: a reservation is a tax whenever the
    launcher has nothing to draw. Contention is now MEASURED — is the launcher
    missing its frame? — and while frames land on time, nothing is limited at
    all. When they slip, apps are squeezed by weight between them and by an
    adaptive pressure that deepens while frames keep slipping and relaxes when
    they recover. The launcher is prioritised rather than pooled, because it
    draws every app's pixels: if it cannot paint, nothing else on screen
    matters.

94. **The memory pool is the host's to size.** How much memory there is to
    share is something the embedder knows and makepad does not, so
    `set_memory_pool` exists and the built-in number is documented as a guess.

95. **Missed frames alone are not evidence against the apps.** The first cut
    of the measured signal trimmed apps whenever the launcher missed its
    frame, which meant a software-rasterised or otherwise slow host squeezed
    every app permanently — found the hard way, when no mini-app button
    handler could finish under the headless test renderer. Contention now
    requires the apps to be using a meaningful share of the window as well.
    A machine can be slow for reasons trimming an app cannot fix.

96. **A saved copy of a built-in never overrides what that built-in declares.**
    Modifying a built-in writes a copy under `apps/<id>/`, and that copy
    shadows the code. Copies written before the permission model existed carry
    no `permissions` at all, which silently stripped Weather and News of
    network: the apps fell back to their offline content, App Info showed
    nothing to switch on, and there was no way back because a built-in's
    declarations are deliberately not user-editable. The loader unions the
    built-in's declarations back in — a union rather than a replacement,
    because a refine may legitimately have ADDED capabilities and those are
    the user's to keep. A user's own app is left alone: declaring nothing is a
    legitimate thing for it to do, and it has an "Add a capability" row.
