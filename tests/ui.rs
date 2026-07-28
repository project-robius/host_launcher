//! Headless UI tests for the launcher, driven through makepad_test.
//!
//! Run with:
//! ```sh
//! HOST_LAUNCHER_FRESH=1 cargo test --test ui -- --test-threads=1
//! ```
//! `HOST_LAUNCHER_FRESH=1` makes every app instance start from the default
//! home layout and skip persistence, so tests are order-independent and don't
//! touch the developer's real launcher state.

use makepad_test::{makepad_test, RemoteKeyModifiers, RemoteMouseDown, RemoteMouseMove, RemoteMouseUp, Selector, StudioToApp, TestApp};

/// The home screen comes up with its seeded icons, and the clock widget's
/// Splash isolate ticks (its label goes from the placeholder to a real time).
#[makepad_test]
fn home_screen_smoke(app: TestApp) {
    app.locator(Selector::id("name").text_exact("Calculator"))
        .wait_visible();
    app.locator(Selector::id("name").text_exact("News"))
        .wait_visible();
    // The clock widget runs `std.start_interval` inside its own Splash VM;
    // a ":" in the label proves the isolate timer fired and `ui.*` resolved.
    app.locator(Selector::id("w_time_sm").text_contains(":"))
        .wait_visible();
    // The weather widget is a second, independent Splash isolate.
    app.locator(Selector::all().text_exact("San Francisco"))
        .wait_visible();
}

/// Tapping an app icon opens its mini-app fullscreen; the header close button
/// returns to the home screen.
#[makepad_test]
fn open_and_close_mini_app(app: TestApp) {
    app.locator(Selector::id("name").text_exact("Calculator"))
        .wait_visible()
        .click();
    // The calculator's Splash content is live once its display label appears.
    app.locator(Selector::id("display").text_exact("0"))
        .wait_visible();
    app.locator(Selector::id("back_button")).wait_visible().click();
    app.locator(Selector::id("display")).wait_hidden();
}

/// The calculator mini-app actually computes: 7 × 8 = 56, entirely inside
/// its isolated Splash VM.
#[makepad_test]
fn calculator_computes(app: TestApp) {
    app.locator(Selector::id("name").text_exact("Calculator"))
        .wait_visible()
        .click();
    app.locator(Selector::id("display").text_exact("0"))
        .wait_visible();
    app.locator(Selector::widget_type("GlassButton").text_exact("7"))
        .wait_visible()
        .click();
    app.locator(Selector::id("display")).wait_text("7");
    // The header close button is also a GlassButton with "×" and is drawn first
    // (header before content), so target the calculator's multiply key at nth(1).
    app.locator(Selector::widget_type("GlassButton").text_exact("×").nth(1))
        .wait_visible()
        .click();
    app.locator(Selector::widget_type("GlassButton").text_exact("8"))
        .wait_visible()
        .click();
    app.locator(Selector::widget_type("GlassButton").text_exact("="))
        .wait_visible()
        .click();
    app.locator(Selector::id("display")).wait_text("56");
}

/// Swiping the pager horizontally flips to the second page (where the
/// user-installed sample apps live), and swiping back returns.
#[makepad_test]
fn swipe_between_pages(app: TestApp) {
    app.locator(Selector::id("name").text_exact("Counter"))
        .wait_hidden();
    app.locator(Selector::id("home_pager"))
        .wait_visible()
        .drag_by(-300.0, 0.0);
    app.locator(Selector::id("name").text_exact("Counter"))
        .wait_visible();
    app.locator(Selector::id("home_pager")).drag_by(300.0, 0.0);
    app.locator(Selector::id("name").text_exact("Counter"))
        .wait_hidden();
}

/// Swiping up opens the app drawer listing every installed app; the sort
/// button toggles between alphabetical and recents ordering.
#[makepad_test]
fn app_drawer_opens_and_sorts(app: TestApp) {
    app.locator(Selector::id("home_pager"))
        .wait_visible()
        .drag_by(0.0, -250.0);
    app.locator(Selector::id("sort_button"))
        .wait_visible()
        .wait_text("A–Z");
    // Every installed app appears in the drawer, including ones not on page 1.
    app.locator(Selector::id("d_name").text_exact("Stopwatch"))
        .wait_visible();
    app.locator(Selector::id("sort_button")).click();
    app.locator(Selector::id("sort_button")).wait_text("Recent");
}

/// Opening an app from the drawer works and records it as most recent.
#[makepad_test]
fn open_app_from_drawer(app: TestApp) {
    app.locator(Selector::id("home_pager"))
        .wait_visible()
        .drag_by(0.0, -250.0);
    app.locator(Selector::id("d_name").text_exact("Notes"))
        .wait_visible()
        .click();
    app.locator(Selector::id("editor")).wait_visible();
    app.locator(Selector::id("back_button")).wait_visible().click();
    app.locator(Selector::id("editor")).wait_hidden();
}

/// A mini-app keeps its state while "closed" (iOS-style backgrounding):
/// reopening the counter app shows the previously incremented value.
#[makepad_test]
fn mini_app_keeps_state_when_closed(app: TestApp) {
    app.locator(Selector::id("home_pager"))
        .wait_visible()
        .drag_by(-300.0, 0.0);
    app.locator(Selector::id("name").text_exact("Counter"))
        .wait_visible()
        .click();
    app.locator(Selector::id("display").text_exact("0")).wait_visible();
    app.locator(Selector::widget_type("GlassButton").text_exact("+"))
        .wait_visible()
        .click()
        .click()
        .click();
    app.locator(Selector::id("display")).wait_text("3");
    app.locator(Selector::id("back_button")).wait_visible().click();
    app.locator(Selector::id("display")).wait_hidden();
    // Reopen: the isolate was kept alive, so the count survives.
    app.locator(Selector::id("name").text_exact("Counter"))
        .wait_visible()
        .click();
    app.locator(Selector::id("display").text_exact("3")).wait_visible();
}

/// The to-do mini-app supports adding a task through its text input. To-Do is a
/// dock favorite (label-less), so it's opened from the drawer here.
#[makepad_test]
fn todo_add_task(app: TestApp) {
    app.locator(Selector::id("home_pager"))
        .wait_visible()
        .drag_by(0.0, -250.0);
    app.locator(Selector::id("d_name").text_exact("To-Do"))
        .wait_visible()
        .click();
    app.locator(Selector::id("task_input"))
        .wait_visible()
        .fill("Write more tests");
    app.locator(Selector::widget_type("GlassButton").text_exact("Add"))
        .wait_visible()
        .click();
    app.locator(Selector::all().text_contains("Write more tests"))
        .wait_visible();
}

