//! The top-level application: a home screen launcher that hosts Splash mini-apps.
//!
//! See `handle_startup()` for the first code that runs on app startup.

use std::time::{SystemTime, UNIX_EPOCH};

use makepad_widgets::*;

use crate::{
    launcher::{
        app_drawer::{AppDrawerAction, AppDrawerRef, AppDrawerWidgetRefExt},
        context_menu::{ContextMenuAction, LauncherContextMenuWidgetRefExt, MenuContext, MenuSource},
        home_pager::{HomePagerAction, HomePagerRef, HomePagerWidgetRefExt},
        page_indicator::{PageIndicatorRef, PageIndicatorWidgetRefExt},
    },
    mini_apps::{
        builtin,
        mini_app_screen::{MiniAppScreenAction, MiniAppScreenRef, MiniAppScreenWidgetRefExt},
        registry::{
            AppRegistry, GRID_ROWS, HomePage, LauncherLayout, MiniAppId, PlacedItem, PlacedKind,
            WidgetInstanceId,
        },
    },
    persistence,
};

app_main!(App);

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    load_all_resources() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.inner_size: vec2(420, 860)
                window.title: "Host Launcher"
                pass.clear_color: #x05070e
                body +: {
                    flow: Overlay
                    show_bg: true
                    draw_bg.color: #x05070e

                    // The animated backdrop deliberately bleeds under the system
                    // bars; each screen pads its own content out of the unsafe areas.
                    LauncherBackdrop{}

                    home_screen := HomeScreen{}

                    // The drawer sits above the home screen but below an open
                    // mini-app: opening an app from the drawer animates the drawer
                    // closed while the app zooms up on top, so the app (topmost)
                    // must receive input first.
                    app_drawer := AppDrawer{
                        margin: Inset{top: (mod.widgets.SAFE_INSET_PAD_TOP)}
                        padding: Inset{
                            bottom: (8.0 + mod.widgets.SAFE_INSET_PAD_BOTTOM),
                            left: (16.0 + mod.widgets.SAFE_INSET_PAD_LEFT),
                            right: (16.0 + mod.widgets.SAFE_INSET_PAD_RIGHT),
                        }
                    }

                    mini_app_screen := MiniAppScreen{}

                    context_menu_modal := Modal{
                        content := LauncherContextMenu{}
                    }
                }
            }
        }
    }
}

/// The top-level app state, shared with widgets via `Scope::with_data`.
pub struct AppState {
    pub registry: AppRegistry,
    pub layout: LauncherLayout,
    /// Whether the home screen is in edit (rearrange) mode.
    pub edit_mode: bool,
    /// Set by widgets after any layout mutation; the app saves and clears it.
    pub layout_dirty: bool,
    /// Whether the home pager should process gestures. False while an overlay
    /// (a mini-app, the drawer, or a menu) is on top, so the pager doesn't
    /// react to taps meant for the layer above it.
    pub home_input_enabled: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            registry: AppRegistry::default(),
            layout: LauncherLayout::default(),
            edit_mode: false,
            layout_dirty: false,
            home_input_enabled: true,
        }
    }
}

#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
    #[rust]
    app_state: AppState,
    /// Dev-only screenshot facility, enabled by HOST_LAUNCHER_AUTOMATION_DIR:
    /// touching `<dir>/shot_request` captures the next GPU frame to `<dir>/frame.png`.
    #[rust]
    automation_dir: Option<std::path::PathBuf>,
    #[rust]
    automation_timer: Timer,
    /// Tracks whether the home screen is currently hidden behind the open drawer.
    #[rust]
    home_hidden_for_drawer: bool,
}

impl App {
    fn home_pager(&self, cx: &mut Cx) -> HomePagerRef {
        self.ui.home_pager(cx, ids!(home_pager))
    }

    fn drawer(&self, cx: &mut Cx) -> AppDrawerRef {
        self.ui.app_drawer(cx, ids!(app_drawer))
    }

