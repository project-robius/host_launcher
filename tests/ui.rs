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
    app.locator(Selector::id("name").text_exact("Weather"))
        .wait_visible();
    // The clock widget runs `std.start_interval` inside its own Splash VM;
    // a ":" in the label proves the isolate timer fired and `ui.*` resolved.
    app.locator(Selector::id("w_time").text_contains(":"))
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

/// The to-do mini-app supports adding a task through its text input.
#[makepad_test]
fn todo_add_task(app: TestApp) {
    app.locator(Selector::id("name").text_exact("To-Do"))
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
    app.locator(Selector::id("name").text_exact("Notes")).wait_visible();
    let snap = app.locator(Selector::id("name").text_exact("Notes")).snapshot();
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
    app.locator(Selector::all().text_exact("Edit Home Screen")).wait_visible().click();
    // Edit mode shows the '×' remove badges on icons.
    app.locator(Selector::all().text_exact("×")).wait_visible();
}

/// In edit mode, dragging an app icon to an empty cell moves it there
/// (the headline long-press-to-rearrange interaction).
#[makepad_test]
fn edit_mode_drag_reorder(app: TestApp) {
    // Enter edit mode via the context menu.
    app.locator(Selector::id("name").text_exact("Notes")).wait_visible();
    let snap = app.locator(Selector::id("name").text_exact("Notes")).snapshot();
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
    app.locator(Selector::all().text_exact("Edit Home Screen")).wait_visible().click();

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