/// Long-pressing a home icon (press, hold past the 0.5s threshold, release in
/// place) opens its context menu with the expected actions.
#[makepad_test]
fn long_press_opens_context_menu(app: TestApp) {
    app.locator(Selector::id("name").text_exact("Calculator")).wait_visible();
    let snap = app.locator(Selector::id("name").text_exact("Calculator")).snapshot();
    // Press on the icon tile, which sits just above the name label.
    let (x, y) = (snap.x as f64 + snap.width as f64 / 2.0, snap.y as f64 - 24.0);
    let down = RemoteMouseDown { button_raw_bits: 1, x, y, time: 0.0, modifiers: RemoteKeyModifiers::default() };
    let up = RemoteMouseUp { button_raw_bits: 1, x, y, time: 0.0, modifiers: RemoteKeyModifiers::default() };
    app.forward(vec![StudioToApp::MouseDown(down)]);
    // Keep the app pumping past the 0.5s long-press threshold (a bare sleep
    // wouldn't advance the headless app's timers).
    for _ in 0 .. 9 {
        let _ = app.widget_snapshot();
        std::thread::sleep(std::time::Duration::from_millis(90));
    }
    app.forward(vec![StudioToApp::MouseUp(up)]);
    // The menu (inside a Modal) lists the app's actions.
    app.locator(Selector::all().text_contains("App Info")).wait_visible();
    app.locator(Selector::all().text_exact("Remove from Home")).wait_visible();
}

/// Long-pressing empty home-screen space turns on edit mode, which reveals the
/// remove badges on every icon.
#[makepad_test]
fn context_menu_enters_edit_mode(app: TestApp) {
    enter_edit_mode(&app);
    // Edit mode reveals a remove badge on every icon (SDF-drawn, id "badge").
    app.locator(Selector::id("badge")).wait_visible();
}

/// Interactive widgets: a tap on a widget's own in-VM button updates it IN
/// PLACE, without opening the app. Adds the Counter widget, leaves edit mode,
/// then taps "+" and watches the count go 0 -> 1 inside the tile's Splash.
#[makepad_test]
fn interactive_widget_button_updates_in_place(app: TestApp) {
    // Enter edit mode via the News icon's context menu.
    enter_edit_mode(&app);
    app.locator(Selector::id("badge")).wait_visible();
    for _ in 0 .. 18 {
        let _ = app.widget_snapshot();
        std::thread::sleep(std::time::Duration::from_millis(40));
    }

    // Add the Counter widget via the gallery: open it, pick Counter (shows the
    // live preview + sizes), then Add at the default size.
    app.locator(Selector::all().text_exact("＋ Widget")).wait_visible().click();
    app.locator(Selector::all().text_contains("Counter")).wait_visible().click();
    app.locator(Selector::all().text_contains("Add")).wait_visible().click();

    // Leave edit mode by tapping an empty cell. first_fit lands the 2x2 counter at
    // cols 2-3 / rows 4-5 (bottom-right) in the default page, so the bottom-LEFT
    // cell (col 0, row 5) is clear.
    let pager = app.locator(Selector::id("home_pager")).snapshot();
    let (ex, ey) = (
        pager.x as f64 + pager.width as f64 * 0.12,
        pager.y as f64 + pager.height as f64 * 0.9,
    );
    app.forward(vec![
        StudioToApp::MouseDown(RemoteMouseDown {
            button_raw_bits: 1, x: ex, y: ey, time: 0.0, modifiers: RemoteKeyModifiers::default(),
        }),
        StudioToApp::MouseUp(RemoteMouseUp {
            button_raw_bits: 1, x: ex, y: ey, time: 0.0, modifiers: RemoteKeyModifiers::default(),
        }),
    ]);
    app.locator(Selector::id("badge")).wait_hidden();

    // The counter shows 0 and no 1; tapping its in-VM "+" bumps it to 1 in place.
    app.locator(Selector::all().text_exact("0")).wait_visible();
    app.locator(Selector::all().text_exact("1")).wait_hidden();
    app.locator(Selector::all().text_exact("+")).wait_visible().click();
    app.locator(Selector::all().text_exact("1")).wait_visible();
}

/// Submit the create bar. Plain Return only submits when a physical keyboard is
/// attached — makepad lets a soft keyboard's Return type a newline in the now
/// multi-line prompt instead — and the headless harness reports none. So tests
/// press Send, exactly like a phone user has to.
fn submit_prompt(app: &TestApp) {
    app.locator(Selector::id("create_send")).wait_visible().click();
}

/// A primary-button tap (down+up) at an absolute point.
fn tap(app: &TestApp, x: f64, y: f64) {
    app.forward(vec![
        StudioToApp::MouseDown(RemoteMouseDown {
            button_raw_bits: 1, x, y, time: 0.0, modifiers: RemoteKeyModifiers::default(),
        }),
        StudioToApp::MouseUp(RemoteMouseUp {
            button_raw_bits: 1, x, y, time: 0.0, modifiers: RemoteKeyModifiers::default(),
        }),
    ]);
}

/// Regression: the resize indicator (and the widget's frozen interactivity) must
/// not get stuck after the widget's context menu is dismissed by tapping outside
/// it — the makepad Modal closes on such a tap without routing through the app's
/// close path, so the pager reconciles the leaked resize_hint itself.
#[makepad_test]
fn widget_menu_outside_tap_unfreezes_widget(app: TestApp) {
    enter_edit_mode(&app);
    // Add the Counter widget (2x2 -> bottom-right of the default page).
    app.locator(Selector::all().text_exact("＋ Widget")).wait_visible().click();
    app.locator(Selector::all().text_contains("Counter")).wait_visible().click();
    app.locator(Selector::all().text_contains("Add")).wait_visible().click();
    // Leave edit mode (bottom-left cell is empty).
    let pager = app.locator(Selector::id("home_pager")).snapshot();
    let (px, py) = (pager.x as f64, pager.y as f64);
    let (pw, ph) = (pager.width as f64, pager.height as f64);
    tap(&app, px + pw * 0.12, py + ph * 0.9);
    app.locator(Selector::id("badge")).wait_hidden();

    // Right-click the counter widget's upper area (cols 2-3, row 4 — its caption/
    // value, not the buttons) to open its context menu, which freezes the widget.
    let (wx, wy) = (px + pw * 0.75, py + ph * 0.74);
    app.forward(vec![
        StudioToApp::MouseDown(RemoteMouseDown {
            button_raw_bits: 2, x: wx, y: wy, time: 0.0, modifiers: RemoteKeyModifiers::default(),
        }),
        StudioToApp::MouseUp(RemoteMouseUp {
            button_raw_bits: 2, x: wx, y: wy, time: 0.0, modifiers: RemoteKeyModifiers::default(),
        }),
    ]);
    app.locator(Selector::all().text_exact("Remove Widget")).wait_visible();
    // Dismiss by tapping empty space far from the menu (top-left).
    tap(&app, px + pw * 0.12, py + ph * 0.08);
    app.locator(Selector::all().text_exact("Remove Widget")).wait_hidden();

    // The widget is live again (resize_hint cleared): tapping "+" counts 0 -> 1.
    app.locator(Selector::all().text_exact("1")).wait_hidden();
    app.locator(Selector::all().text_exact("+")).wait_visible().click();
    app.locator(Selector::all().text_exact("1")).wait_visible();
}