    fn mini_app_screen(&self, cx: &mut Cx) -> MiniAppScreenRef {
        self.ui.mini_app_screen(cx, ids!(mini_app_screen))
    }

    fn page_indicator(&self, cx: &mut Cx) -> PageIndicatorRef {
        self.ui.page_indicator(cx, ids!(page_indicator))
    }

    /// Whether this run should ignore saved state and not persist anything.
    /// Used by the UI test suite so tests always start from the default layout.
    fn is_fresh_run() -> bool {
        std::env::var("HOST_LAUNCHER_FRESH").is_ok_and(|v| v == "1")
    }

    /// Builds the registry (built-ins + surviving user apps) and the home layout,
    /// either restored from disk or freshly seeded.
    fn init_state(&mut self) {
        let mut registry = AppRegistry::new(builtin::builtin_apps());

        let saved = if Self::is_fresh_run() {
            None
        } else {
            persistence::load_launcher_layout().unwrap_or_else(|e| {
                error!("Failed to load launcher layout: {e}");
                None
            })
        };

        let mut layout = match saved {
            Some(saved) => saved,
            None => Self::default_layout(),
        };

        // Seed the deletable sample apps unless the user has uninstalled them.
        for sample in builtin::user_sample_apps() {
            let uninstalled = layout.uninstalled_user_apps.contains(&sample.id);
            let already = layout.user_apps.iter().any(|a| a.id == sample.id);
            if !uninstalled && !already {
                layout.user_apps.push(sample);
            }
        }
        for user_app in &layout.user_apps {
            registry.insert(user_app.clone());
        }

        // Drop any placements that refer to apps that no longer exist.
        for page in &mut layout.pages {
            page.items.retain(|item| registry.contains(item.app_id()));
        }
        if layout.pages.is_empty() {
            layout.pages.push(HomePage::default());
        }

        self.app_state = AppState {
            registry,
            layout,
            edit_mode: false,
            layout_dirty: false,
            home_input_enabled: true,
        };
    }

    /// The out-of-the-box home layout: clock widget up top, then app icons,
    /// with the user samples on a second page.
    fn default_layout() -> LauncherLayout {
        let mut layout = LauncherLayout::default();
        let mut page0 = HomePage::default();
        page0.items.push(PlacedItem {
            kind: PlacedKind::Widget {
                instance: 1,
                app_id: "clock".into(),
                cols: 2,
                rows: 1,
            },
            col: 0,
            row: 0,
        });
        page0.items.push(PlacedItem {
            kind: PlacedKind::Widget {
                instance: 2,
                app_id: "weather".into(),
                cols: 2,
                rows: 2,
            },
            col: 2,
            row: 0,
        });
        let icons0 = [
            ("weather", 0u8, 2u8),
            ("news", 1, 2),
            ("todo", 0, 3),
            ("notes", 1, 3),
            ("calculator", 2, 3),
            ("clock", 3, 3),
            ("settings", 0, 4),
            ("calendar", 1, 4),
            ("music", 2, 4),
            ("gallery", 3, 4),
        ];
        for (id, col, row) in icons0 {
            page0.items.push(PlacedItem {
                kind: PlacedKind::App { id: id.to_string() },
                col,
                row,
            });
        }
        let mut page1 = HomePage::default();
        page1.items.push(PlacedItem {
            kind: PlacedKind::App { id: "counter".into() },
            col: 0,
            row: 2,
        });
        page1.items.push(PlacedItem {
            kind: PlacedKind::App { id: "stopwatch".into() },
            col: 1,
            row: 2,
        });
        layout.pages = vec![page0, page1];
        layout.next_widget_instance = 3;
        layout
    }

    fn save_if_dirty(&mut self) {
        if self.app_state.layout_dirty {
            self.app_state.layout_dirty = false;
            if Self::is_fresh_run() {
                return;
            }
            if let Err(e) = persistence::save_launcher_layout(&self.app_state.layout) {
                error!("Failed to save launcher layout: {e}");
            }
        }
    }

