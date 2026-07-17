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
    app.locator(Selector::id("close_button")).wait_visible().click();
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
    app.locator(Selector::id("close_button")).wait_visible().click();
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
    app.locator(Selector::all().text_exact("Open")).wait_visible();
    app.locator(Selector::all().text_exact("Remove from Home")).wait_visible();
}

/// Choosing "Edit Home Screen" from the context menu turns on edit mode, which
/// reveals the remove badges on every icon.
#[makepad_test]
fn context_menu_enters_edit_mode(app: TestApp) {
    app.locator(Selector::id("name").text_exact("News")).wait_visible();
    let snap = app.locator(Selector::id("name").text_exact("News")).snapshot();
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
    app.locator(Selector::all().text_exact("Jiggle & Edit")).wait_visible().click();
    // Edit mode shows the '×' remove badges on icons.
    app.locator(Selector::all().text_exact("×")).wait_visible();
}

/// In edit mode, dragging an app icon to an empty cell moves it there
/// (the headline long-press-to-rearrange interaction).
#[makepad_test]
fn edit_mode_drag_reorder(app: TestApp) {
    // Enter edit mode via the context menu.
    app.locator(Selector::id("name").text_exact("News")).wait_visible();
    let snap = app.locator(Selector::id("name").text_exact("News")).snapshot();
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
    app.locator(Selector::all().text_exact("Jiggle & Edit")).wait_visible().click();

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
    let snap = app.locator(Selector::id("name").text_exact("News")).wait_visible().snapshot();
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
    app.locator(Selector::all().text_exact("Jiggle & Edit")).wait_visible().click();
    app.locator(Selector::all().text_exact("×")).wait_visible();

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
    // Edit mode shows the '×' remove badges and the management bar.
    app.locator(Selector::all().text_exact("×")).wait_visible();
    app.locator(Selector::all().text_exact("Done")).wait_visible();
    // A quick tap on empty space exits edit mode.
    app.forward(vec![
        StudioToApp::MouseDown(RemoteMouseDown {
            button_raw_bits: 1, x, y, time: 0.0, modifiers: RemoteKeyModifiers::default(),
        }),
        StudioToApp::MouseUp(RemoteMouseUp {
            button_raw_bits: 1, x, y, time: 0.0, modifiers: RemoteKeyModifiers::default(),
        }),
    ]);
    app.locator(Selector::all().text_exact("Done")).wait_hidden();
}