/// Dock icon geometry: the x of dock slot `i`'s left edge, and the icons' top y.
/// Mirrors the dock's own layout maths (5 favourites, 56pt icons, inset ends).
fn dock_slot(app: &TestApp, i: f64) -> (f64, f64) {
    let dock = app.locator(Selector::id("dock")).snapshot();
    let (dx, dy) = (dock.x as f64, dock.y as f64);
    let (dw, dh) = (dock.width as f64, dock.height as f64);
    let icon = 56.0_f64;
    let bar_h = icon + 32.0;
    let bar_y = dy + (dh - bar_h) * 0.5;
    let icon_y = bar_y + (bar_h - icon) * 0.5;
    let edge = 30.0_f64.min(dw * 0.08);
    let n = 5.0_f64;
    let gap = ((dw - 2.0 * edge) - n * icon) / (n - 1.0);
    (dx + edge + i * (icon + gap), icon_y)
}

/// Presses at `from` and slides to `to` in steps, leaving the finger DOWN so the
/// drag's in-flight state (drop outlines, opened slots) can be inspected.
fn drag_hold(app: &TestApp, from: (f64, f64), to: (f64, f64)) {
    let mut msgs = vec![StudioToApp::MouseDown(RemoteMouseDown {
        button_raw_bits: 1, x: from.0, y: from.1, time: 0.0,
        modifiers: RemoteKeyModifiers::default(),
    })];
    for i in 1 ..= 10 {
        let f = i as f64 / 10.0;
        msgs.push(StudioToApp::MouseMove(RemoteMouseMove {
            time: 0.0,
            x: from.0 + (to.0 - from.0) * f,
            y: from.1 + (to.1 - from.1) * f,
            modifiers: RemoteKeyModifiers::default(),
        }));
    }
    app.forward(msgs);
}

/// Releases a held drag at `to`.
fn drag_release(app: &TestApp, to: (f64, f64)) {
    app.forward(vec![StudioToApp::MouseUp(RemoteMouseUp {
        button_raw_bits: 1, x: to.0, y: to.1, time: 0.0,
        modifiers: RemoteKeyModifiers::default(),
    })]);
}

/// Presses at `from`, slides to `to` in steps, and releases — a drag.
fn drag(app: &TestApp, from: (f64, f64), to: (f64, f64)) {
    drag_hold(app, from, to);
    drag_release(app, to);
}

/// In jiggle mode the dock behaves like the rest of the home screen: each
/// favourite carries the same "×" badge (confirm-then-remove), and a home icon
/// can be dragged into the freed slot — where, being a dock icon, it loses its
/// grid name label.
#[makepad_test]
fn dock_badge_removes_and_accepts_dropped_icon(app: TestApp) {
    enter_edit_mode(&app);

    // Tap the first favourite's "×" (Weather) and confirm.
    let (x0, icon_y) = dock_slot(&app, 0.0);
    tap(&app, x0, icon_y);
    app.locator(Selector::all().text_contains("from the dock?")).wait_visible();
    app.locator(Selector::all().text_exact("Remove")).wait_visible().click();

    // Drag the News icon off the grid into the freed dock slot.
    let news = app.locator(Selector::id("name").text_exact("News")).snapshot();
    let from = (news.x as f64 + news.width as f64 / 2.0, news.y as f64 - 24.0);
    // Where the leftmost remaining favourite (Notes) sits before anything hovers.
    let notes_before = app.locator(Selector::id("glyph").text_exact("📝")).snapshot();
    // Hold the drag over the very left of the dock without releasing: the dock
    // should open a slot ahead of Notes, shuffling it right.
    drag_hold(&app, from, (x0 + 4.0, icon_y + 28.0));
    let notes_hovered = app.locator(Selector::id("glyph").text_exact("📝")).snapshot();
    assert!(
        notes_hovered.x > notes_before.x,
        "dock should open a slot for the hovering drag, shuffling Notes right \
         (was x={}, now x={})",
        notes_before.x,
        notes_hovered.x,
    );
    drag_release(&app, (x0 + 4.0, icon_y + 28.0));
    // Docked icons are label-less, so its grid label is gone.
    app.locator(Selector::id("name").text_exact("News")).wait_hidden();
}

/// Long-pressing a dock favourite opens its shortcut menu; sliding that same
/// still-down finger into a drag has to take the menu with it, the way a grid
/// icon's does — otherwise the menu hangs over the drag.
#[makepad_test]
fn dock_drag_hides_its_context_menu(app: TestApp) {
    app.locator(Selector::id("name").text_exact("News")).wait_visible();
    let (x1, icon_y) = dock_slot(&app, 1.0);
    let (sx, sy) = (x1 + 28.0, icon_y + 28.0);

    // Hold still on the favourite until the long press opens its menu.
    app.forward(vec![StudioToApp::MouseDown(RemoteMouseDown {
        button_raw_bits: 1, x: sx, y: sy, time: 0.0,
        modifiers: RemoteKeyModifiers::default(),
    })]);
    for _ in 0 .. 9 {
        let _ = app.widget_snapshot();
        std::thread::sleep(std::time::Duration::from_millis(90));
    }
    app.locator(Selector::all().text_contains("App Info")).wait_visible();

    // Now slide out of the menu into a drag, up onto the grid, and release.
    let pager = app.locator(Selector::id("home_pager")).snapshot();
    let (tx, ty) = (
        pager.x as f64 + pager.width as f64 * 0.12,
        pager.y as f64 + pager.height as f64 * 0.9,
    );
    let mut msgs = Vec::new();
    for i in 1 ..= 10 {
        let f = i as f64 / 10.0;
        msgs.push(StudioToApp::MouseMove(RemoteMouseMove {
            time: 0.0,
            x: sx + (tx - sx) * f,
            y: sy + (ty - sy) * f,
            modifiers: RemoteKeyModifiers::default(),
        }));
    }
    app.forward(msgs);
    app.locator(Selector::all().text_contains("App Info")).wait_hidden();
    drag_release(&app, (tx, ty));
}

/// A dock favourite can be dragged out onto the home grid, where it becomes a
/// normal labelled icon (and leaves the dock).
#[makepad_test]
fn dock_icon_drags_out_to_home(app: TestApp) {
    enter_edit_mode(&app);
    // Notes lives in the dock, so it has no grid label yet.
    app.locator(Selector::id("name").text_exact("Notes")).wait_hidden();

    let (x1, icon_y) = dock_slot(&app, 1.0);
    let pager = app.locator(Selector::id("home_pager")).snapshot();
    let (px, py) = (pager.x as f64, pager.y as f64);
    let (pw, ph) = (pager.width as f64, pager.height as f64);
    // Up into the empty bottom-left grid cell.
    drag(&app, (x1 + 28.0, icon_y + 28.0), (px + pw * 0.12, py + ph * 0.9));

    app.locator(Selector::id("name").text_exact("Notes")).wait_visible();
}

/// Enters edit mode by long-pressing EMPTY home-screen space (shared
/// preamble) — the route that works on touch, now that the per-app menu keeps
/// only app verbs.
fn enter_edit_mode(app: &TestApp) {
    let pager = app.locator(Selector::id("home_pager")).wait_visible().snapshot();
    // Low on the page, clear of any icon.
    let (x, y) = (
        pager.x as f64 + pager.width as f64 * 0.5,
        pager.y as f64 + pager.height as f64 * 0.86,
    );
    app.forward(vec![StudioToApp::MouseDown(RemoteMouseDown {
        button_raw_bits: 1, x, y, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })]);
    for _ in 0 .. 9 {
        let _ = app.widget_snapshot();
        std::thread::sleep(std::time::Duration::from_millis(90));
    }
    app.forward(vec![StudioToApp::MouseUp(RemoteMouseUp {
        button_raw_bits: 1, x, y, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })]);
    // Holding empty space enters edit mode directly (no menu hop, and it works
    // on touch where there's no right-click).
    app.locator(Selector::id("badge")).wait_visible();
    for _ in 0 .. 18 {
        let _ = app.widget_snapshot();
        std::thread::sleep(std::time::Duration::from_millis(40));
    }
}