    /// Opens a mini-app fullscreen, recording it as most-recently-used.
    fn open_app(&mut self, cx: &mut Cx, app_id: &MiniAppId, from_rect: Rect) {
        let Some(manifest) = self.app_state.registry.get(app_id).cloned() else {
            error!("BUG: tried to open unknown app {app_id}");
            return;
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.app_state.layout.recents.insert(app_id.clone(), now);
        self.app_state.layout_dirty = true;
        self.mini_app_screen(cx).open_app(cx, &manifest, from_rect);
    }

    /// Shows the long-press context menu for an app or widget.
    fn show_context_menu(
        &mut self,
        cx: &mut Cx,
        app_id: &MiniAppId,
        widget_instance: Option<u64>,
        source: MenuSource,
    ) {
        let Some(manifest) = self.app_state.registry.get(app_id) else {
            return;
        };
        let on_home = self.app_state.layout.pages.iter().any(|p| {
            p.items.iter().any(
                |it| matches!(&it.kind, PlacedKind::App { id } if id == app_id),
            )
        });
        let context = MenuContext {
            app_id: app_id.clone(),
            widget_instance,
            source,
            running: self.mini_app_screen(cx).is_running(app_id),
            on_home,
            has_widget: manifest.widget.is_some(),
            builtin: manifest.builtin,
        };
        let (glyph, name) = (manifest.icon.clone(), manifest.name.clone());
        self.ui
            .launcher_context_menu(cx, ids!(context_menu_modal.content))
            .show(cx, &glyph, &name, context);
        self.ui.modal(cx, ids!(context_menu_modal)).open(cx);
    }

    fn close_context_menu(&mut self, cx: &mut Cx) {
        self.ui.modal(cx, ids!(context_menu_modal)).close(cx);
    }

    /// Keeps the home screen hidden while the drawer covers it, so its icons
    /// don't bleed through the translucent drawer panel (Android-style). Only
    /// toggles on an actual state change to avoid re-dirtying every frame.
    fn sync_overlays(&mut self, cx: &mut Cx) {
        let drawer_open = self.drawer(cx).is_open();
        if drawer_open != self.home_hidden_for_drawer {
            self.home_hidden_for_drawer = drawer_open;
            self.ui
                .widget(cx, ids!(home_screen))
                .set_visible(cx, !drawer_open);
            cx.redraw_all();
        }
    }

    /// Whether the home pager is the frontmost interactive layer. When an
    /// overlay is up, the pager must not react to gestures meant for it.
    fn home_input_enabled(&mut self, cx: &mut Cx) -> bool {
        !self.drawer(cx).is_open()
            && !self.mini_app_screen(cx).is_showing()
            && !self.ui.modal(cx, ids!(context_menu_modal)).is_open()
    }

    /// Adds an app icon to the first page with room.
    fn add_app_to_home(&mut self, app_id: &MiniAppId) {
        let layout = &mut self.app_state.layout;
        for page in &mut layout.pages {
            if let Some((col, row)) = page.first_fit(1, 1) {
                page.items.push(PlacedItem {
                    kind: PlacedKind::App { id: app_id.clone() },
                    col,
                    row,
                });
                self.app_state.layout_dirty = true;
                return;
            }
        }
        if layout.pages.len() < crate::mini_apps::registry::MAX_PAGES {
            let mut page = HomePage::default();
            page.items.push(PlacedItem {
                kind: PlacedKind::App { id: app_id.clone() },
                col: 0,
                row: 0,
            });
            layout.pages.push(page);
            self.app_state.layout_dirty = true;
        }
    }

    /// Places a new widget instance for the given app on the first page with room.
    fn add_widget_to_home(&mut self, app_id: &MiniAppId) {
        let Some(spec) = self
            .app_state
            .registry
            .get(app_id)
            .and_then(|m| m.widget.clone())
        else {
            return;
        };
        let (cols, rows) = spec.default_span;
        let layout = &mut self.app_state.layout;
        let instance = layout.alloc_widget_instance();

        let placed = |col: u8, row: u8| PlacedItem {
            kind: PlacedKind::Widget {
                instance,
                app_id: app_id.clone(),
                cols,
                rows,
            },
            col,
            row,
        };
        for page in &mut layout.pages {
            if let Some((col, row)) = page.first_fit(cols, rows.min(GRID_ROWS)) {
                page.items.push(placed(col, row));
                self.app_state.layout_dirty = true;
                return;
            }
        }
        if layout.pages.len() < crate::mini_apps::registry::MAX_PAGES {
            let mut page = HomePage::default();
            page.items.push(placed(0, 0));
            layout.pages.push(page);
            self.app_state.layout_dirty = true;
        }
    }

    /// Removes all home-screen icons of the given app.
    fn remove_app_from_home(&mut self, app_id: &MiniAppId) {
        if self
            .app_state
            .layout
            .remove_items(|it| matches!(&it.kind, PlacedKind::App { id } if id == app_id))
        {
            self.app_state.layout_dirty = true;
        }
    }

    /// Removes a placed widget instance from the home screen.
    fn remove_widget_from_home(&mut self, instance: WidgetInstanceId) {
        if self.app_state.layout.remove_items(
            |it| matches!(&it.kind, PlacedKind::Widget { instance: i, .. } if *i == instance),
        ) {
            self.app_state.layout_dirty = true;
        }
    }

    /// Uninstalls a user app entirely: stops it, removes all its placements,
    /// and remembers not to re-seed it.
    fn uninstall_app(&mut self, cx: &mut Cx, app_id: &MiniAppId) {
        let Some(manifest) = self.app_state.registry.get(app_id) else {
            return;
        };
        if manifest.builtin {
            error!("BUG: tried to uninstall built-in app {app_id}");
            return;
        }
        self.mini_app_screen(cx).force_stop(cx, app_id);
        self.app_state.registry.remove(app_id);
        self.app_state
            .layout
            .remove_items(|it| it.app_id() == app_id);
        self.app_state.layout.user_apps.retain(|a| &a.id != app_id);
        self.app_state.layout.recents.remove(app_id);
        if !self.app_state.layout.uninstalled_user_apps.contains(app_id) {
            self.app_state
                .layout
                .uninstalled_user_apps
                .push(app_id.clone());
        }
        self.app_state.layout_dirty = true;
        cx.redraw_all();
    }

    /// Back-navigation priority: context menu, mini-app, drawer, edit mode,
    /// then snapping home to the first page. Returns true if back was consumed.
    fn handle_back(&mut self, cx: &mut Cx) -> bool {
        if self.ui.modal(cx, ids!(context_menu_modal)).is_open() {
            self.close_context_menu(cx);
            return true;
        }
        if self.mini_app_screen(cx).is_showing() {
            self.mini_app_screen(cx).close_active(cx);
            return true;
        }
        if self.drawer(cx).is_open() {
            self.drawer(cx).close(cx);
            return true;
        }
        if self.app_state.edit_mode {
            self.app_state.edit_mode = false;
            cx.redraw_all();
            return true;
        }
        if !self.home_pager(cx).is_on_first_page() {
            let layout = self.app_state.layout.clone();
            self.home_pager(cx).go_to_first_page(cx, &layout);
            return true;
        }
        false
    }
}

impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        // Give Splash scripts a real local clock (there's no timezone database in
        // the platform, so the host supplies the UTC offset).
        let offset_secs = utc_offset_secs();
        makepad_widgets::makepad_platform::script::timer::set_script_local_utc_offset_secs(
            offset_secs,
        );

