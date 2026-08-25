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
    // The only GlassButton with "×" is the calculator's multiply key: the header
    // close button used to be one too (hence an nth(1) here), but it's SDF now
    // so it carries no text and can't collide with this selector.
    app.locator(Selector::widget_type("GlassButton").text_exact("×"))
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

/// While a split pick is pending, the app drawer must be usable IN FRONT of
/// the docked sliver: it opens over everything, every app in it is tappable
/// (the sliver stops drawing and fencing while the drawer is up), and picking
/// one forms the split.
#[makepad_test]
fn split_pick_from_drawer(app: TestApp) {
    app.locator(Selector::id("name").text_exact("Clock"))
        .wait_visible()
        .click();
    app.locator(Selector::id("title").text_exact("Clock")).wait_visible();
    deny_prompt_if_shown(&app);
    settle(&app, 14);
    app.locator(Selector::id("split_button")).wait_visible().click();
    settle(&app, 10);
    app.locator(Selector::id("name").text_exact("Calculator")).wait_visible();
    // Swipe up: the drawer opens on top; a drawer row must be visible and
    // clickable even where the sliver used to peek.
    app.locator(Selector::id("home_pager"))
        .wait_visible()
        .drag_by(0.0, -250.0);
    app.locator(Selector::id("d_name").text_exact("Stopwatch"))
        .wait_visible()
        .click();
    settle(&app, 14);
    // Both panes up, divider between them.
    app.locator(Selector::id("title").text_exact("Clock")).wait_visible();
    app.locator(Selector::id("title").text_exact("Stopwatch")).wait_visible();
    app.locator(Selector::id("divider_pill")).wait_visible();
    // Tear down for the next test.
    app.locator(Selector::id("back_button")).wait_visible();
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

/// How many icon slots are occupied on the dock's row, once it has settled.
///
/// Read out of `widget_dump` rather than through a locator: text selectors are
/// unusable around a dock rebuild (see `dock_badge_removes_and_accepts_dropped_icon`),
/// and the snapshot reports dock glyphs as not visible while a drag is in
/// flight. The dump keeps reporting the drawn icons at their real rects
/// throughout.
///
/// Counted by DISTINCT x, because a removed favourite lingers in the dump at
/// the position it used to occupy — which is also where its left-hand
/// neighbour slides to, so a raw line count keeps reporting the pre-removal
/// number forever.
fn dock_row_len(app: &TestApp, row_y: f64) -> usize {
    let count = |app: &TestApp| {
        let mut xs: Vec<i64> = app
            .widget_dump()
            .lines()
            .filter_map(|line| {
                let f: Vec<&str> = line.split_whitespace().collect();
                if f.len() < 6 || f[2] != "glyph" {
                    return None;
                }
                let y = f[5].parse::<f64>().ok()?;
                ((y - row_y).abs() < 40.0).then(|| f[4].parse::<f64>().ok())?.map(|x| x as i64)
            })
            .collect();
        xs.sort_unstable();
        xs.dedup();
        xs.len()
    };
    let mut last = count(app);
    let mut stable = 0;
    for _ in 0 .. 40 {
        let _ = app.widget_snapshot();
        std::thread::sleep(std::time::Duration::from_millis(50));
        let now = count(app);
        stable = if now == last { stable + 1 } else { 0 };
        last = now;
        if stable >= 3 {
            break;
        }
    }
    last
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
    // The REST of the dock survives the removal. This is the regression that
    // kept coming back: pruning one favourite marks the dock's children dirty,
    // which drops all of them from the widget tree, and the survivors were
    // never re-inserted — so removing one icon made the whole dock
    // unaddressable. wait_visible, not a bare snapshot: the rebuild takes a
    // frame.
    let notes = app.locator(Selector::id("glyph").text_exact("📝")).wait_visible().snapshot();
    let dock_row_y = notes.y as f64;

    // Hold the drag over the dock, then drop it there.
    //
    // NOT asserted: the live preview where the row shuffles aside to open the
    // hovered slot. The harness can't see it. Mid-drag the widget snapshot
    // reports every dock glyph as `visible: false` and carries text for none of
    // them (the dragged icon and the untouched ones alike), while `widget_dump`
    // over-reports the other way — it still lists the favourite that was just
    // removed, at its old rect. The icons really are drawn throughout; it's
    // both observation routes that are wrong, in opposite directions. What the
    // user actually gets — the drop lands in the dock — is asserted below, and
    // `dock_icon_drags_out_to_home` covers the reverse trip.
    drag_hold(&app, from, (x0 + 4.0, icon_y + 28.0));
    // Let the drag come to rest before letting go. The slot a drop lands in
    // follows the dragged icon's CENTRE, not the finger (`dock_slot_at`), and
    // that centre eases toward the finger over several frames — release too
    // early and the drop resolves against a position the drag was still
    // travelling through, putting the icon somewhere neither of us meant.
    for _ in 0 .. 12 {
        let _ = app.widget_snapshot();
        std::thread::sleep(std::time::Duration::from_millis(60));
    }
    drag_release(&app, (x0 + 4.0, icon_y + 28.0));
    // The dock is back to full: the default dock ships FIVE favourites, one
    // was just removed leaving four, and the dropped icon makes five. (This
    // read four until the suite was run the documented way, with
    // HOST_LAUNCHER_FRESH=1 — against a persisted profile with a
    // four-favourite dock the old arithmetic happened to line up.)
    //
    // Counted off the dock's ROW, because after a dock rebuild the harness
    // stops reporting text for ANY glyph — 📝, ✅, 🎵 and the newly dropped 📰
    // all come back empty, so every text selector reads zero whatever actually
    // happened. The widgets are there and correctly laid out (evenly spaced
    // across the bar); it's the introspection that goes stale, so this counts
    // what is drawn.
    let docked = dock_row_len(&app, dock_row_y);
    assert_eq!(
        docked, 5,
        "the drop should have landed in the dock: 4 surviving favourites + News",
    );
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
/// Uninstalling an app you MADE doesn't destroy it: the store keeps offering
/// it back.
///
/// A catalog app can always be fetched again from the catalog, so the gap was
/// invisible until you uninstalled something generated or imported — those
/// exist nowhere else, and removing the manifest was the only copy gone. A
/// prompt you can't reproduce is real work destroyed by a menu item.
#[makepad_test]
fn uninstalling_a_generated_app_keeps_it_in_the_store(app: TestApp) {
    app.locator(Selector::id("create_input"))
        .wait_visible()
        .fill("pomodoro timer");
    submit_prompt(&app);
    app.locator(Selector::id("name").text_exact("Pomodoro")).wait_visible();

    // Clear the console so the icon underneath can be reached.
    app.locator(Selector::id("create_done")).wait_visible().click();
    app.locator(Selector::id("create_options")).wait_visible();
    let bar = app.locator(Selector::id("create_bar")).snapshot();
    tap(&app, bar.x as f64 / 2.0, bar.y as f64 + bar.height as f64 / 2.0);
    app.locator(Selector::id("create_options")).wait_hidden();

    // Long-press it and uninstall.
    let snap = app.locator(Selector::id("name").text_exact("Pomodoro")).snapshot();
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
    app.locator(Selector::all().text_exact("Uninstall")).wait_visible().click();
    app.locator(Selector::id("name").text_exact("Pomodoro")).wait_hidden();

    // It is offered again in the store, and comes back when asked for.
    enter_edit_mode(&app);
    app.locator(Selector::all().text_exact("＋ App")).wait_visible().click();
    app.locator(Selector::id("row_name").text_exact("Pomodoro")).wait_visible();
    app.locator(Selector::id("row_action").nth(0)).wait_visible();
}

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
    // By id, not by type+text: FOUR GlassButtons carry the text "Cancel"
    // (search, providers, and two confirm dialogs), and `confirm_cancel`
    // reports visible at 0x0, so a type+text selector can click a
    // zero-sized button that isn't the one on screen.
    app.locator(Selector::id("s_cancel")).wait_visible().click();
    // The overlay closes over a few frames; pump them before asserting.
    settle(&app, 10);
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

    // Resizing is its own mode now, armed by the widget's context menu —
    // jiggle/edit mode no longer shows a grip at all, so entering edit mode
    // here would leave nothing to drag.
    let pager = app.locator(Selector::id("home_pager")).snapshot();
    let (px, py) = (pager.x as f64, pager.y as f64);
    let (pw, ph) = (pager.width as f64, pager.height as f64);
    let (rcx, rcy) = (px + pw / 4.0, py + ph / 6.0 * 0.5);
    app.forward(vec![
        StudioToApp::MouseDown(RemoteMouseDown {
            button_raw_bits: 2, x: rcx, y: rcy, time: 0.0, modifiers: RemoteKeyModifiers::default(),
        }),
        StudioToApp::MouseUp(RemoteMouseUp {
            button_raw_bits: 2, x: rcx, y: rcy, time: 0.0, modifiers: RemoteKeyModifiers::default(),
        }),
    ]);
    app.locator(Selector::all().text_exact("Remove Widget")).wait_visible();
    for _ in 0 .. 4 {
        let _ = app.widget_snapshot();
        std::thread::sleep(std::time::Duration::from_millis(40));
    }
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

/// Growing an app ICON past 1x1 runs the real app right there on the home
/// screen; its title bar can throw it fullscreen (the SAME live instance, with
/// a stand-in holding its cells), bring it back, and shrink it to an icon.
#[makepad_test]
fn app_runs_on_home_screen(app: TestApp) {
    // Calculator is a plain icon to start with: no keypad on the home screen.
    app.locator(Selector::id("name").text_exact("Calculator")).wait_visible();
    app.locator(Selector::all().text_exact("7")).wait_hidden();

    let pager = app.locator(Selector::id("home_pager")).snapshot();
    let (px, py) = (pager.x as f64, pager.y as f64);
    let cell_w = pager.width as f64 / 4.0;
    let cell_h = pager.height as f64 / 6.0;

    // Right-click the Calculator icon (col 2, row 3) to arm the resize frame.
    let (rcx, rcy) = (px + cell_w * 2.5, py + cell_h * 3.5);
    app.forward(vec![
        StudioToApp::MouseDown(RemoteMouseDown {
            button_raw_bits: 2, x: rcx, y: rcy, time: 0.0, modifiers: RemoteKeyModifiers::default(),
        }),
        StudioToApp::MouseUp(RemoteMouseUp {
            button_raw_bits: 2, x: rcx, y: rcy, time: 0.0, modifiers: RemoteKeyModifiers::default(),
        }),
    ]);
    app.locator(Selector::all().text_exact("Remove from Home")).wait_visible();
    settle(&app, 4);

    // Drag its bottom-right handle out by one cell each way: 1x1 -> 2x2.
    let (hx, hy) = (px + cell_w * 3.0, py + cell_h * 4.0);
    let mut msgs = vec![StudioToApp::MouseDown(RemoteMouseDown {
        button_raw_bits: 1, x: hx, y: hy, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })];
    for i in 1 ..= 6 {
        let f = i as f64 / 6.0;
        msgs.push(StudioToApp::MouseMove(RemoteMouseMove {
            time: 0.0, x: hx + cell_w * f, y: hy + cell_h * f,
            modifiers: RemoteKeyModifiers::default(),
        }));
    }
    msgs.push(StudioToApp::MouseUp(RemoteMouseUp {
        button_raw_bits: 1, x: hx + cell_w, y: hy + cell_h, time: 0.0,
        modifiers: RemoteKeyModifiers::default(),
    }));
    app.forward(msgs);
    settle(&app, 10);

    // The app is LIVE in its cells: tile bar plus real calculator keys.
    app.locator(Selector::id("tile_title").text_exact("Calculator")).wait_visible();
    app.locator(Selector::all().text_exact("7")).wait_visible();

    // ⤢ throws it fullscreen; the cells keep its place with a stand-in.
    // ⤢ hands the RUNNING widget to the app screen. (The stand-in left in the
    // cells isn't assertable here: a fullscreen app covers home, so the whole
    // home screen — stand-in included — is hidden while it's up.)
    app.locator(Selector::id("tile_expand")).wait_visible().click();
    settle(&app, 14);
    app.locator(Selector::id("title").text_exact("Calculator")).wait_visible();

    // Closing it puts the app back in its cells, still running.
    app.locator(Selector::id("back_button")).wait_visible().click();
    settle(&app, 16);
    app.locator(Selector::id("tile_title").text_exact("Calculator")).wait_visible();
    app.locator(Selector::all().text_exact("7")).wait_visible();

    // Shrinking lives in the long-press menu now, not the title bar.
    let bar = app.locator(Selector::id("tile_title")).wait_visible().snapshot();
    app.forward(vec![
        StudioToApp::MouseDown(RemoteMouseDown {
            button_raw_bits: 2, x: bar.x as f64 + 8.0, y: bar.y as f64 + 6.0,
            time: 0.0, modifiers: RemoteKeyModifiers::default(),
        }),
        StudioToApp::MouseUp(RemoteMouseUp {
            button_raw_bits: 2, x: bar.x as f64 + 8.0, y: bar.y as f64 + 6.0,
            time: 0.0, modifiers: RemoteKeyModifiers::default(),
        }),
    ]);
    app.locator(Selector::all().text_exact("Shrink to Icon Only")).wait_visible().click();
    settle(&app, 10);
    app.locator(Selector::all().text_exact("7")).wait_hidden();
    app.locator(Selector::id("name").text_exact("Calculator")).wait_visible();
}

/// The stand-in's reason to exist: split-screen pick shows the home screen
/// again WHILE the app is still running as a pane, so the cells that app came
/// from must show the disabled card — and tapping it must bring the app home.
#[makepad_test]
fn home_app_stand_in_during_split_pick(app: TestApp) {
    let pager = app.locator(Selector::id("home_pager")).snapshot();
    let (px, py) = (pager.x as f64, pager.y as f64);
    let cell_w = pager.width as f64 / 4.0;
    let cell_h = pager.height as f64 / 6.0;

    // Grow the Calculator icon (col 2, row 3) into a live tile.
    let (rcx, rcy) = (px + cell_w * 2.5, py + cell_h * 3.5);
    app.forward(vec![
        StudioToApp::MouseDown(RemoteMouseDown {
            button_raw_bits: 2, x: rcx, y: rcy, time: 0.0, modifiers: RemoteKeyModifiers::default(),
        }),
        StudioToApp::MouseUp(RemoteMouseUp {
            button_raw_bits: 2, x: rcx, y: rcy, time: 0.0, modifiers: RemoteKeyModifiers::default(),
        }),
    ]);
    app.locator(Selector::all().text_exact("Remove from Home")).wait_visible();
    settle(&app, 4);
    let (hx, hy) = (px + cell_w * 3.0, py + cell_h * 4.0);
    let mut msgs = vec![StudioToApp::MouseDown(RemoteMouseDown {
        button_raw_bits: 1, x: hx, y: hy, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })];
    for i in 1 ..= 6 {
        let f = i as f64 / 6.0;
        msgs.push(StudioToApp::MouseMove(RemoteMouseMove {
            time: 0.0, x: hx + cell_w * f, y: hy + cell_h * f,
            modifiers: RemoteKeyModifiers::default(),
        }));
    }
    msgs.push(StudioToApp::MouseUp(RemoteMouseUp {
        button_raw_bits: 1, x: hx + cell_w, y: hy + cell_h, time: 0.0,
        modifiers: RemoteKeyModifiers::default(),
    }));
    app.forward(msgs);
    settle(&app, 10);
    app.locator(Selector::id("tile_title").text_exact("Calculator")).wait_visible();

    // Expand it, then ask for a split: pick mode puts home back on screen with
    // the app parked as a sliver — so now the stand-in is actually visible.
    app.locator(Selector::id("tile_expand")).wait_visible().click();
    settle(&app, 14);
    app.locator(Selector::id("split_button")).wait_visible().click();
    settle(&app, 12);
    app.locator(Selector::id("away_label")).wait_visible();

    // Tapping it brings the app home: the pick ends and the tile runs again.
    let away = app.locator(Selector::id("away_label")).snapshot();
    tap(
        &app,
        away.x as f64 + away.width as f64 * 0.5,
        away.y as f64 + away.height as f64 * 0.5,
    );
    settle(&app, 16);
    app.locator(Selector::id("away_label")).wait_hidden();
    app.locator(Selector::id("tile_title").text_exact("Calculator")).wait_visible();
    app.locator(Selector::all().text_exact("7")).wait_visible();
}

/// Closing a fullscreen app must leave NO close button on the home screen.
#[makepad_test]
fn closing_app_leaves_no_close_button(app: TestApp) {
    app.locator(Selector::id("name").text_exact("Clock")).wait_visible().click();
    app.locator(Selector::id("title").text_exact("Clock")).wait_visible();
    deny_prompt_if_shown(&app);
    deny_prompt_if_shown(&app);
    settle(&app, 14);
    app.locator(Selector::id("back_button")).wait_visible().click();
    settle(&app, 20);
    app.locator(Selector::id("back_button")).wait_hidden();
    // ...and it must not merely be flagged hidden: the bug this covers was a
    // button that reported hidden and kept PAINTING, because a GlassButton
    // draws into its own overlay draw list and MiniAppScreen::draw_walk
    // early-returns while Hidden without ever beginning its list, so nothing
    // flushed it. A snapshot cannot see that, which is exactly why the plain
    // assertion above stayed green through a bug you could see on screen.
    // What IS checkable is the structural fix: the × is a plain SDF View, so
    // it owns no overlay that could outlive it.
    let close_buttons: Vec<_> = app
        .widget_snapshot()
        .into_iter()
        .filter(|w| w.id == "back_button")
        .collect();
    assert!(!close_buttons.is_empty(), "no back_button in the tree at all");
    for w in &close_buttons {
        assert!(
            !w.widget_type.contains("GlassButton"),
            "back_button is glass again ({}): its overlay outlives the app and \
             hangs over the home screen after every close",
            w.widget_type
        );
    }
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

    // A finished run offers the two ways out of the console, and Open goes
    // straight into what was just built rather than making you find its icon.
    app.locator(Selector::id("create_done")).wait_visible();


    app.locator(Selector::id("create_open")).wait_visible().click();
    app.locator(Selector::all().text_exact("1500")).wait_visible();
    app.locator(Selector::id("back_button")).wait_visible().click();
    // Open also cleared the panel: the composer is back, console gone.
    app.locator(Selector::id("create_output")).wait_hidden();
    app.locator(Selector::id("create_input")).wait_visible();

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

    // The console keeps the WHOLE run, both turns of it. It used to show a
    // rolling 700-byte window of the current turn only, so the attempt that
    // failed to compile — the thing you'd actually want to look at — was gone
    // before the repair finished. The turn marker proves the earlier output
    // survived the boundary rather than being replaced by it.
    for _ in 0 .. 40 {
        app.locator(Selector::id("create_output")).scroll(0.0, 220.0);
    }
    app.locator(Selector::id("line").text_contains("repair 1")).wait_visible();
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
    app.locator(Selector::id("name").text_exact("Pomodoro")).wait_visible();

    // The finished console is still up, overlaying the top of the grid. A tap
    // outside only COLLAPSES it (and would be eaten doing so), so clear it the
    // one way that actually puts the composer back before reaching for the
    // icon underneath.
    app.locator(Selector::id("create_done")).wait_visible().click();
    app.locator(Selector::id("create_output")).wait_hidden();
    // "New prompt" hands the caret back, and a focused prompt opens the
    // options row — which means the composer is expanded and owns the next
    // press wherever it lands. Wait for that to be true rather than racing
    // it, then spend the press folding it away, so the long-press below
    // reaches the grid instead of being eaten as the dismissal.
    app.locator(Selector::id("create_options")).wait_visible();
    let bar = app.locator(Selector::id("create_bar")).snapshot();
    tap(&app, bar.x as f64 / 2.0, bar.y as f64 + bar.height as f64 / 2.0);
    app.locator(Selector::id("create_options")).wait_hidden();
    for _ in 0 .. 8 {
        let _ = app.widget_snapshot();
        std::thread::sleep(std::time::Duration::from_millis(60));
    }

    // Long-press its icon (grip point above the label, like other tests).
    let snap = app.locator(Selector::id("name").text_exact("Pomodoro")).snapshot();
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

/// The agent options (model / effort / thinking) are per-backend: a knob only
/// appears for a backend that can actually deliver it. The suite runs against
/// fake_acp — an arbitrary ACP binary — so the honest answer is no knobs at
/// all, and none of the three rows may appear. A control that silently does
/// nothing would be worse than no control.
///
/// The row they live in still shows, because it also carries the backend name
/// and the way in to the Providers page — those must never be unreachable.
#[makepad_test]
fn agent_knobs_stay_hidden_for_an_unconfigurable_backend(app: TestApp) {
    app.locator(Selector::id("create_input"))
        .wait_visible()
        .fill("a pomodoro timer");
    // Send appears, so the bar has definitely noticed the text...
    app.locator(Selector::id("create_send")).wait_visible();
    // ...but this agent exposes nothing to configure.
    app.locator(Selector::id("opt_0")).wait_hidden();
    app.locator(Selector::id("opt_1")).wait_hidden();
    app.locator(Selector::id("opt_2")).wait_hidden();
    // The way in to Providers is still there.
    app.locator(Selector::id("create_providers")).wait_visible();
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
    // The console is a virtualized list: only the lines ON SCREEN are widgets,
    // so "is the whole run kept?" is answered by scrolling to a line rather
    // than by asserting every line at once. That IS the feature — a test that
    // could see all of it without scrolling wouldn't be testing virtualization.
    //
    // The list tails, so what's on screen is the NEWEST end of the run — the
    // agent's thinking, which is all a reasoning model emits for the opening
    // stretch. Without it the panel sits on one unchanging line.
    app.locator(Selector::id("line").text_contains("Thinking")).wait_visible();

    // However tall the console gets, the BAR must stop clear of the dock.
    // The cap used to be measured against the console alone, so the
    // finished-run footer ("New prompt") hung past it — measured 24px INTO the
    // dock, over the favourites. The gap is kept below the whole bar now.
    let bar = app.locator(Selector::id("create_bar")).snapshot();
    let dock = app.locator(Selector::id("dock")).snapshot();
    let gap = dock.y as i64 - (bar.y as i64 + bar.height as i64);
    assert!(
        (20 ..= 40).contains(&gap),
        "the create bar must clear the dock by ~30px, got {gap}px \
         (bar {}..{}, dock starts {})",
        bar.y,
        bar.y as i64 + bar.height as i64,
        dock.y,
    );

    // A spinner says the run is live; it must be gone once it isn't.
    app.locator(Selector::id("create_spinner")).wait_visible();

    // The ✨ yields its slot to the chevron — exactly one is ever on screen.
    app.locator(Selector::id("create_glyph")).wait_hidden();

    // Scrolling back through a finished run is covered by
    // `create_bar_repairs_broken_app` rather than here: forty scroll events
    // outlast the run, and everything below this point needs it still live.

    // Hide → just the status line; show → console back.
    app.locator(Selector::id("create_toggle")).wait_visible().click();
    app.locator(Selector::id("create_output")).wait_hidden();
    app.locator(Selector::id("create_toggle")).click();
    app.locator(Selector::id("create_output")).wait_visible();

    // Stop asks first — it throws away a turn's work and the tokens are
    // already spent, and it sits where Open and Retry appear moments later, so
    // a mis-timed tap on a nearly-finished run used to destroy it.
    app.locator(Selector::id("create_cancel")).wait_visible().click();
    app.locator(Selector::all().text_contains("Stop the agent?")).wait_visible();
    // The run is still going while the sheet is up.
    app.locator(Selector::id("create_output")).wait_visible();
    app.locator(Selector::id("confirm_remove")).wait_visible().click();

    // Confirmed: the console goes with it and the ✨ comes back for the next
    // prompt.
    app.locator(Selector::id("create_output")).wait_hidden();
    app.locator(Selector::id("create_glyph")).wait_visible();
    app.locator(Selector::id("create_spinner")).wait_hidden();
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
    // To-Do lives in the label-less dock, so target its ✅ glyph — but only
    // once the app is really gone. Its own content carries a ✅ too, so while
    // the closing zoom is still on screen the selector matches two widgets and
    // refuses to resolve.
    app.locator(Selector::id("back_button")).wait_visible().click();
    app.locator(Selector::id("task_input")).wait_hidden();
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
    // Scroll to the history first: a section below the fold now reports
    // hidden (it used to report visible at 0x0), so asserting before scrolling
    // waits for something that is genuinely not on screen yet.
    scroll_app_info_to(&app, "vh_restore");
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
        .locator(Selector::id("create_prompt"))
        .wait_visible()
        .snapshot()
        .height;

    app.locator(Selector::id("create_input")).fill(
        "a habit tracker with three habits I can rename, a seven day streak \
         grid for each one, tap a day to toggle it done, a running total at \
         the top, and everything saved between launches",
    );

    let grown = app.locator(Selector::id("create_prompt")).snapshot().height;
    assert!(
        grown > one_line,
        "a long prompt should wrap and grow the field (was {one_line}, now {grown})"
    );

    let icon_after = app.locator(Selector::id("name").text_exact("News")).snapshot();
    assert_eq!(
        icon_after.y, icon_before.y,
        "the expanding prompt must overlay the grid, not push it down"
    );

    // Dismissing it folds the whole thing back to one line — the draft is kept
    // (the field goes single-line and scrolls sideways), but a tall panel must
    // not be left behind with nothing in it.
    let pager = app.locator(Selector::id("home_pager")).snapshot();
    tap(
        &app,
        pager.x as f64 + pager.width as f64 * 0.5,
        pager.y as f64 + pager.height as f64 * 0.85,
    );
    for _ in 0 .. 6 {
        let _ = app.widget_snapshot();
        std::thread::sleep(std::time::Duration::from_millis(60));
    }
    // The field keeps the text and stays the same widget — collapsing clamps
    // its row count to one — so the slot is back to its resting height.
    let collapsed = app.locator(Selector::id("create_prompt")).snapshot().height;
    assert_eq!(
        collapsed, one_line,
        "dismissing the composer should collapse it back to a single line \
         (was {grown} tall)"
    );

    // ...and COLLAPSING IS NOT CLEARING. Tapping away from the bar is an
    // ordinary way to look at something else; a draft that evaporates because
    // the field lost focus is lost work. Only Done empties it.
    //
    // Asserted through the height rather than the text because the harness
    // reports `None` for a TextInput's content — but the height is proof
    // enough: the field only grows back to the multi-line size if the draft
    // is still in it. A cleared field would come back one line tall.
    app.locator(Selector::id("create_input")).click();
    for _ in 0 .. 6 {
        let _ = app.widget_snapshot();
        std::thread::sleep(std::time::Duration::from_millis(60));
    }
    assert_eq!(
        app.locator(Selector::id("create_prompt")).snapshot().height,
        grown,
        "refocusing must bring the draft back — losing it to a tap outside is lost work"
    );
}

/// The empty band BELOW an app's label belongs to the background, not to the
/// icon. It used to belong to the icon — the whole grid cell did — which meant
/// long-pressing anywhere near a row opened that app's menu instead of
/// entering jiggle mode, and the gaps between rows were unusable.
#[makepad_test]
fn empty_space_below_a_label_is_background_not_the_icon(app: TestApp) {
    let icon = app.locator(Selector::id("name").text_exact("News")).wait_visible().snapshot();
    // Just under the label: still inside the icon's grid cell, but past the
    // icon+label group. A long-press here must reach the background handler.
    let (x, y) = (
        icon.x as f64 + icon.width as f64 / 2.0,
        (icon.y + icon.height) as f64 + 14.0,
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
    // Background long-press enters edit mode; an icon long-press would have
    // opened its context menu instead.
    app.locator(Selector::id("badge")).wait_visible();
    app.locator(Selector::all().text_exact("Remove from Home")).wait_hidden();
}

/// Long-press an icon by its name label (the tile sits just above it) and open
/// the App Info page from the menu.
fn open_app_info(app: &TestApp, name: &str) {
    let snap = app
        .locator(Selector::id("name").text_exact(name))
        .wait_visible()
        .snapshot();
    let (x, y) = (snap.x as f64 + snap.width as f64 / 2.0, snap.y as f64 - 24.0);
    app.forward(vec![StudioToApp::MouseDown(RemoteMouseDown {
        button_raw_bits: 1, x, y, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })]);
    // Pump past the 0.5s long-press threshold (a bare sleep wouldn't advance
    // the headless app's timers).
    for _ in 0 .. 9 {
        let _ = app.widget_snapshot();
        std::thread::sleep(std::time::Duration::from_millis(90));
    }
    app.forward(vec![StudioToApp::MouseUp(RemoteMouseUp {
        button_raw_bits: 1, x, y, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })]);
    app.locator(Selector::all().text_contains("App Info")).wait_visible().click();
}

/// App Info → View opens the source in a code popup ON TOP of the page (its ×
/// goes back to App Info rather than to the home screen), and the popup is
/// showing the app it was opened from.
#[makepad_test]
fn app_info_view_source_opens_a_code_popup(app: TestApp) {
    open_app_info(&app, "Calculator");
    scroll_app_info_to(&app, "ai_view_source");
    app.locator(Selector::all().text_exact("App code")).wait_visible();
    app.locator(Selector::id("ai_view_source")).wait_visible().click();

    // The popup identifies the app, and the code view is up.
    app.locator(Selector::id("sm_title").text_exact("Calculator")).wait_visible();
    app.locator(Selector::id("sm_subtitle").text_contains("built-in")).wait_visible();
    app.locator(Selector::id("sm_code")).wait_visible();

    // Closing it returns to App Info, which is still open behind it.
    app.locator(Selector::id("sm_close")).wait_visible().click();
    app.locator(Selector::id("sm_code")).wait_hidden();
    app.locator(Selector::all().text_exact("Saved data")).wait_visible();
}

/// Export writes a shareable bundle, and Import installs it back: the exported
/// app appears in the Import list and installing it puts a SECOND copy on the
/// home screen (a fresh id — importing never overwrites what you have).
#[makepad_test]
fn export_then_import_installs_a_second_copy(app: TestApp) {
    open_app_info(&app, "Calculator");
    scroll_app_info_to(&app, "ai_export");
    app.locator(Selector::id("ai_export")).wait_visible().click();
    // The hint reports the file it wrote, so the export definitely landed.
    app.locator(Selector::id("ai_export_hint").text_contains("calculator.splashapp"))
        .wait_visible();
    app.locator(Selector::id("ai_close")).wait_visible().click();

    // Right-click empty space → Import App…
    let snap = app.locator(Selector::id("home_pager")).wait_visible().snapshot();
    let x = snap.x as f64 + snap.width as f64 / 2.0;
    let y = snap.y as f64 + snap.height as f64 * 0.9;
    app.forward(vec![StudioToApp::MouseDown(RemoteMouseDown {
        button_raw_bits: 2, x, y, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })]);
    app.forward(vec![StudioToApp::MouseUp(RemoteMouseUp {
        button_raw_bits: 2, x, y, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })]);
    app.locator(Selector::all().text_exact("Import App…")).wait_visible().click();

    // The export shows up as an installable row, read out of the file itself.
    app.locator(Selector::all().text_exact("Import App")).wait_visible();
    app.locator(Selector::id("imp_sub").text_exact("calculator.splashapp")).wait_visible();
    app.locator(Selector::id("imp_action").nth(0)).wait_visible().click();

    // Two Calculators now — the import got its own id rather than replacing
    // the built-in.
    app.locator(Selector::id("name").text_exact("Calculator").nth(1)).wait_visible();
}

/// A generation whose repair budget runs out offers Retry in the bar, and
/// pressing it runs the same request again without it being retyped. The fake
/// agent's "hopeless" scenario never produces a script that compiles, so both
/// runs end the same way — what's under test is the affordance and the re-run,
/// not the outcome.
#[makepad_test]
fn failed_generation_offers_an_inline_retry(app: TestApp) {
    app.locator(Selector::id("create_input"))
        .wait_visible()
        .fill("a hopeless app");
    submit_prompt(&app);

    // Repairs exhausted: the console says so and Retry appears in Stop's place.
    app.locator(Selector::id("create_status").text_contains("kept failing")).wait_visible();
    app.locator(Selector::id("create_retry")).wait_visible();
    app.locator(Selector::id("create_cancel")).wait_hidden();

    // Pressing it starts a fresh run — Stop is back, so a generation is
    // genuinely in flight and not just the old console still on screen...
    app.locator(Selector::id("create_retry")).click();
    app.locator(Selector::id("create_cancel")).wait_visible();
    // ...and it fails the same way, offering the retry again.
    app.locator(Selector::id("create_cancel")).wait_hidden();
    app.locator(Selector::id("create_retry")).wait_visible();

    // A press outside COLLAPSES the console — it keeps the log and the Retry
    // offer, it does not throw them away. To the LEFT of the bar, not below
    // it: a failed console is tall enough to outgrow the pager entirely (748
    // vs 620 in this window), so there is no point under it that isn't the
    // dock, and tapping the dock opens an app instead.
    let bar = app.locator(Selector::id("create_bar")).snapshot();
    tap(&app, bar.x as f64 / 2.0, bar.y as f64 + bar.height as f64 / 2.0);
    app.locator(Selector::id("create_output")).wait_hidden();
    // The chevron brings it back, proving nothing was destroyed...
    app.locator(Selector::id("create_toggle")).wait_visible().click();
    app.locator(Selector::id("create_output")).wait_visible();
    app.locator(Selector::id("create_retry")).wait_visible();
    // ...and "New prompt" is the one thing that does tear it down.
    app.locator(Selector::id("create_done")).wait_visible().click();
    app.locator(Selector::id("create_input")).wait_visible();
    app.locator(Selector::id("create_output")).wait_hidden();
    // It also hands the caret back, so the next prompt can just be typed.
    // Proved by TYPING — no click first, no locator, the keystrokes go
    // wherever key focus happens to be. The harness reports `None` for a
    // TextInput's content, so the observable is Send: it appears only when
    // the field is non-empty, which it can only become if the caret is in it.
    app.locator(Selector::id("create_send")).wait_hidden();
    app.type_text("next one");
    app.locator(Selector::id("create_send")).wait_visible();
}



/// The Providers page is the one place credentials live: it lists every
/// provider with its state, adding one asks for the key against a NAMED
/// provider, and the field is masked until you ask to see it.
#[makepad_test]
fn providers_page_adds_a_key_behind_a_masked_field(app: TestApp) {
    // The ✨ is one way in (the create bar's "＋ Providers" is the other).
    app.locator(Selector::id("create_glyph")).wait_visible().click();
    app.locator(Selector::all().text_exact("AI Providers")).wait_visible();
    // Every known provider is listed with what it needs, so adding one is a
    // choice rather than a guess about key formats.
    app.locator(Selector::all().text_exact("Anthropic (Claude)")).wait_visible();
    app.locator(Selector::all().text_exact("Kimi (Coding Plan)")).wait_visible();

    // The page also says HOW a run executes, which nothing else on it reveals:
    // the rows name the service and where its key came from, and none of that
    // distinguishes a child process from an agent compiled into this binary.
    // The suite runs under HOST_LAUNCHER_AGENT_CMD, so that is what it must
    // report — naming the override rather than the `octos acp` it isn't using.
    app.locator(Selector::id("pv_runtime").text_contains("child process")).wait_visible();
    app.locator(Selector::id("pv_runtime").text_contains("HOST_LAUNCHER_AGENT_CMD"))
        .wait_visible();

    // The key field only appears once a provider is chosen, and it names the
    // provider it belongs to. WHICH row is first depends on what's already
    // configured (the suite shares one config dir), so this asserts the
    // mechanism, not a particular provider — `providers::tests` pins the
    // per-provider behaviour exactly.
    app.locator(Selector::id("pv_key_input")).wait_hidden();
    // Adding lives in the row's left gutter now (a circular +), not in a
    // right-hand "Add" button — `pr_action` is only ever "Edit" on a row that
    // already has a key.
    app.locator(Selector::id("pr_add").nth(0)).wait_visible().click();
    app.locator(Selector::id("pv_key_input")).wait_visible();
    app.locator(Selector::id("pv_entry_title").text_contains("Key for")).wait_visible();

    // Masked by default; the eye reveals and swaps which eye is offered.
    app.locator(Selector::id("pv_key_show")).wait_visible();
    app.locator(Selector::id("pv_key_hide")).wait_hidden();
    app.locator(Selector::id("pv_key_show")).click();
    app.locator(Selector::id("pv_key_hide")).wait_visible();

    // An empty field is refused rather than saved.
    app.locator(Selector::all().text_exact("Save")).wait_visible().click();
    app.locator(Selector::id("pv_status").text_contains("Paste a key")).wait_visible();

    // A plain `sk-` key is shared by several providers, so it's never
    // second-guessed: it saves, and the bar says the provider is ready.
    app.locator(Selector::id("pv_key_input")).wait_visible().fill("sk-plain-test-key");
    app.locator(Selector::all().text_exact("Save")).click();
    app.locator(Selector::all().text_contains("ready")).wait_visible();
}

/// The prompt only offers provider setup when there ISN'T one. Running under
/// HOST_LAUNCHER_AGENT_CMD — which is how a Claude subscription is used, and
/// how this whole suite runs — is a complete setup that needs no key and no
/// octos config, so clicking the prompt must just open the options row.
/// Getting this wrong nags a launcher that works perfectly.
#[makepad_test]
fn a_launcher_with_an_agent_command_is_not_nagged_for_a_provider(app: TestApp) {
    app.locator(Selector::id("create_input")).wait_visible().click();
    app.locator(Selector::id("create_options")).wait_visible();
    app.locator(Selector::all().text_exact("AI Providers")).wait_hidden();
}

/// ...and with a working setup, the Providers page must NOT be shouting about
/// something being missing. The blocker banner is deliberately loud — it sits
/// above every provider row — so a false positive would be the first thing a
/// perfectly configured user sees.
#[makepad_test]
fn a_working_setup_shows_no_blocker_banner(app: TestApp) {
    app.locator(Selector::id("create_glyph")).wait_visible().click();
    app.locator(Selector::all().text_exact("AI Providers")).wait_visible();
    app.locator(Selector::id("pv_blocker")).wait_hidden();
    app.locator(Selector::all().text_contains("isn't installed")).wait_hidden();
}

/// ...and that agent is listed as the thing in use, with the note explaining
/// why the key rows below it are dormant.
#[makepad_test]
fn the_agent_command_is_shown_as_the_provider_in_use(app: TestApp) {
    app.locator(Selector::id("create_glyph")).wait_visible().click();
    app.locator(Selector::all().text_exact("AI Providers")).wait_visible();
    app.locator(Selector::id("pv_note").text_contains("HOST_LAUNCHER_AGENT_CMD"))
        .wait_visible();
    app.locator(Selector::id("pr_detail").text_contains("In use")).wait_visible();
}



// ---------------------------------------------------------------------------
// Screenshot sweeps (visual review, not regression tests)
// ---------------------------------------------------------------------------

/// Resizes the (headless) window and pumps a few frames so layout — and the
/// queued `on_app_resize` script notifications — settle before asserting or
/// screenshotting.
fn resize_window(app: &TestApp, w: f64, h: f64) {
    app.forward(vec![StudioToApp::WindowGeomChange {
        dpi_factor: 1.0,
        window_id: 0,
        left: 0.0,
        top: 0.0,
        width: w,
        height: h,
    }]);
    settle(app, 5);
}

/// Pumps the app's event loop `n` frames (snapshot + a beat of real time).
/// Scrolls App Info until `id` is really WITHIN the visible body, rather than
/// merely present. A widget below the fold still reports visible and a click
/// on it lands on nothing, and this page has grown twice now — so a test that
/// clicks low on it should ask for its target instead of guessing a pixel
/// amount that the next section invalidates.
fn scroll_app_info_to(app: &TestApp, id: &str) {
    // Ask the SNAPSHOT whether the target is on screen rather than waiting on
    // a locator: a widget scrolled out of the body reports hidden, so
    // `wait_visible` on it would block until the timeout instead of telling us
    // to scroll. (It used to report visible at 0x0, which was worse — a click
    // then landed on nothing.)
    // FULLY inside, not merely touching: a row scrolled to the bottom edge
    // reports visible while the button inside it is still clipped, and the
    // click then lands on nothing.
    let fully_in_view = |app: &TestApp| {
        let snap = app.widget_snapshot();
        let Some(body) = snap.iter().find(|w| w.id == "ai_body") else {
            return false;
        };
        let Some(t) = snap.iter().find(|w| w.id == id && w.visible && w.height > 0) else {
            return false;
        };
        let (top, bottom) = (body.y, body.y + body.height);
        // A CLIPPED widget reports its clipped height, so "its box fits" is
        // true for one sitting exactly on the bottom edge with its contents
        // cut off. Insist on a row's worth of room below it instead.
        let room = t.height.max(96);
        t.y >= top && t.y + room <= bottom
    };
    for _ in 0..14 {
        if fully_in_view(app) {
            return;
        }
        app.locator(Selector::id("ai_body")).scroll(0.0, 200.0);
        settle(app, 3);
    }
}

fn settle(app: &TestApp, n: usize) {
    for _ in 0..n {
        let _ = app.widget_snapshot();
        std::thread::sleep(std::time::Duration::from_millis(40));
    }
}

/// Answers a pending permission launch prompt with "Don't Allow", when one is
/// up. For flows that open a network-declaring app (clock, news, weather) but
/// aren't themselves about permissions: denied apps keep their offline demo
/// content, which is exactly what those tests assert against.
fn deny_prompt_if_shown(app: &TestApp) {
    settle(app, 2);
    if app.locator(Selector::id("perm_deny")).count() > 0 {
        app.locator(Selector::id("perm_deny")).click();
        settle(app, 2);
    }
}

/// Saves the current frame under `name` in $HOST_LAUNCHER_SHOTS.
fn snap(app: &TestApp, out: &std::path::Path, name: &str) {
    let src = app.screenshot();
    let _ = std::fs::copy(&src, out.join(format!("{name}.png")));
}

/// Opens every installed mini app at several window sizes and saves
/// screenshots for visual review. Skipped unless HOST_LAUNCHER_SHOTS (an
/// output directory) is set, so normal suite runs don't pay for it.
/// HOST_LAUNCHER_SHOT_APPS=Calculator,Clock filters which apps run.
#[makepad_test]
fn screenshot_sweep(app: TestApp) {
    let Ok(dir) = std::env::var("HOST_LAUNCHER_SHOTS") else { return };
    let out = std::path::PathBuf::from(dir);
    let _ = std::fs::create_dir_all(&out);
    let filter: Vec<String> = std::env::var("HOST_LAUNCHER_SHOT_APPS")
        .map(|s| s.split(',').map(|a| a.trim().to_string()).collect())
        .unwrap_or_default();
    // Narrow ~ a left-right split pane; short ~ a top-bottom pane; wide ~ a
    // desktop window. The phone shape last so each app closes from a known
    // geometry.
    let sizes: [(f64, f64, &str); 4] = [
        (1280.0, 800.0, "wide"),
        (250.0, 700.0, "narrow"),
        (500.0, 400.0, "short"),
        (420.0, 860.0, "phone"),
    ];
    let apps = [
        "Calculator", "Calendar", "Clock", "Counter", "Gallery", "Sandbox",
        "Music", "News", "Notes", "Settings", "Stopwatch", "To-Do", "Weather",
    ];
    for name in apps {
        if !filter.is_empty() && !filter.iter().any(|f| f == name) {
            continue;
        }
        // Open from the drawer via its search filter — works for every app
        // regardless of which home page (if any) carries its icon.
        app.locator(Selector::id("home_pager"))
            .wait_visible()
            .drag_by(0.0, -250.0);
        app.locator(Selector::id("search_input")).wait_visible().fill(name);
        app.locator(Selector::id("d_name").text_exact(name))
            .wait_visible()
            .click();
        app.locator(Selector::id("title").text_exact(name)).wait_visible();
        deny_prompt_if_shown(&app);
        settle(&app, 12);
        for (w, h, tag) in sizes {
            resize_window(&app, w, h);
            snap(&app, &out, &format!("{}_{}_{}x{}", name, tag, w as i64, h as i64));
        }
        app.locator(Selector::id("back_button")).wait_visible().click();
        app.locator(Selector::id("title").text_exact(name)).wait_hidden();
        settle(&app, 3);
    }
}

/// Walks the whole split-screen flow — dock via the header split button, pick
/// the second app, drag the divider, open its menu, swap, rotate, collapse —
/// saving a screenshot at each step. Gated like `screenshot_sweep`.
#[makepad_test]
fn screenshot_split_sweep(app: TestApp) {
    let Ok(dir) = std::env::var("HOST_LAUNCHER_SHOTS") else { return };
    let out = std::path::PathBuf::from(dir);
    let _ = std::fs::create_dir_all(&out);

    // Clock fullscreen, then dock it to the top pane (portrait -> top/bottom).
    app.locator(Selector::id("name").text_exact("Clock"))
        .wait_visible()
        .click();
    app.locator(Selector::id("title").text_exact("Clock")).wait_visible();
    deny_prompt_if_shown(&app);
    settle(&app, 12);
    app.locator(Selector::id("split_button")).wait_visible().click();
    settle(&app, 12);
    snap(&app, &out, "split_1_pick");

    // Pick the calculator from the visible home half.
    app.locator(Selector::id("name").text_exact("Calculator"))
        .wait_visible()
        .click();
    app.locator(Selector::id("title").text_exact("Calculator")).wait_visible();
    settle(&app, 14);
    snap(&app, &out, "split_2_both");

    // Drag the divider down: clock grows, calculator shrinks.
    let div = app.locator(Selector::id("divider_pill")).wait_visible().snapshot();
    let cx = div.x as f64 + div.width as f64 * 0.5;
    let cy = div.y as f64 + div.height as f64 * 0.5;
    drag(&app, (cx, cy), (cx, cy + 150.0));
    settle(&app, 8);
    snap(&app, &out, "split_3_dragged");

    // Tap the divider for its menu, then swap the panes.
    let div = app.locator(Selector::id("divider_pill")).snapshot();
    tap(
        &app,
        div.x as f64 + div.width as f64 * 0.5,
        div.y as f64 + div.height as f64 * 0.5,
    );
    settle(&app, 4);
    snap(&app, &out, "split_4_menu");
    app.locator(Selector::id("menu_swap")).wait_visible().click();
    settle(&app, 12);
    snap(&app, &out, "split_5_swapped");

    // Rotate to a left-right split.
    let div = app.locator(Selector::id("divider_pill")).snapshot();
    tap(
        &app,
        div.x as f64 + div.width as f64 * 0.5,
        div.y as f64 + div.height as f64 * 0.5,
    );
    app.locator(Selector::id("menu_rotate")).wait_visible().click();
    settle(&app, 12);
    snap(&app, &out, "split_6_rotated");

    // Drag the divider almost all the way left: the smaller side closes and
    // the survivor goes fullscreen.
    let div = app.locator(Selector::id("divider_pill")).snapshot();
    let cx = div.x as f64 + div.width as f64 * 0.5;
    let cy = div.y as f64 + div.height as f64 * 0.5;
    drag(&app, (cx, cy), (30.0, cy));
    settle(&app, 14);
    snap(&app, &out, "split_7_collapsed");
}


// ---------------------------------------------------------------------------
// Resizable hosts + split screen
// ---------------------------------------------------------------------------

/// A mini app reflows when the window changes size: the calculator swaps to
/// its compact tier in a short window (its `on_app_resize` hook fired), back
/// to the full tier when room returns, and its column is width-capped and
/// centered in a wide window.
#[makepad_test]
fn mini_app_adapts_to_window_size(app: TestApp) {
    app.locator(Selector::id("name").text_exact("Calculator"))
        .wait_visible()
        .click();
    app.locator(Selector::id("display").text_exact("0")).wait_visible();
    settle(&app, 8);
    app.locator(Selector::id("keypad")).wait_visible();
    app.locator(Selector::id("keypad_sm")).wait_hidden();

    // Short window -> compact keypad.
    resize_window(&app, 500.0, 400.0);
    app.locator(Selector::id("keypad_sm")).wait_visible();
    app.locator(Selector::id("keypad")).wait_hidden();

    // Wide window -> full tier again, capped at 520 and centered.
    resize_window(&app, 1280.0, 800.0);
    app.locator(Selector::id("keypad")).wait_visible();
    app.locator(Selector::id("keypad_sm")).wait_hidden();
    let col = app.locator(Selector::id("col")).wait_visible().snapshot();
    assert!(
        col.width as f64 <= 521.0,
        "column should cap at 520, got {}",
        col.width
    );
    let center = col.x as f64 + col.width as f64 * 0.5;
    assert!(
        (center - 640.0).abs() < 30.0,
        "column should be centered (~640), center at {center}"
    );

    resize_window(&app, 420.0, 860.0);
    app.locator(Selector::id("back_button")).wait_visible().click();
    app.locator(Selector::id("display")).wait_hidden();
}

/// The whole split-screen story, Android-style: dock via the header split
/// button, pick the partner from the live home screen, drag the divider (both
/// panes resize; the small pane's app flips to its compact tier), swap the
/// panes from the divider menu, rotate the split axis, and drag the divider
/// to an edge to close the smaller side.
#[makepad_test]
fn split_screen_full_flow(app: TestApp) {
    // Clock fullscreen; home is hidden behind it.
    app.locator(Selector::id("name").text_exact("Clock"))
        .wait_visible()
        .click();
    app.locator(Selector::id("title").text_exact("Clock")).wait_visible();
    deny_prompt_if_shown(&app);
    settle(&app, 14);
    app.locator(Selector::id("name").text_exact("Calculator")).wait_hidden();

    // Dock it: pick mode leaves home live to choose the second app.
    app.locator(Selector::id("split_button")).wait_visible().click();
    settle(&app, 10);
    app.locator(Selector::id("name").text_exact("Calculator")).wait_visible();

    // Pick the calculator -> split, divider between the panes.
    app.locator(Selector::id("name").text_exact("Calculator")).click();
    app.locator(Selector::id("title").text_exact("Calculator")).wait_visible();
    settle(&app, 12);
    let div = app.locator(Selector::id("divider_pill")).wait_visible().snapshot();
    let (cx0, cy0) = (
        div.x as f64 + div.width as f64 * 0.5,
        div.y as f64 + div.height as f64 * 0.5,
    );
    assert!((cy0 - 430.0).abs() < 25.0, "divider should start centered, y {cy0}");

    // Drag down: clock grows, calculator shrinks and goes compact. The
    // release lands near 2/3 and magnets onto it.
    drag(&app, (cx0, cy0), (cx0, cy0 + 150.0));
    settle(&app, 10);
    let div = app.locator(Selector::id("divider_pill")).snapshot();
    let cy1 = div.y as f64 + div.height as f64 * 0.5;
    assert!(
        (558.0..580.0).contains(&cy1),
        "divider should snap to 2/3 (~569), y {cy1}"
    );
    app.locator(Selector::id("keypad_sm")).wait_visible();

    // Tap the divider -> menu; Swap exchanges the panes.
    let clock_y_before = app
        .locator(Selector::id("title").text_exact("Clock"))
        .snapshot()
        .y as f64;
    tap(&app, cx0, cy1);
    app.locator(Selector::id("menu_swap")).wait_visible().click();
    settle(&app, 10);
    let clock_y_after = app
        .locator(Selector::id("title").text_exact("Clock"))
        .snapshot()
        .y as f64;
    assert!(
        clock_y_after > clock_y_before + 100.0,
        "swap should move Clock to the lower pane ({clock_y_before} -> {clock_y_after})"
    );

    // Rotate: the divider strip turns vertical (left-right split).
    let div = app.locator(Selector::id("divider_pill")).snapshot();
    tap(
        &app,
        div.x as f64 + div.width as f64 * 0.5,
        div.y as f64 + div.height as f64 * 0.5,
    );
    app.locator(Selector::id("menu_rotate")).wait_visible().click();
    settle(&app, 10);
    let div = app.locator(Selector::id("divider_pill")).snapshot();
    assert!(
        div.height > div.width,
        "after rotate the divider should be vertical ({} x {})",
        div.width,
        div.height
    );

    // Drag the divider almost to the left edge: the smaller side (the
    // calculator) closes and the clock goes fullscreen.
    let (cx2, cy2) = (
        div.x as f64 + div.width as f64 * 0.5,
        div.y as f64 + div.height as f64 * 0.5,
    );
    drag(&app, (cx2, cy2), (30.0, cy2));
    settle(&app, 12);
    app.locator(Selector::id("divider_pill")).wait_hidden();
    app.locator(Selector::id("display")).wait_hidden();
    app.locator(Selector::id("title").text_exact("Clock")).wait_visible();

    // Back (Escape) closes the surviving app to home.
    app.press_key(makepad_test::KeyCode::Escape);
    app.locator(Selector::id("title").text_exact("Clock")).wait_hidden();
    app.locator(Selector::id("name").text_exact("Calculator")).wait_visible();
}

/// Abandoning a pick: the header split button docks the app, and pressing it
/// again (or closing the pane) restores fullscreen with home hidden again.
#[makepad_test]
fn split_pick_cancels_back_to_fullscreen(app: TestApp) {
    app.locator(Selector::id("name").text_exact("Clock"))
        .wait_visible()
        .click();
    app.locator(Selector::id("title").text_exact("Clock")).wait_visible();
    deny_prompt_if_shown(&app);
    settle(&app, 14);
    app.locator(Selector::id("split_button")).wait_visible().click();
    settle(&app, 10);
    app.locator(Selector::id("name").text_exact("Calculator")).wait_visible();
    // The create bar hides while picking so its rect can't shadow the grid.
    app.locator(Selector::id("create_bar")).wait_hidden();
    // Widget tiles hide too: a glass tile composites its whole subtree above
    // the main pass, so a drawn tile would float IN FRONT of the docked app
    // (the weather widget literally covered the app before this rule).
    app.locator(Selector::all().text_exact("San Francisco")).wait_hidden();
    // The docked app is SHIFTED nearly offscreen (fullscreen-sized, only a
    // PICK_PEEK sliver at the top edge), so even the top grid rows are
    // pickable. Its header — split button included — hangs offscreen;
    // tapping the sliver is what cancels the pick.
    let pager = app.locator(Selector::id("home_pager")).wait_visible().snapshot();
    tap(&app, pager.x as f64 + pager.width as f64 * 0.5, 16.0);
    settle(&app, 10);
    app.locator(Selector::id("name").text_exact("Calculator")).wait_hidden();
    app.locator(Selector::id("title").text_exact("Clock")).wait_visible();
    // The bar stays hidden while the (fullscreen) app covers home; it comes
    // back once the app is closed.
    app.locator(Selector::id("back_button")).wait_visible().click();
    app.locator(Selector::id("title").text_exact("Clock")).wait_hidden();
    app.locator(Selector::id("create_bar")).wait_visible();
    // Home again: the widget tiles come back.
    app.locator(Selector::all().text_exact("San Francisco")).wait_visible();
}

// ---------------------------------------------------------------------------
// Permissions + host services (docs/PERMISSIONS.md)
// ---------------------------------------------------------------------------

/// Opening an app that declares `network` (still in Ask) shows the runtime
/// prompt; allowing it restarts the app connected, which then asks for
/// location the same way — and with both grants the weather app fetches REAL
/// data end-to-end (script `net.http_request` → platform HTTP → the test-run
/// fake server → response routed back into the isolate).
#[makepad_test]
fn weather_prompts_then_fetches_live_data(app: TestApp) {
    app.locator(Selector::id("glyph").text_exact("🌤"))
        .wait_visible()
        .click();
    // The launch prompt for network floats over the (netless) app.
    app.locator(Selector::id("perm_title").text_contains("Network"))
        .wait_visible();
    app.locator(Selector::id("perm_allow")).wait_visible().click();
    // The restarted, connected app now asks for location before fetching.
    app.locator(Selector::id("perm_title").text_contains("Location"))
        .wait_visible();
    app.locator(Selector::id("perm_allow")).wait_visible().click();
    // Live data landed: the fake server's geolocation city, uppercased.
    app.locator(Selector::all().text_contains("TESTVILLE"))
        .wait_visible();
}

/// Denying the launch prompt keeps the app fully usable on its offline demo
/// data, and the denial STICKS: reopening does not ask again.
#[makepad_test]
fn denied_network_keeps_weather_sandboxed(app: TestApp) {
    app.locator(Selector::id("glyph").text_exact("🌤"))
        .wait_visible()
        .click();
    app.locator(Selector::id("perm_title").text_contains("Network"))
        .wait_visible();
    app.locator(Selector::id("perm_deny")).wait_visible().click();
    // Offline fallback renders the demo city; nothing fetched.
    app.locator(Selector::id("subtitle").text_contains("SAN FRANCISCO")).wait_visible();
    // Close and reopen: Denied is persisted, so no second ask.
    app.locator(Selector::id("back_button")).wait_visible().click();
    app.locator(Selector::id("subtitle").text_contains("SAN FRANCISCO")).wait_hidden();
    app.locator(Selector::id("glyph").text_exact("🌤"))
        .wait_visible()
        .click();
    app.locator(Selector::id("subtitle").text_contains("SAN FRANCISCO")).wait_visible();
    app.locator(Selector::id("perm_title")).wait_hidden();
}

/// App Info's PERMISSIONS section shows every declared permission with its
/// state, and tapping a row's button flips the grant both ways.
#[makepad_test]
fn app_info_lists_and_toggles_permissions(app: TestApp) {
    open_app_info(&app, "News");
    app.locator(Selector::all().text_exact("PERMISSIONS")).wait_visible();
    // News declares network + open-url + notifications: a runtime tier
    // (asks) and a normal tier (auto-granted while declared).
    app.locator(Selector::id("pr_title").text_exact("Network")).wait_visible();
    app.locator(Selector::id("pr_title").text_exact("Open Links")).wait_visible();
    // Rows render in declaration order and News declares network first, so
    // the first visible toggle is Network's. Runtime tier defaults to Ask;
    // granting then revoking round-trips through all three states.
    app.locator(Selector::id("pr_state").nth(0)).wait_visible().wait_text("Asks");
    app.locator(Selector::id("pr_toggle").nth(0)).click();
    app.locator(Selector::id("pc_allow")).wait_visible().click();
    app.locator(Selector::id("pr_state").nth(0)).wait_text("Allowed");
    app.locator(Selector::id("pr_toggle").nth(0)).click();
    app.locator(Selector::id("pc_deny")).wait_visible().click();
    app.locator(Selector::id("pr_state").nth(0)).wait_text("Blocked");
}

/// With the network grant in place, the news app fetches live headlines from
/// the fake wire and drops its canned ones.
#[makepad_test]
fn news_goes_live_after_grant(app: TestApp) {
    app.locator(Selector::id("name").text_exact("News"))
        .wait_visible()
        .click();
    app.locator(Selector::id("perm_title").text_contains("Network"))
        .wait_visible();
    app.locator(Selector::id("perm_allow")).wait_visible().click();
    app.locator(Selector::all().text_contains("Fake wire headline 1"))
        .wait_visible();
}

/// The sandbox probe app declares NOTHING, and proves deny-by-default from
/// the inside: stripped modules, no net runtime, refused host services, and
/// the newly-neutered `cx.quit`. Every probe must read ✅ in the stock state.
#[makepad_test]
fn sandbox_probe_shows_deny_by_default(app: TestApp) {
    app.locator(Selector::id("home_pager"))
        .wait_visible()
        .drag_by(0.0, -250.0);
    app.locator(Selector::id("d_name").text_exact("Sandbox"))
        .wait_visible()
        .click();
    app.locator(Selector::all().text_contains("cx.quit")).wait_visible();
    // Every static probe reads ✅ (a ❌-count would false-positive on the
    // Splash widget's own text, which is the SOURCE and contains ❌ literals).
    app.locator(Selector::id("svc_status_w").text_exact("✅ DENIED")).wait_visible();
    app.locator(Selector::all().text_exact("✅ NEUTERED")).wait_visible();
    app.locator(Selector::all().text_exact("✅ NONE GRANTED")).wait_visible();
}

/// Permissions answer "may it"; the request budget answers "how often". The
/// probe's flood button fires 80 requests at a service that needs no
/// permission at all — the launcher must still refuse the tail of the burst,
/// which is the whole defense against an app that simply will not stop
/// asking.
#[makepad_test]
fn rate_limit_refuses_a_flood_of_host_requests(app: TestApp) {
    open_sandbox_probe(&app);
    // Untested until asked: the row only reports a real burst.
    app.locator(Selector::id("flood_status_w").text_exact("— UNTESTED")).wait_visible();
    app.locator(Selector::id("flood_btn")).wait_visible().click();
    // "REFUSED n/80" — the count varies with how the burst lands across
    // frames, so assert the verdict, not the arithmetic.
    app.locator(Selector::id("flood_status_w").text_contains("✅ REFUSED")).wait_visible();
    // One burst is a single strike out of four: the app is refused, not
    // stopped, and stays on screen.
    app.locator(Selector::id("svc_status_w")).wait_visible();
}

/// Taps the sandbox probe's drawer entry, opening the drawer first only if it
/// isn't already showing. Force-stopping an app can leave the drawer open
/// behind it, which hides the home pager — so "swipe up, then tap" is not a
/// safe assumption once the launcher has stopped something.
/// Opens a mini-app from its home icon and waits for the open ZOOM to settle.
/// A click during that animation lands where the widget will be rather than
/// where it is, and it fails much later and somewhere else — as "the app
/// ignored the button", which reads like a broken app rather than a race.
fn open_mini_app(app: &TestApp, glyph: &str, ready: &str) {
    app.locator(Selector::id("glyph").text_exact(glyph))
        .wait_visible()
        .click();
    app.locator(Selector::id(ready)).wait_visible();
    settle(app, 6);
}

fn open_sandbox_probe(app: &TestApp) {
    // Always swipe up first. Stopping an app leaves the launcher on home with
    // the drawer closed, and a closed drawer's rows keep reporting `visible`
    // at 0x0 — so "tap it if it looks visible" clicks empty wallpaper and the
    // test fails three steps later with no clue why.
    app.locator(Selector::id("home_pager"))
        .wait_visible()
        .drag_by(0.0, -250.0);
    app.locator(Selector::id("d_name").text_exact("Sandbox"))
        .wait_visible()
        .click();
    // Let the open ZOOM finish. A click during it lands where the button will
    // be rather than where it is, and the failure surfaces three steps later
    // as "the app never did the thing".
    app.locator(Selector::id("row_fs")).wait_visible();
    settle(&app, 6);
}

/// An app that keeps hammering after being refused is STOPPED. Four bursts,
/// spaced so each one outlives the previous cooldown and earns its own strike:
/// the launcher force-stops the app and tells the user what happened, rather
/// than refusing its requests forever.
///
/// The relaunch block and "let it run again" are deliberately NOT asserted
/// here. Reaching this point already costs four wall-clock cooldowns, and
/// piling the rest on top pushed the whole flow past what the harness will sit
/// through; the grant store covers those transitions in unit tests
/// (`a_restricted_app_loses_every_capability`).
#[makepad_test]
fn repeated_flooding_gets_the_app_stopped(app: TestApp) {
    open_sandbox_probe(&app);
    app.locator(Selector::id("flood_btn")).wait_visible().click();
    app.locator(Selector::id("flood_status_w").text_contains("✅ REFUSED")).wait_visible();
    // Three more bursts. The cooldown is wall-clock, so real sleeping is what
    // makes the next burst count as a fresh offense rather than being swept up
    // by the cooldown already running.
    for _ in 0..3 {
        std::thread::sleep(std::time::Duration::from_millis(3200));
        app.locator(Selector::id("flood_btn")).wait_visible().click();
    }
    // Stopped: the notice names the app and says why.
    app.locator(Selector::id("restricted_title").text_contains("was stopped")).wait_visible();
    app.locator(Selector::id("restricted_body").text_contains("shut it down")).wait_visible();
    app.locator(Selector::id("restricted_ok")).wait_visible().click();
    settle(&app, 10);
    // The app really is gone, not just hidden behind the notice.
    app.locator(Selector::id("flood_btn")).wait_hidden();
}

/// notes → todo cross-app IPC: sending prompts the SENDER's ipc grant, the
/// resident todo isolate receives the task, and the status label reports each
/// stage honestly ("Open To-Do first" before the receiver exists).
#[makepad_test]
fn notes_sends_task_to_todo_via_ipc(app: TestApp) {
    // Notes first, without todo running: delivery finds no receiver.
    open_mini_app(&app, "📝", "editor");
    app.locator(Selector::all().text_exact("→ To-Do")).wait_visible().click();
    // First use of ipc prompts (sender side).
    app.locator(Selector::id("perm_title").text_contains("App Messaging"))
        .wait_visible();
    app.locator(Selector::id("perm_allow")).wait_visible().click();
    app.locator(Selector::all().text_exact("Open To-Do first")).wait_visible();

    // Boot todo (it stays resident when "closed"), then send again.
    app.locator(Selector::id("back_button")).wait_visible().click();
    app.locator(Selector::id("editor")).wait_hidden();
    open_mini_app(&app, "✅", "task_input");
    app.locator(Selector::id("back_button")).wait_visible().click();
    app.locator(Selector::id("task_input")).wait_hidden();

    open_mini_app(&app, "📝", "editor");
    app.locator(Selector::all().text_exact("→ To-Do")).wait_visible().click();
    // Already granted: no prompt, straight to delivery.
    app.locator(Selector::all().text_exact("Sent to To-Do ✓")).wait_visible();

    // The welcome note's first line landed in todo's list.
    app.locator(Selector::id("back_button")).wait_visible().click();
    app.locator(Selector::all().text_exact("Sent to To-Do ✓")).wait_hidden();
    open_mini_app(&app, "✅", "task_input");
    app.locator(Selector::all().text_contains("This whole app is a Splash"))
        .wait_visible();
}

/// The counter app and its home widget are separate isolates of ONE app:
/// same-app IPC (permission-free) plus the shared fs jail keep their counts
/// in lockstep.
#[makepad_test]
fn counter_widget_syncs_with_app(app: TestApp) {
    enter_edit_mode(&app);
    app.locator(Selector::all().text_exact("＋ Widget")).wait_visible().click();
    app.locator(Selector::all().text_contains("Counter")).wait_visible().click();
    app.locator(Selector::all().text_contains("Add")).wait_visible().click();
    let pager = app.locator(Selector::id("home_pager")).snapshot();
    let (px, py) = (pager.x as f64, pager.y as f64);
    let (pw, ph) = (pager.width as f64, pager.height as f64);
    tap(&app, px + pw * 0.12, py + ph * 0.9);
    app.locator(Selector::id("badge")).wait_hidden();
    app.locator(Selector::id("value_lg").text_exact("0")).wait_visible();

    // Open the counter APP (second page) and increment there.
    app.locator(Selector::id("home_pager")).drag_by(-300.0, 0.0);
    app.locator(Selector::id("name").text_exact("Counter"))
        .wait_visible()
        .click();
    app.locator(Selector::id("display").text_exact("0")).wait_visible();
    app.locator(Selector::widget_type("GlassButton").text_exact("+"))
        .wait_visible()
        .click();
    app.locator(Selector::id("display").text_exact("1")).wait_visible();

    // Back home: the widget's OTHER isolate heard the same-app broadcast.
    app.locator(Selector::id("back_button")).wait_visible().click();
    app.locator(Selector::id("display")).wait_hidden();
    app.locator(Selector::id("value_lg").text_exact("1")).wait_visible();
}

/// The settings app's PRIVACY section reads the whole grant table through its
/// privileged overview service (host-gated to the builtin settings id).
#[makepad_test]
fn settings_shows_privacy_overview(app: TestApp) {
    app.locator(Selector::id("name").text_exact("Settings"))
        .wait_visible()
        .click();
    app.locator(Selector::all().text_exact("PRIVACY")).wait_visible();
    // Weather declares permissions, so it appears in the overview.
    app.locator(Selector::all().text_contains("Weather")).wait_visible();
    app.locator(Selector::all().text_contains("Change grants in each app's App Info page."))
        .wait_visible();
}

/// An App Info permission row opens the three-state choice sheet, and all
/// three answers are reachable from any state. The two-button toggle this
/// replaced was a one-way door: Ask -> Allowed -> Blocked, with no way back to
/// "ask me the next time it's needed" — the state every runtime permission
/// starts in. Blocked -> Asks -> Allowed here, through the sheet, with the
/// row's state label following each answer.
#[makepad_test]
fn app_info_permission_sheet_offers_three_states(app: TestApp) {
    open_app_info(&app, "News");
    app.locator(Selector::all().text_exact("PERMISSIONS")).wait_visible();
    // Rows render in declaration order and News declares network first, so
    // row 0 is Network: runtime tier, so it starts out asking.
    app.locator(Selector::id("pr_title").text_exact("Network")).wait_visible();
    app.locator(Selector::id("pr_state").nth(0)).wait_text("Asks");

    // The row's button opens the sheet rather than flipping anything itself,
    // and the sheet names the permission AND the app it belongs to.
    app.locator(Selector::id("pr_toggle").nth(0)).wait_visible().click();
    app.locator(Selector::id("pc_title").text_contains("Network")).wait_visible();
    app.locator(Selector::id("pc_allow")).wait_visible();
    app.locator(Selector::id("pc_ask")).wait_visible();
    app.locator(Selector::id("pc_deny")).wait_visible().click();
    app.locator(Selector::id("pr_state").nth(0)).wait_text("Blocked");

    // The regression: a blocked permission can go BACK to asking (which is
    // what no toggle could express)...
    app.locator(Selector::id("pr_toggle").nth(0)).wait_visible().click();
    app.locator(Selector::id("pc_ask")).wait_visible().click();
    app.locator(Selector::id("pr_state").nth(0)).wait_text("Asks");
    // ...and from there straight to allowed.
    app.locator(Selector::id("pr_toggle").nth(0)).wait_visible().click();
    app.locator(Selector::id("pc_allow")).wait_visible().click();
    app.locator(Selector::id("pr_state").nth(0)).wait_text("Allowed");
}

/// The permission manager answers the question App Info can't: not "what may
/// this app do?" but "which apps can see my location?". Every capability is
/// listed with a tally, and drilling into one names the apps that declare it.
///
/// Reached here through the home screen's background menu.
/// `HOST_LAUNCHER_DEBUG_STATE=permissions` opens the same page, but the
/// harness spawns the app before the test body runs, so a test can't set the
/// app's environment — the menu is the route a user has anyway.
#[makepad_test]
fn permission_manager_lists_apps_per_capability(app: TestApp) {
    // Right-click empty space, low on the page, for the home-screen menu.
    let snap = app.locator(Selector::id("home_pager")).wait_visible().snapshot();
    let x = snap.x as f64 + snap.width as f64 / 2.0;
    let y = snap.y as f64 + snap.height as f64 * 0.9;
    app.forward(vec![StudioToApp::MouseDown(RemoteMouseDown {
        button_raw_bits: 2, x, y, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })]);
    app.forward(vec![StudioToApp::MouseUp(RemoteMouseUp {
        button_raw_bits: 2, x, y, time: 0.0, modifiers: RemoteKeyModifiers::default(),
    })]);
    app.locator(Selector::all().text_exact("Permissions…")).wait_visible().click();

    // The whole catalog is listed, each row with its tally. Location is second
    // in the catalog, and Weather is the only app that declares it — runtime
    // tier, still on Ask, so nothing is allowed to use it yet.
    app.locator(Selector::all().text_exact("CAPABILITIES")).wait_visible();
    app.locator(Selector::id("cap_title").text_exact("Location")).wait_visible();
    app.locator(Selector::id("cap_tally").nth(1)).wait_text("1 app · none allowed");

    // Drilling in names the app, and says where it stands...
    app.locator(Selector::id("cap_open").nth(1)).wait_visible().click();
    app.locator(Selector::id("pm_title").text_exact("Location")).wait_visible();
    app.locator(Selector::id("ca_name").text_exact("Weather")).wait_visible();
    app.locator(Selector::id("ca_state").nth(0)).wait_text("Asks");

    // ...and Back is a stage change inside the page, not a dismissal: the
    // capability list comes back, the app list goes away.
    app.locator(Selector::id("pm_back")).wait_visible().click();
    app.locator(Selector::id("ca_name")).wait_hidden();
    app.locator(Selector::id("cap_title").text_exact("Location")).wait_visible();
}

/// "Allow Once" is a grant for this run of the app and nothing else: the app
/// gets the capability immediately, but the answer never reaches the store, so
/// the next launch has to ask again. Weather's network grant shows both
/// halves — a once-grant restarts it connected (its boot fetch gets past the
/// `host.has("network")` gate and goes on to ask for location), and closing it
/// takes the grant down with its isolates.
#[makepad_test]
fn prompt_allow_once_does_not_persist(app: TestApp) {
    app.locator(Selector::id("glyph").text_exact("🌤"))
        .wait_visible()
        .click();
    app.locator(Selector::id("perm_title").text_contains("Network"))
        .wait_visible();
    // Only a runtime tier offers a one-shot grant, and network is one.
    app.locator(Selector::id("perm_once")).wait_visible().click();
    // The app proceeds WITH the capability: it restarted connected, so it now
    // asks the next question its fetch needs.
    app.locator(Selector::id("perm_title").text_contains("Location"))
        .wait_visible();
    app.locator(Selector::id("perm_deny")).wait_visible().click();
    app.locator(Selector::id("subtitle").text_contains("SAN FRANCISCO")).wait_visible();

    // Closing an app here does not STOP it (state survives a close, by
    // design), so the session grant rightly outlives that. Force Stop is what
    // ends the run — and with it the grant, which never touched disk.
    app.locator(Selector::id("back_button")).wait_visible().click();
    app.locator(Selector::id("subtitle").text_contains("SAN FRANCISCO")).wait_hidden();
    // Weather lives in the dock, so reach its App Info by long-pressing the
    // dock glyph (open_app_info wants a home-grid name label).
    let snap = app.locator(Selector::id("glyph").text_exact("🌤")).wait_visible().snapshot();
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
    app.locator(Selector::all().text_contains("App Info")).wait_visible().click();
    app.locator(Selector::all().text_exact("Force Stop")).wait_visible().click();
    app.locator(Selector::id("ai_close")).wait_visible().click();
    app.locator(Selector::id("glyph").text_exact("🌤"))
        .wait_visible()
        .click();
    app.locator(Selector::id("perm_title").text_contains("Network"))
        .wait_visible();
}

/// A generated app declares its capabilities in its header, and the launcher
/// both honors that and TELLS the user: the refined app asks for clipboard
/// access, so App Info lists it where they can block it.
#[makepad_test]
fn generated_app_declares_and_discloses_permissions(app: TestApp) {
    app.locator(Selector::id("create_input"))
        .wait_visible()
        .fill("make the notes app show a word count");
    submit_prompt(&app);
    app.locator(Selector::id("glyph").text_exact("🍅")).wait_visible();

    // The install flash names what it can now ask for.
    app.locator(Selector::all().text_contains("now wants: files"))
        .wait_visible();

    // ...and the declaration is real: App Info lists it, blockable.
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
    app.locator(Selector::all().text_contains("App Info")).wait_visible().click();
    app.locator(Selector::id("pr_title").text_exact("Files")).wait_visible();
}

/// App Info's RESOURCES section: every resource is listed with the amount in
/// force, and one can be changed to an exact value without disturbing the
/// others. This is the "how much may it use" half of the same page that
/// answers "what may it do".
#[makepad_test]
fn app_info_lists_and_sets_resource_amounts(app: TestApp) {
    open_app_info(&app, "News");
    scroll_app_info_to(&app, "ai_res_0");
    app.locator(Selector::all().text_exact("RESOURCES")).wait_visible();

    // Every resource has a row. Asserted from the snapshot rather than by
    // visibility: the six rows are taller than the page, so demanding that
    // all of them be on screen at once would be a test about scrolling.
    let titles: Vec<String> = app
        .widget_snapshot()
        .into_iter()
        .filter(|w| w.id == "rr_title")
        .filter_map(|w| w.text.clone())
        .collect();
    for want in [
        "Priority",
        "Processor limit",
        "Memory limit",
        "Timers",
        "Downloads at once",
        "Storage",
    ] {
        assert!(titles.iter().any(|t| t == want), "no row for {want}: {titles:?}");
    }
    // And there is no "fastest timer" rule any more: how often an app wants to
    // work is its business, and what it pays is the wakeups.
    assert!(
        !titles.iter().any(|t| t == "Fastest timer"),
        "the fastest-timer rule should be gone: {titles:?}"
    );

    // Priority ships as Normal, and the ceilings ship OFF — an app on its own
    // is limited by nothing, which is the whole point of the model.
    app.locator(Selector::id("rr_value").nth(0)).wait_visible().wait_text("Normal");

    // Change one: the sheet names the app and offers exact amounts.
    app.locator(Selector::id("rr_set").nth(0)).click();
    app.locator(Selector::id("rc_title").text_contains("Priority")).wait_visible();
    app.locator(Selector::id("rc_0")).wait_visible().click();
    settle(&app, 6);
    app.locator(Selector::id("rr_value").nth(0)).wait_text("Low");

    // And back to the default, which is its own deliberate choice.
    app.locator(Selector::id("rr_set").nth(0)).click();
    app.locator(Selector::id("rc_default")).wait_visible().click();
    settle(&app, 6);
    app.locator(Selector::id("rr_value").nth(0)).wait_text("Normal");
}

/// The timer cap, from inside a real app. The probe asks for far more timers
/// than any app needs; makepad refuses the ones past the isolate's allowance
/// and the app carries on with what it got. This is the half of the resource
/// story the host cannot enforce itself — only the VM can see a script asking.
#[makepad_test]
fn a_mini_app_cannot_hoard_timers(app: TestApp) {
    open_sandbox_probe(&app);
    app.locator(Selector::id("timers_status_w").text_exact("— UNTESTED")).wait_visible();
    app.locator(Selector::id("timers_btn")).wait_visible().click();
    // "✅ CAPPED AT n" — the count is the isolate's allowance, and a tile and a
    // fullscreen app are allowed different numbers, so assert the verdict.
    app.locator(Selector::id("timers_status_w").text_contains("✅ CAPPED AT")).wait_visible();
}