/// The widget gallery's size chooser works: opening the Counter widget defaults
/// to its 2×2 size (named on the Add button), and tapping the 2×1 chip changes
/// the chosen size — which is what gets placed.
#[makepad_test]
fn widget_gallery_size_chooser(app: TestApp) {
    enter_edit_mode(&app);
    app.locator(Selector::all().text_exact("＋ Widget")).wait_visible().click();
    // Open the Counter widget's detail (no Counter icon on page 0, so the gallery
    // row is the only "Counter" match).
    app.locator(Selector::all().text_contains("Counter")).wait_visible().click();
    // Counter defaults to 2×2, named on the Add button.
    app.locator(Selector::all().text_contains("Add 2×2")).wait_visible();
    // Choosing the 2×1 chip changes the size to be placed.
    app.locator(Selector::all().text_contains("2×1")).wait_visible().click();
    app.locator(Selector::all().text_contains("Add 2×1")).wait_visible();
}

/// The App Store installs a catalog app: tapping "Get" on Dice flips it to
/// "Remove" (one fewer "Get") and drops a Dice icon onto the home screen.
#[makepad_test]
fn app_store_installs_catalog_app(app: TestApp) {
    enter_edit_mode(&app);
    app.locator(Selector::all().text_exact("＋ App")).wait_visible().click();
    // Two catalog apps (Dice, Tip) are installable, and Dice isn't on home yet.
    app.locator(Selector::all().text_exact("Get").nth(1)).wait_visible();
    app.locator(Selector::id("name").text_exact("Dice")).wait_hidden();
    // Install the first catalog app (Dice).
    app.locator(Selector::all().text_exact("Get").nth(0)).wait_visible().click();
    // One "Get" remains (Tip); Dice flipped to "Remove".
    app.locator(Selector::all().text_exact("Get").nth(1)).wait_hidden();
    // Dismiss the store by tapping above its panel; the Dice icon is now on home.
    let pager = app.locator(Selector::id("home_pager")).snapshot();
    let (cx, cy) = (
        pager.x as f64 + pager.width as f64 * 0.5,
        pager.y as f64 + pager.height as f64 * 0.06,
    );
    app.forward(vec![
        StudioToApp::MouseDown(RemoteMouseDown {
            button_raw_bits: 1, x: cx, y: cy, time: 0.0, modifiers: RemoteKeyModifiers::default(),
        }),
        StudioToApp::MouseUp(RemoteMouseUp {
            button_raw_bits: 1, x: cx, y: cy, time: 0.0, modifiers: RemoteKeyModifiers::default(),
        }),
    ]);
    app.locator(Selector::id("name").text_exact("Dice")).wait_visible();
}