        self.init_state();

        if !Self::is_fresh_run() {
            let window_ref = self.ui.window(cx, ids!(main_window));
            if let Err(e) = persistence::load_window_state(window_ref, cx) {
                error!("Failed to restore window geometry: {e}");
            }
        }

        if let Ok(dir) = std::env::var("HOST_LAUNCHER_AUTOMATION_DIR") {
            self.automation_dir = Some(dir.into());
            self.automation_timer = cx.start_interval(0.15);
        }

        // Debug-only: jump straight to a UI state on startup so its live GPU
        // rendering (glass lensing) can be screenshotted without input injection.
        // e.g. HOST_LAUNCHER_DEBUG_STATE=open:calculator or =drawer or =edit.
        if let Ok(state) = std::env::var("HOST_LAUNCHER_DEBUG_STATE") {
            if let Some(app_id) = state.strip_prefix("open:") {
                let from = Rect {
                    pos: dvec2(180.0, 400.0),
                    size: dvec2(56.0, 56.0),
                };
                self.open_app(cx, &app_id.to_string(), from);
            } else if state == "drawer" {
                self.drawer(cx).open(cx);
            } else if state == "edit" {
                self.app_state.edit_mode = true;
                cx.redraw_all();
            }
        }
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        for action in actions {
            if let Some(widget_action) = action.as_widget_action() {
                match widget_action.cast::<HomePagerAction>() {
                    HomePagerAction::OpenApp { app_id, from_rect } => {
                        self.open_app(cx, &app_id, from_rect);
                    }
                    HomePagerAction::OpenDrawer => {
                        self.drawer(cx).open(cx);
                    }
                    HomePagerAction::ShowContextMenu {
                        app_id,
                        widget_instance,
                        anchor: _,
                    } => {
                        let source = if widget_instance.is_some() {
                            MenuSource::HomeWidget
                        } else {
                            MenuSource::HomeIcon
                        };
                        self.show_context_menu(cx, &app_id, widget_instance, source);
                    }
                    HomePagerAction::PageChanged { position, count } => {
                        self.page_indicator(cx).set_state(cx, position, count);
                    }
                    HomePagerAction::None => (),
                }

                match widget_action.cast::<AppDrawerAction>() {
                    AppDrawerAction::OpenApp { app_id, from_rect } => {
                        self.drawer(cx).close(cx);
                        self.open_app(cx, &app_id, from_rect);
                    }
                    AppDrawerAction::ShowContextMenu { app_id, anchor: _ } => {
                        self.show_context_menu(cx, &app_id, None, MenuSource::Drawer);
                    }
                    AppDrawerAction::None => (),
                }

                match widget_action.cast::<ContextMenuAction>() {
                    ContextMenuAction::Open(app_id) => {
                        self.close_context_menu(cx);
                        self.drawer(cx).close(cx);
                        // No icon rect handy here; zoom out from the screen center.
                        let center = Rect {
                            pos: dvec2(180.0, 400.0),
                            size: dvec2(56.0, 56.0),
                        };
                        self.open_app(cx, &app_id, center);
                    }
                    ContextMenuAction::AddToHome(app_id) => {
                        self.close_context_menu(cx);
                        self.add_app_to_home(&app_id);
                        cx.redraw_all();
                    }
                    ContextMenuAction::AddWidget(app_id) => {
                        self.close_context_menu(cx);
                        self.add_widget_to_home(&app_id);
                        cx.redraw_all();
                    }
                    ContextMenuAction::RemoveFromHome(app_id) => {
                        self.close_context_menu(cx);
                        self.remove_app_from_home(&app_id);
                        cx.redraw_all();
                    }
                    ContextMenuAction::RemoveWidget(instance) => {
                        self.close_context_menu(cx);
                        self.remove_widget_from_home(instance);
                        cx.redraw_all();
                    }
                    ContextMenuAction::EnterEditMode => {
                        self.close_context_menu(cx);
                        self.app_state.edit_mode = true;
                        cx.redraw_all();
                    }
                    ContextMenuAction::ForceStop(app_id) => {
                        self.close_context_menu(cx);
                        self.mini_app_screen(cx).force_stop(cx, &app_id);
                    }
                    ContextMenuAction::Uninstall(app_id) => {
                        self.close_context_menu(cx);
                        self.uninstall_app(cx, &app_id);
                    }
                    ContextMenuAction::None => (),
                }

                if let MiniAppScreenAction::FullyClosed =
                    widget_action.cast::<MiniAppScreenAction>()
                {
                    cx.redraw_all();
                }
            }
        }

        // The drawer-handle chevron is a click fallback for the swipe-up gesture.
        if self
            .ui
            .button(cx, ids!(drawer_handle))
            .clicked(actions)
        {
            self.drawer(cx).open(cx);
        }

        self.save_if_dirty();
    }

    fn handle_shutdown(&mut self, cx: &mut Cx) {
        if !Self::is_fresh_run() {
            let window_ref = self.ui.window(cx, ids!(main_window));
            let _ = persistence::save_window_state(window_ref, cx);
        }
        self.app_state.layout_dirty = true;
        self.save_if_dirty();
    }
}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        // Order matters: base widgets first, then app widgets, then the app UI.
        crate::makepad_widgets::script_mod(vm);
        crate::shared::script_mod(vm);
        crate::launcher::script_mod(vm);
        crate::mini_apps::script_mod(vm);
        self::script_mod(vm)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        if self.automation_timer.is_event(event).is_some() {
            if let Some(dir) = &self.automation_dir {
                let request = dir.join("shot_request");
                if request.exists() {
                    let _ = std::fs::remove_file(&request);
                    cx.capture_next_frame_to_file(dir.join("frame.png"));
                }
                let tree_request = dir.join("tree_request");
                if tree_request.exists() {
                    let _ = std::fs::remove_file(&tree_request);
                    let dump = cx.widget_tree().compact_dump(cx);
                    let _ = std::fs::write(dir.join("tree.txt"), dump);
                }
                #[cfg(target_os = "macos")]
                {
                    let activate_request = dir.join("activate_request");
                    if activate_request.exists() {
                        let _ = std::fs::remove_file(&activate_request);
                        cx.macos_activate_app();
                    }
                }
            }
        }

        // Back button (Android) and Escape (desktop) share the same routing.
        if let Event::BackPressed { handled } = event {
            if !handled.get() && self.handle_back(cx) {
                handled.set(true);
            }
        }
        if let Event::KeyDown(KeyEvent {
            key_code: KeyCode::Escape,
            ..
        }) = event
        {
            self.handle_back(cx);
        }

        // Persist state when the app is backgrounded or asked to quit.
        if matches!(
            event,
            Event::Pause | Event::Background | Event::QuitRequested { .. }
        ) && !Self::is_fresh_run()
        {
            let window_ref = self.ui.window(cx, ids!(main_window));
            let _ = persistence::save_window_state(window_ref, cx);
            self.app_state.layout_dirty = true;
            self.save_if_dirty();
        }

        self.match_event(cx, event);
        // Let the pager know whether it's the frontmost layer before it sees the event.
        self.app_state.home_input_enabled = self.home_input_enabled(cx);
        let mut scope = Scope::with_data(&mut self.app_state);
        self.ui.handle_event(cx, event, &mut scope);

        // The drawer can open/close from its own gestures (swipe up/down); keep
        // the home screen's visibility in sync after the UI has handled events.
        self.sync_overlays(cx);
    }
}

/// The local timezone's offset from UTC in seconds. std::time can't provide it,
/// so we ask `date +%z` on unix; elsewhere we fall back to 0 (UTC).
fn utc_offset_secs() -> i64 {
    #[cfg(unix)]
    {
        if let Ok(output) = std::process::Command::new("date").arg("+%z").output() {
            let s = String::from_utf8_lossy(&output.stdout);
            let s = s.trim();
            if s.len() >= 5 {
                let sign = if s.starts_with('-') { -1i64 } else { 1i64 };
                let hours: i64 = s[1 ..= 2].parse().unwrap_or(0);
                let mins: i64 = s[3 ..= 4].parse().unwrap_or(0);
                return sign * (hours * 3600 + mins * 60);
            }
        }
    }
    0
}