/// In edit mode, dragging an app icon to an empty cell moves it there
/// (the headline long-press-to-rearrange interaction).
#[makepad_test]
fn edit_mode_drag_reorder(app: TestApp) {
    // Enter edit mode via the context menu.
    enter_edit_mode(&app);
    // Let the edit-bar reveal animation settle so grid positions are final.
    for _ in 0 .. 18 {
        let _ = app.widget_snapshot();
        std::thread::sleep(std::time::Duration::from_millis(40));
    }

    // Drag Calculator down into an empty cell near the bottom of the page.
    let cal = app.locator(Selector::id("name").text_exact("Calculator")).snapshot();
    let (sx, sy) = (cal.x as f64 + cal.width as f64 / 2.0, cal.y as f64 - 24.0);
    let (tx, ty) = (sx, sy + 240.0);
    let mut msgs = vec![StudioToApp::MouseDown(RemoteMouseDown {
        button_raw_bits: 1, x: sx, y: sy, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })];
    for i in 1 ..= 10 {
        let t = i as f64 / 10.0;
        msgs.push(StudioToApp::MouseMove(RemoteMouseMove {
            time: 0.0, x: sx + (tx - sx) * t, y: sy + (ty - sy) * t,
            modifiers: RemoteKeyModifiers::default(),
        }));
    }
    app.forward(msgs);
    let _ = app.widget_snapshot();
    app.forward(vec![StudioToApp::MouseUp(RemoteMouseUp {
        button_raw_bits: 1, x: tx, y: ty, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })]);

    // Calculator's icon should now sit lower than where it started (moved down a row).
    // Poll until it settles at the new row.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut moved = false;
    while std::time::Instant::now() < deadline {
        let now = app.locator(Selector::id("name").text_exact("Calculator")).snapshot();
        if (now.y as f64) > cal.y as f64 + 100.0 {
            moved = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(moved, "Calculator icon did not move down after the drag");
}

/// The bottom dock holds label-less favorite icons; tapping one opens the app.
/// Notes lives only in the dock (removed from the grid), so its glyph is unique.
#[makepad_test]
fn dock_icon_opens_app(app: TestApp) {
    app.locator(Selector::id("glyph").text_exact("📝"))
        .wait_visible()
        .click();
    app.locator(Selector::id("editor")).wait_visible();
    app.locator(Selector::id("back_button")).wait_visible().click();
    app.locator(Selector::id("editor")).wait_hidden();
}

/// Swiping down on the home screen opens the Spotlight-style search overlay,
/// whose field filters the app list; Cancel dismisses it.
#[makepad_test]
fn swipe_down_opens_search(app: TestApp) {
    app.locator(Selector::id("home_pager"))
        .wait_visible()
        .drag_by(0.0, 220.0);
    app.locator(Selector::id("s_input")).wait_visible().fill("calc");
    // Only matching apps remain in the results grid.
    app.locator(Selector::id("d_name").text_exact("Calculator")).wait_visible();
    app.locator(Selector::id("d_name").text_exact("Weather")).wait_hidden();
    app.locator(Selector::widget_type("GlassButton").text_exact("Cancel"))
        .wait_visible()
        .click();
    app.locator(Selector::id("s_input")).wait_hidden();
}

/// The app drawer's search field filters the grid as you type.
#[makepad_test]
fn drawer_search_filters(app: TestApp) {
    app.locator(Selector::id("home_pager"))
        .wait_visible()
        .drag_by(0.0, -250.0);
    app.locator(Selector::id("search_input")).wait_visible().fill("note");
    app.locator(Selector::id("d_name").text_exact("Notes")).wait_visible();
    app.locator(Selector::id("d_name").text_exact("Calculator")).wait_hidden();
}

/// Right-clicking empty home-screen space opens the background menu.
#[makepad_test]
fn right_click_background_opens_menu(app: TestApp) {
    app.locator(Selector::id("home_pager")).wait_visible();
    // Right-click (secondary button) low on the screen, below the icon grid.
    let snap = app.locator(Selector::id("home_pager")).snapshot();
    // The bottom grid row (row 5) is empty in the default layout.
    let x = snap.x as f64 + snap.width as f64 / 2.0;
    let y = snap.y as f64 + snap.height as f64 * 0.9;
    app.forward(vec![StudioToApp::MouseDown(RemoteMouseDown {
        button_raw_bits: 2, x, y, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })]);
    app.forward(vec![StudioToApp::MouseUp(RemoteMouseUp {
        button_raw_bits: 2, x, y, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })]);
    app.locator(Selector::all().text_exact("Edit Home Screen")).wait_visible();
    app.locator(Selector::all().text_exact("Change Wallpaper")).wait_visible();
}

/// Resizing a widget makes its Splash content reflow: growing the clock from
/// 2x1 to 2x2 reveals its world-clock rows, shrinking it back hides them
/// (the host's on_widget_resize call into the isolate).
#[makepad_test]
fn widget_resize_reflows_content(app: TestApp) {
    // World clocks are hidden at the default 2x1 span.
    app.locator(Selector::id("w_time_sm").text_contains(":")).wait_visible();
    app.locator(Selector::all().text_exact("London")).wait_hidden();

    // Enter edit mode via the News icon's context menu.
    enter_edit_mode(&app);
    app.locator(Selector::id("badge")).wait_visible();
    // Let the edit-bar reveal animation settle so the pager/handle geometry is final.
    for _ in 0 .. 18 {
        let _ = app.widget_snapshot();
        std::thread::sleep(std::time::Duration::from_millis(40));
    }

    // The clock tile spans cols 0-1, row 0; its resize handle sits at the
    // bottom-right corner of that cell block.
    let pager = app.locator(Selector::id("home_pager")).snapshot();
    let (px, py) = (pager.x as f64, pager.y as f64);
    let (pw, ph) = (pager.width as f64, pager.height as f64);
    let (cx0, cy0) = (px + pw * 0.5 - 4.0, py + ph / 6.0 - 4.0);

    // Drag the handle down one cell: 2x1 -> 2x2 reveals the world clocks.
    let mut msgs = vec![StudioToApp::MouseDown(RemoteMouseDown {
        button_raw_bits: 1, x: cx0, y: cy0, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })];
    for i in 1 ..= 6 {
        msgs.push(StudioToApp::MouseMove(RemoteMouseMove {
            time: 0.0, x: cx0, y: cy0 + ph / 6.0 * (i as f64 / 6.0),
            modifiers: RemoteKeyModifiers::default(),
        }));
    }
    msgs.push(StudioToApp::MouseUp(RemoteMouseUp {
        button_raw_bits: 1, x: cx0, y: cy0 + ph / 6.0, time: 0.0,
        modifiers: RemoteKeyModifiers::default(),
    }));
    app.forward(msgs);
    app.locator(Selector::all().text_exact("London")).wait_visible();

    // Drag it back up: 2x2 -> 2x1 hides them again.
    let (cx1, cy1) = (px + pw * 0.5 - 4.0, py + ph * 2.0 / 6.0 - 4.0);
    let mut msgs = vec![StudioToApp::MouseDown(RemoteMouseDown {
        button_raw_bits: 1, x: cx1, y: cy1, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })];
    for i in 1 ..= 6 {
        msgs.push(StudioToApp::MouseMove(RemoteMouseMove {
            time: 0.0, x: cx1, y: cy1 - ph / 6.0 * (i as f64 / 6.0),
            modifiers: RemoteKeyModifiers::default(),
        }));
    }
    msgs.push(StudioToApp::MouseUp(RemoteMouseUp {
        button_raw_bits: 1, x: cx1, y: cy1 - ph / 6.0, time: 0.0,
        modifiers: RemoteKeyModifiers::default(),
    }));
    app.forward(msgs);
    app.locator(Selector::all().text_exact("London")).wait_hidden();
}

/// The Android resize handle shown with a widget's context menu is grabbable:
/// right-click the clock to open its menu (which arms the resize indicator), then
/// drag the bottom-right handle down one cell. 2x1 -> 2x2 reveals the world clocks.
/// Regression test — the handle was previously purely decorative (ungrabbable),
/// because the modal menu swallowed the press.
#[makepad_test]
fn resize_widget_from_context_menu_handle(app: TestApp) {
    // World clocks are hidden at the default 2x1 clock span.
    app.locator(Selector::id("w_time_sm").text_contains(":")).wait_visible();
    app.locator(Selector::all().text_exact("London")).wait_hidden();

    let pager = app.locator(Selector::id("home_pager")).snapshot();
    let (px, py) = (pager.x as f64, pager.y as f64);
    let (pw, ph) = (pager.width as f64, pager.height as f64);
    let cell_w = pw / 4.0;
    let cell_h = ph / 6.0;

    // Right-click the clock widget (cols 0-1, row 0) to open its context menu,
    // which arms the resize indicator (sets the pager's resize_hint).
    let (rcx, rcy) = (px + cell_w, py + cell_h * 0.5);
    app.forward(vec![
        StudioToApp::MouseDown(RemoteMouseDown {
            button_raw_bits: 2, x: rcx, y: rcy, time: 0.0, modifiers: RemoteKeyModifiers::default(),
        }),
        StudioToApp::MouseUp(RemoteMouseUp {
            button_raw_bits: 2, x: rcx, y: rcy, time: 0.0, modifiers: RemoteKeyModifiers::default(),
        }),
    ]);
    // The widget menu is up (and resize_hint is set a frame later).
    app.locator(Selector::all().text_exact("Remove Widget")).wait_visible();
    for _ in 0 .. 4 {
        let _ = app.widget_snapshot();
        std::thread::sleep(std::time::Duration::from_millis(40));
    }

    // The clock's resize handle sits at the bottom-right corner of its 2x1 cell
    // block: col 2 * cell_w, row 1 * cell_h. Grab it and drag down one cell.
    let (hx, hy) = (px + cell_w * 2.0, py + cell_h);
    let mut msgs = vec![StudioToApp::MouseDown(RemoteMouseDown {
        button_raw_bits: 1, x: hx, y: hy, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })];
    for i in 1 ..= 6 {
        msgs.push(StudioToApp::MouseMove(RemoteMouseMove {
            time: 0.0, x: hx, y: hy + cell_h * (i as f64 / 6.0),
            modifiers: RemoteKeyModifiers::default(),
        }));
    }
    msgs.push(StudioToApp::MouseUp(RemoteMouseUp {
        button_raw_bits: 1, x: hx, y: hy + cell_h, time: 0.0,
        modifiers: RemoteKeyModifiers::default(),
    }));
    app.forward(msgs);

    // The clock grew to 2x2, revealing the world-clock rows — proving the handle
    // grabbed and the drag resized the widget.
    app.locator(Selector::all().text_exact("London")).wait_visible();
}

/// Long-pressing empty home-screen space enters jiggle/edit mode (iOS-style),
/// revealing the remove badges; tapping empty space again exits it.
#[makepad_test]
fn long_press_empty_space_starts_jiggle(app: TestApp) {
    app.locator(Selector::id("home_pager")).wait_visible();
    let snap = app.locator(Selector::id("home_pager")).snapshot();
    // The bottom grid row is empty in the default layout.
    let x = snap.x as f64 + snap.width as f64 * 0.5;
    let y = snap.y as f64 + snap.height as f64 * 0.9;
    app.forward(vec![StudioToApp::MouseDown(RemoteMouseDown {
        button_raw_bits: 1, x, y, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })]);
    for _ in 0 .. 9 {
        let _ = app.widget_snapshot();
        std::thread::sleep(std::time::Duration::from_millis(90));
    }
    app.forward(vec![StudioToApp::MouseUp(RemoteMouseUp {
        button_raw_bits: 1, x, y, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })]);
    // Edit mode shows the remove badges (SDF-drawn, id "badge") and the edit bar
    // (with its page controls).
    app.locator(Selector::id("badge")).wait_visible();
    app.locator(Selector::all().text_exact("＋ Page")).wait_visible();
    // Let the edit-bar reveal animation settle so the grid is at its final position
    // before the exit tap (the slide is deliberately unhurried).
    for _ in 0 .. 18 {
        let _ = app.widget_snapshot();
        std::thread::sleep(std::time::Duration::from_millis(40));
    }
    // A quick tap on empty space exits edit mode; the badges disappear.
    app.forward(vec![
        StudioToApp::MouseDown(RemoteMouseDown {
            button_raw_bits: 1, x, y, time: 0.0, modifiers: RemoteKeyModifiers::default(),
        }),
        StudioToApp::MouseUp(RemoteMouseUp {
            button_raw_bits: 1, x, y, time: 0.0, modifiers: RemoteKeyModifiers::default(),
        }),
    ]);
    app.locator(Selector::id("badge")).wait_hidden();
}

/// Long-pressing and *holding* an app in the drawer hands the touch to the home
/// screen (Android-style): the drawer slides away and the same drag drops the
/// app onto the grid. Counter starts on page 2, so success = it appears on the
/// first home page after the drag-out.
#[makepad_test]
fn drag_app_from_drawer_to_home(app: TestApp) {
    app.locator(Selector::id("name").text_exact("Counter")).wait_hidden();
    // Open the drawer and grab Counter's tile (just above its name label).
    app.locator(Selector::id("home_pager"))
        .wait_visible()
        .drag_by(0.0, -250.0);
    let snap = app
        .locator(Selector::id("d_name").text_exact("Counter"))
        .wait_visible()
        .snapshot();
    let (sx, sy) = (snap.x as f64 + snap.width as f64 / 2.0, snap.y as f64 - 24.0);
    app.forward(vec![StudioToApp::MouseDown(RemoteMouseDown {
        button_raw_bits: 1, x: sx, y: sy, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })]);
    // Hold past the 0.5s long-press threshold so the drag-out kicks in.
    for _ in 0 .. 9 {
        let _ = app.widget_snapshot();
        std::thread::sleep(std::time::Duration::from_millis(90));
    }
    // Drag into the lower-left of the (revealing) home grid — an empty cell — and drop.
    let pager = app.locator(Selector::id("home_pager")).snapshot();
    let (tx, ty) = (
        pager.x as f64 + pager.width as f64 * 0.3,
        pager.y as f64 + pager.height as f64 * 0.72,
    );
    let mut msgs = Vec::new();
    for i in 1 ..= 12 {
        let t = i as f64 / 12.0;
        msgs.push(StudioToApp::MouseMove(RemoteMouseMove {
            time: 0.0, x: sx + (tx - sx) * t, y: sy + (ty - sy) * t,
            modifiers: RemoteKeyModifiers::default(),
        }));
    }
    app.forward(msgs);
    let _ = app.widget_snapshot();
    app.forward(vec![StudioToApp::MouseUp(RemoteMouseUp {
        button_raw_bits: 1, x: tx, y: ty, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })]);
    // Counter now lives on the first home page.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if app
            .locator(Selector::id("name").text_exact("Counter"))
            .snapshot()
            .width
            > 0
        {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "Counter was not placed on the home screen");
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// The AI create bar generates and installs an app end to end, offline: the
/// suite runs with HOST_LAUNCHER_AGENT_CMD pointing at the fake_acp binary
/// (see src/bin/fake_acp.rs), which streams a valid Splash app split across
/// chunks. Typing a request and hitting return must end with the new app's
/// icon on the home grid — exercising the ACP client, fence extraction,
/// parser validation, manifest install, and the bar's status round-trip.
#[makepad_test]
fn create_bar_generates_and_installs_app(app: TestApp) {
    app.locator(Selector::id("create_input"))
        .wait_visible()
        .fill("pomodoro timer");
    submit_prompt(&app);
    // The fake agent replies instantly; the icon should land on the grid.
    app.locator(Selector::id("name").text_exact("Pomodoro")).wait_visible();
    // The bar flashes the result and stays out of the input state meanwhile.
    app.locator(Selector::all().text_contains("added")).wait_visible();
    // Open the generated app and check it actually runs (its title renders).
    app.locator(Selector::id("name").text_exact("Pomodoro")).click();
    app.locator(Selector::all().text_exact("1500")).wait_visible();
    app.locator(Selector::id("back_button")).wait_visible().click();
}

/// A generated app that fails validation is sent back to the agent with the
/// compile errors, and the repaired reply installs. The fake agent streams a
/// non-parsing script for any request containing "broken", then a valid one
/// for the repair prompt — so this test passing proves the validator caught
/// the bad script and the repair loop ran.
#[makepad_test]
fn create_bar_repairs_broken_app(app: TestApp) {
    app.locator(Selector::id("create_input"))
        .wait_visible()
        .fill("broken thing");
    submit_prompt(&app);
    app.locator(Selector::id("name").text_exact("Pomodoro")).wait_visible();
}

/// "Modify App…" on a GENERATED app rewrites it in place (the built-in path
/// is covered separately): same pipeline, but the prompt carries the current
/// source and the result keeps the app's id. The fake agent answers any refine prompt with a renamed
/// "Pomodoro Pro" variant, so the icon's label changing in place (no second
/// icon appearing) proves the update path end to end.
#[makepad_test]
fn refine_updates_generated_app_in_place(app: TestApp) {
    // First create the app via the bar.
    app.locator(Selector::id("create_input"))
        .wait_visible()
        .fill("pomodoro timer");
    submit_prompt(&app);
    let icon = app.locator(Selector::id("name").text_exact("Pomodoro")).wait_visible();

    // Long-press its icon (grip point above the label, like other tests).
    let snap = icon.snapshot();
    let (x, y) = (snap.x as f64 + snap.width as f64 / 2.0, snap.y as f64 - 24.0);
    app.forward(vec![StudioToApp::MouseDown(RemoteMouseDown {
        button_raw_bits: 1, x, y, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })]);
    for _ in 0 .. 9 {
        let _ = app.widget_snapshot();
        std::thread::sleep(std::time::Duration::from_millis(90));
    }
    app.forward(vec![StudioToApp::MouseUp(RemoteMouseUp {
        button_raw_bits: 1, x, y, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })]);

    // Pick Modify, type the change after the prefill, submit. (fill() replaces
    // the whole field, so it carries the "✏️ Pomodoro: " prefix the focused
    // bar already holds — wiping that prefix deliberately means "create", the
    // behavior the intent tests cover.)
    app.locator(Selector::all().text_contains("Modify App")).wait_visible().click();
    app.locator(Selector::id("create_input"))
        .wait_visible()
        .fill("✏️ Pomodoro: add a reset button");
    submit_prompt(&app);

    // The app was renamed in place — new label appears, old label is gone.
    app.locator(Selector::id("name").text_exact("Pomodoro Pro")).wait_visible();
    app.locator(Selector::id("name").text_exact("Pomodoro")).wait_hidden();
}

/// The first-run setup modal: paste an API key → the provider is inferred
/// from its prefix → Save writes a minimal octos config. The suite runs with
/// OCTOS_CONFIG_DIR pointed at a temp dir so the test never touches a real
/// ~/.octos (and the generation tests are unaffected — their env override
/// short-circuits backend selection before any config is read).
#[makepad_test]
fn setup_modal_saves_pasted_key(app: TestApp) {
    // Opened at boot via HOST_LAUNCHER_DEBUG_STATE=setup? No — this test runs
    // in the normal suite; open it by tapping the bar's ✨ glyph instead.
    app.locator(Selector::id("create_glyph")).wait_visible().click();
    app.locator(Selector::all().text_exact("Set up AI generation")).wait_visible();

    app.locator(Selector::id("setup_key_input"))
        .wait_visible()
        .fill("sk-ant-api03-testkey");
    app.locator(Selector::all().text_exact("Detected: anthropic")).wait_visible();

    app.locator(Selector::all().text_exact("Save")).wait_visible().click();
    // Modal closes; the bar flashes readiness.
    app.locator(Selector::all().text_contains("anthropic ready")).wait_visible();

    // The config landed where OCTOS_CONFIG_DIR points, with the right shape.
    let dir = std::env::var("OCTOS_CONFIG_DIR").expect("suite sets OCTOS_CONFIG_DIR");
    let text = std::fs::read_to_string(std::path::Path::new(&dir).join("config.json"))
        .expect("setup wrote config.json");
    assert!(text.contains("\"provider\": \"anthropic\""));
    assert!(text.contains("ANTHROPIC_API_KEY"));
}

/// The agent options (model / effort / thinking) are per-backend: they only
/// appear for a backend whose knobs we can actually deliver. The suite runs
/// against fake_acp — an arbitrary ACP binary — so the honest answer is no
/// options at all, and the row must stay hidden even with the prompt focused
/// and full of text. A control that silently does nothing would be worse than
/// no control.
#[makepad_test]
fn agent_options_stay_hidden_for_an_unconfigurable_backend(app: TestApp) {
    app.locator(Selector::id("create_input"))
        .wait_visible()
        .fill("a pomodoro timer");
    // Send appears, so the bar has definitely noticed the text...
    app.locator(Selector::id("create_send")).wait_visible();
    // ...but there's nothing to configure on this agent.
    app.locator(Selector::id("create_options")).wait_hidden();
}

/// The prompt space becomes the agent's console once submitted: the same box
/// that held the request shows what the agent is doing, it keeps the WHOLE run
/// rather than a trailing window, the ✨ yields its slot to the show/hide
/// chevron, and Stop puts the composer back.
///
/// Runs against the fake agent's "slow" scenario: it fires a burst of tool
/// calls and then stalls, so every assertion here happens while the generation
/// is genuinely in flight. It ends on Stop rather than waiting for the install
/// (which create_bar_generates_and_installs_app covers) — otherwise the tail of
/// the test would be racing the stall, and each assertion costs a widget-
/// snapshot round trip, which is slow enough under a loaded full-suite run to
/// lose that race intermittently.
#[makepad_test]
fn agent_console_shows_and_collapses(app: TestApp) {
    app.locator(Selector::id("create_input"))
        .wait_visible()
        .fill("slow pomodoro");
    submit_prompt(&app);

    // The console takes over the prompt's space, and keeps the whole run: the
    // agent fires 20 tool calls, and the FIRST line is still there afterwards —
    // a last-N console would have dropped it long ago.
    app.locator(Selector::id("create_output")).wait_visible();
    app.locator(Selector::id("activity_log").text_contains("Starting agent")).wait_visible();
    app.locator(Selector::id("activity_log").text_contains("(20)")).wait_visible();

    // The ✨ yields its slot to the chevron — exactly one is ever on screen.
    app.locator(Selector::id("create_glyph")).wait_hidden();

    // Hide → just the status line; show → console back.
    app.locator(Selector::id("create_toggle")).wait_visible().click();
    app.locator(Selector::id("create_output")).wait_hidden();
    app.locator(Selector::id("create_toggle")).click();
    app.locator(Selector::id("create_output")).wait_visible();

    // Stop ends the run: the console goes with it and the ✨ comes back for
    // the next prompt.
    app.locator(Selector::id("create_cancel")).wait_visible().click();
    app.locator(Selector::id("create_output")).wait_hidden();
    app.locator(Selector::id("create_glyph")).wait_visible();
}

/// Mini-app data persists on real disk through the sandboxed `fs`: a to-do
/// added, then the app FORCE-STOPPED (its Splash VM torn down — in-VM state
/// gone), then reopened — the task must come back from /todo.json in the
/// app's private storage. Also proves the jail write/read round-trip and the
/// boot-time load pattern end to end.
#[makepad_test]
fn todo_persists_across_force_stop(app: TestApp) {
    // Open To-Do from the drawer and add a task.
    app.locator(Selector::id("home_pager"))
        .wait_visible()
        .drag_by(0.0, -250.0);
    app.locator(Selector::id("d_name").text_exact("To-Do"))
        .wait_visible()
        .click();
    app.locator(Selector::id("task_input"))
        .wait_visible()
        .fill("Survive a force stop");
    app.locator(Selector::widget_type("GlassButton").text_exact("Add"))
        .wait_visible()
        .click();
    app.locator(Selector::all().text_contains("Survive a force stop")).wait_visible();

    // Back to home, then Force Stop the app from its long-press menu — this
    // tears down the whole Splash VM, so module-level state is truly gone.
    // To-Do lives in the label-less dock, so target its ✅ glyph.
    app.locator(Selector::id("back_button")).wait_visible().click();
    let snap = app.locator(Selector::id("glyph").text_exact("✅")).wait_visible().snapshot();
    let (x, y) = (
        snap.x as f64 + snap.width as f64 / 2.0,
        snap.y as f64 + snap.height as f64 / 2.0,
    );
    app.forward(vec![StudioToApp::MouseDown(RemoteMouseDown {
        button_raw_bits: 1, x, y, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })]);
    for _ in 0 .. 9 {
        let _ = app.widget_snapshot();
        std::thread::sleep(std::time::Duration::from_millis(90));
    }
    app.forward(vec![StudioToApp::MouseUp(RemoteMouseUp {
        button_raw_bits: 1, x, y, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })]);
    // Force Stop lives on the App Info page now, not the menu.
    app.locator(Selector::all().text_contains("App Info")).wait_visible().click();
    app.locator(Selector::all().text_exact("Force Stop")).wait_visible().click();
    // Back out with the page's own × — a modal you can only dismiss by tapping
    // the backdrop isn't discoverable. (It stays open after a force stop.)
    app.locator(Selector::id("ai_close")).wait_visible().click();
    app.locator(Selector::all().text_exact("Force Stop")).wait_hidden();

    // Reopen from the dock: the fresh VM must load the task back from its
    // private storage.
    app.locator(Selector::id("glyph").text_exact("✅")).wait_visible().click();
    app.locator(Selector::all().text_contains("Survive a force stop")).wait_visible();
}

/// The widget-gallery preview isolate is torn down on Back and cleanly
/// rebuilt on the next preview — proving an empty set_text() actually stops
/// the isolate (rather than leaving it ticking) and that a fresh one allocs
/// and renders afterward. Regression for the preview-isolate leak.
#[makepad_test]
fn widget_preview_teardown_and_reopen(app: TestApp) {
    enter_edit_mode(&app);
    app.locator(Selector::all().text_exact("＋ Widget")).wait_visible().click();

    // Preview the Counter widget: its live preview renders (isolate A).
    app.locator(Selector::all().text_contains("Counter")).wait_visible().click();
    app.locator(Selector::all().text_contains("Add 2×2")).wait_visible();

    // Back → the list returns and the preview isolate is torn down.
    app.locator(Selector::all().text_exact("‹ Back")).wait_visible().click();
    app.locator(Selector::all().text_contains("Add 2×2")).wait_hidden();

    // Preview again: a FRESH isolate must alloc and render (would fail if the
    // torn-down view/isolate weren't cleanly replaced).
    app.locator(Selector::all().text_contains("Counter")).wait_visible().click();
    app.locator(Selector::all().text_contains("Add 2×2")).wait_visible();
}

/// "Modify App…" prefills the create bar with a pencil + the app's name and
/// focuses it, so the user just types the change. Submitting rewrites that
/// app in place — here a built-in (Notes), which becomes a user override.
#[makepad_test]
fn modify_menu_prefills_bar_and_rewrites_app(app: TestApp) {
    // Long-press the Notes icon in the dock (label-less → target its glyph).
    let snap = app.locator(Selector::id("glyph").text_exact("📝")).wait_visible().snapshot();
    let (x, y) = (
        snap.x as f64 + snap.width as f64 / 2.0,
        snap.y as f64 + snap.height as f64 / 2.0,
    );
    app.forward(vec![StudioToApp::MouseDown(RemoteMouseDown {
        button_raw_bits: 1, x, y, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })]);
    for _ in 0 .. 9 {
        let _ = app.widget_snapshot();
        std::thread::sleep(std::time::Duration::from_millis(90));
    }
    app.forward(vec![StudioToApp::MouseUp(RemoteMouseUp {
        button_raw_bits: 1, x, y, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })]);

    // Pick "Modify App…" — the bar prefills with the pencil + app name.
    app.locator(Selector::all().text_contains("Modify App")).wait_visible().click();
    app.locator(Selector::all().text_contains("✏️ Notes:")).wait_visible();

    // Type the change after the prefix and submit. (fill() replaces the text,
    // so include the prefix the way the focused input already holds it.)
    app.locator(Selector::id("create_input"))
        .wait_visible()
        .fill("✏️ Notes: add a word count");
    submit_prompt(&app);

    // The fake agent answers any modify prompt with a renamed variant, so the
    // rewrite landing in place is visible as the app's new name.
    app.locator(Selector::id("glyph").text_exact("🍅")).wait_visible();
}

/// A plain request that names an installed app and reads like an edit is
/// recognized as a modification — no menu, no prefix: just
/// "make the notes app …". The Notes app is rewritten in place.
#[makepad_test]
fn typed_request_recognizes_modify_intent(app: TestApp) {
    app.locator(Selector::id("create_input"))
        .wait_visible()
        .fill("make the notes app show a word count");
    submit_prompt(&app);
    // Rewritten in place (fake agent renames it), NOT installed as a new app:
    // the modified Notes keeps its dock slot and picks up the new glyph.
    app.locator(Selector::id("glyph").text_exact("🍅")).wait_visible();
    // ...and no second icon appeared on the grid for a "new" app.
    app.locator(Selector::id("name").text_exact("Pomodoro")).wait_hidden();
}

/// Modifying an app archives what it replaced, and the version history can
/// restore it — the undo trail for AI edits. Modify Notes (its pre-edit state
/// is snapshotted), then restore that version and watch the app come back.
#[makepad_test]
fn version_history_restores_a_previous_version(app: TestApp) {
    // Modify the Notes app by typed intent (rewrites it in place).
    app.locator(Selector::id("create_input"))
        .wait_visible()
        .fill("make the notes app show a word count");
    submit_prompt(&app);
    app.locator(Selector::id("glyph").text_exact("🍅")).wait_visible();

    // Long-press the (now-modified) app in the dock and open its history.
    let snap = app.locator(Selector::id("glyph").text_exact("🍅")).snapshot();
    let (x, y) = (
        snap.x as f64 + snap.width as f64 / 2.0,
        snap.y as f64 + snap.height as f64 / 2.0,
    );
    app.forward(vec![StudioToApp::MouseDown(RemoteMouseDown {
        button_raw_bits: 1, x, y, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })]);
    for _ in 0 .. 9 {
        let _ = app.widget_snapshot();
        std::thread::sleep(std::time::Duration::from_millis(90));
    }
    app.forward(vec![StudioToApp::MouseUp(RemoteMouseUp {
        button_raw_bits: 1, x, y, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })]);
    // Version history now lives on the App Info page.
    app.locator(Selector::all().text_contains("App Info")).wait_visible().click();

    // The archived version is listed with the request that superseded it
    // (the note is ellipsized to the row width, so match its stable prefix).
    app.locator(Selector::all().text_contains("VERSION HISTORY")).wait_visible();
    app.locator(Selector::all().text_contains("make the notes")).wait_visible();
    // Restore it: the original Notes app (📝) is back.
    app.locator(Selector::all().text_exact("Restore").nth(0)).wait_visible().click();
    app.locator(Selector::id("glyph").text_exact("📝")).wait_visible();
}

/// The prompt is multi-line and floats OVER the home screen: typing a long
/// request grows the field (it wraps rather than scrolling sideways), and the
/// icons underneath stay exactly where they were — the bar expands in front of
/// the grid rather than displacing it.
#[makepad_test]
fn prompt_expands_over_the_grid_without_moving_it(app: TestApp) {
    let icon_before = app
        .locator(Selector::id("name").text_exact("News"))
        .wait_visible()
        .snapshot();
    let one_line = app
        .locator(Selector::id("create_input"))
        .wait_visible()
        .snapshot()
        .height;

    app.locator(Selector::id("create_input")).fill(
        "a habit tracker with three habits I can rename, a seven day streak \
         grid for each one, tap a day to toggle it done, a running total at \
         the top, and everything saved between launches",
    );

    let grown = app.locator(Selector::id("create_input")).snapshot().height;
    assert!(
        grown > one_line,
        "a long prompt should wrap and grow the field (was {one_line}, now {grown})"
    );

    let icon_after = app.locator(Selector::id("name").text_exact("News")).snapshot();
    assert_eq!(
        icon_after.y, icon_before.y,
        "the expanding prompt must overlay the grid, not push it down"
    );
}
