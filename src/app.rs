//! The top-level application: a home screen launcher that hosts Splash mini-apps.
//!
//! See `handle_startup()` for the first code that runs on app startup.

use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use makepad_widgets::*;

use crate::{
    launcher::{
        app_drawer::{AppDrawerAction, AppDrawerRef, AppDrawerWidgetRefExt},
        app_store::{AppStoreAction, LauncherAppStoreWidgetRefExt, StoreEntry},
        context_menu::{
            BackgroundMenuAction, ContextMenuAction, LauncherBackgroundMenu,
            LauncherContextMenu, LauncherContextMenuWidgetRefExt,
            LauncherWidgetPickerWidgetRefExt, MenuContext, MenuSource, MENU_CALLOUT_H, MENU_WIDTH,
            WidgetPickerAction, WidgetPickerEntry,
        },
        dock::DockAction,
        home_pager::{HomePagerAction, HomePagerRef, HomePagerWidgetRefExt, ItemKey},
        page_indicator::{PageIndicatorRef, PageIndicatorWidgetRefExt},
        search_overlay::{SearchOverlayAction, SearchOverlayRef, SearchOverlayWidgetRefExt},
    },
    mini_apps::{
        builtin,
        mini_app_screen::{MiniAppScreenAction, MiniAppScreenRef, MiniAppScreenWidgetRefExt},
        registry::{
            AppRegistry, HomePage, LauncherLayout, MAX_GRID_COLS, MAX_GRID_ROWS, MAX_PAGES,
            MIN_GRID_COLS, MIN_GRID_ROWS, MiniAppId, PlacedItem, PlacedKind, WidgetInstanceId,
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
                        // Full-bleed sheet: only the rounded TOP edge (with the grab
                        // bar) shows below a strip of home; the panel fills the full
                        // width and runs off the sides + bottom of the screen.
                        margin: Inset{top: (36.0 + mod.widgets.SAFE_INSET_PAD_TOP)}
                    }

                    // The Spotlight search overlay drops in above the home screen
                    // and drawer, but below an open mini-app.
                    search_overlay := SearchOverlay{
                        margin: Inset{top: (mod.widgets.SAFE_INSET_PAD_TOP)}
                    }

                    mini_app_screen := MiniAppScreen{}

                    // Popup menus, Android-style: anchored next to what was pressed
                    // (top-left aligned + margin set at open time), with only a
                    // whisper of a scrim instead of a desktop-modal dim.
                    context_menu_modal := Modal{
                        align: Align{x: 0.0, y: 0.0}
                        bg_view := View{
                            width: Fill
                            height: Fill
                            show_bg: true
                            draw_bg +: {
                                color: uniform(#00000030)
                                pixel: fn() { return self.color }
                            }
                        }
                        content := LauncherContextMenu{}
                    }

                    widget_picker_modal := Modal{
                        content := LauncherWidgetPicker{}
                    }

                    app_store_modal := Modal{
                        align: Align{x: 0.5, y: 0.5}
                        bg_view := View{
                            width: Fill
                            height: Fill
                            show_bg: true
                            draw_bg +: {
                                color: uniform(#00000030)
                                pixel: fn() { return self.color }
                            }
                        }
                        content := LauncherAppStore{}
                    }

                    background_menu_modal := Modal{
                        align: Align{x: 0.0, y: 0.0}
                        bg_view := View{
                            width: Fill
                            height: Fill
                            show_bg: true
                            draw_bg +: {
                                color: uniform(#00000030)
                                pixel: fn() { return self.color }
                            }
                        }
                        content := LauncherBackgroundMenu{}
                    }

                    // Centered "are you sure?" prompt shown before removing an
                    // icon/widget via its edit-mode × badge.
                    confirm_remove_modal := Modal{
                        bg_view := View{
                            width: Fill
                            height: Fill
                            show_bg: true
                            draw_bg +: {
                                color: uniform(#00000073)
                                pixel: fn() { return self.color }
                            }
                        }
                        content := View{
                            width: 300
                            height: Fit
                            flow: Down
                            glass.Panel{
                                width: Fill
                                height: Fit
                                flow: Down
                                spacing: 4
                                padding: Inset{top: 22, bottom: 16, left: 22, right: 22}
                                confirm_title := Label{
                                    text: "Remove?"
                                    draw_text +: {
                                        color: #ffffff
                                        text_style: theme.font_bold{font_size: 17}
                                    }
                                }
                                confirm_body := Label{
                                    width: Fill
                                    text: ""
                                    draw_text +: {
                                        color: #xc8d6f0
                                        text_style: theme.font_regular{font_size: 13}
                                    }
                                }
                                View{width: Fill, height: 16}
                                View{
                                    width: Fill
                                    height: Fit
                                    flow: Right
                                    spacing: 10
                                    align: Align{x: 1.0, y: 0.5}
                                    confirm_cancel := glass.GlassButton{
                                        text: "Cancel"
                                        height: 36
                                        padding: Inset{left: 18, right: 18}
                                        draw_text +: { text_style: theme.font_bold{font_size: 13} }
                                    }
                                    confirm_remove := glass.GlassButton{
                                        text: "Remove"
                                        height: 36
                                        padding: Inset{left: 18, right: 18}
                                        draw_text +: {
                                            color: #xff8a8a
                                            text_style: theme.font_bold{font_size: 13}
                                        }
                                    }
                                }
                            }
                        }
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
    /// Per-app notification counts shown as icon badges. Demo-seeded for now;
    /// a real notification pipeline would update these at runtime.
    pub notifications: HashMap<MiniAppId, u64>,
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
            notifications: HashMap::new(),
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
    /// Which wallpaper tint preset is active (cycled from the background menu).
    #[rust]
    wallpaper: usize,
    /// Whether the edit-mode management bar is currently shown.
    #[rust]
    edit_bar_shown: bool,
    /// Reveal progress of the edit bar: 0 (collapsed) .. 1 (fully shown).
    /// Animated so entering/leaving edit mode slides the grid down/up smoothly
    /// instead of jumping it.
    #[rust]
    edit_bar_anim: f64,
    #[rust]
    edit_bar_frame: NextFrame,
    /// What the confirm-remove modal will act on if confirmed (an item to remove,
    /// or a whole page to delete).
    #[rust]
    pending_confirm: Option<PendingConfirm>,
}

/// The action the shared confirmation modal will carry out on "confirm".
#[derive(Clone, Debug)]
enum PendingConfirm {
    /// Remove a single placed item (its edit-mode × badge was tapped).
    RemoveItem(ItemKey),
    /// Delete a whole home page (and its contents) by index.
    DeletePage(usize),
}

/// Natural (fully-revealed) height of the edit-mode management bar. The reveal
/// animation grows/shrinks the bar's height between 0 and this.
const EDIT_BAR_HEIGHT: f64 = 77.0;

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

    fn search_overlay(&self, cx: &mut Cx) -> SearchOverlayRef {
        self.ui.search_overlay(cx, ids!(search_overlay))
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
        // Give every placed item a unique instance id — migrates layouts saved
        // before app icons carried instances (whose `instance` all default to 0).
        layout.renumber_instances();

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

        // Backfill the dock for layouts saved before docks existed (or before it
        // grew to five slots), and drop favorites whose app was uninstalled. An
        // app promoted into the dock loses its grid icon so it isn't shown twice.
        for id in Self::default_dock() {
            if layout.dock.len() >= 5 {
                break;
            }
            if !layout.dock.contains(&id) {
                layout.remove_items(
                    |it| matches!(&it.kind, PlacedKind::App { id: placed, .. } if placed == &id),
                );
                layout.dock.push(id);
            }
        }
        layout.dock.retain(|id| registry.contains(id));

        // Demo notification counts: a real pipeline would feed these at runtime.
        let notifications = HashMap::from([
            ("news".to_string(), 3),
            ("calendar".to_string(), 25),
            ("music".to_string(), 104),
        ]);

        self.app_state = AppState {
            registry,
            layout,
            notifications,
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
        // Dock favorites (weather/notes/todo/music/gallery) live in the bottom
        // bar, so they aren't repeated in the grid.
        let icons0 = [
            ("news", 1u8, 2u8),
            ("calculator", 2, 3),
            ("clock", 3, 3),
            ("settings", 0, 4),
            ("calendar", 1, 4),
        ];
        for (id, col, row) in icons0 {
            page0.items.push(PlacedItem {
                // instance 0 here is a placeholder; init_state renumbers every
                // placed item to a unique instance after building/loading the layout.
                kind: PlacedKind::App { id: id.to_string(), instance: 0 },
                col,
                row,
            });
        }
        let mut page1 = HomePage::default();
        page1.items.push(PlacedItem {
            kind: PlacedKind::App { id: "counter".into(), instance: 0 },
            col: 0,
            row: 2,
        });
        page1.items.push(PlacedItem {
            kind: PlacedKind::App { id: "stopwatch".into(), instance: 0 },
            col: 1,
            row: 2,
        });
        layout.pages = vec![page0, page1];
        layout.dock = Self::default_dock();
        layout.renumber_instances();
        layout
    }

    /// The out-of-the-box dock favorites (filtered to what's installed at runtime).
    fn default_dock() -> Vec<MiniAppId> {
        ["weather", "notes", "todo", "music", "gallery"]
            .iter()
            .map(|s| s.to_string())
            .collect()
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

    /// Positions a popup panel of the given size next to its anchor rect, like
    /// Android's app-shortcut menu: horizontally centered on the anchor, below
    /// it if there's room and above it otherwise, clamped to the window.
    fn place_popup(&mut self, cx: &mut Cx, target: &[LiveId], size: Vec2d, anchor: Rect) {
        let screen = self
            .ui
            .window(cx, ids!(main_window))
            .get_inner_size(cx);
        let gap = 8.0;
        let x = (anchor.pos.x + anchor.size.x * 0.5 - size.x * 0.5)
            .clamp(8.0, (screen.x - size.x - 8.0).max(8.0));
        // Sit the menu flush against the anchor: dropped below it (iOS-style) when
        // there's room, otherwise above it. If it fits neither side fully, use the
        // side with more room and let it run toward the screen edge — never
        // clamped back *over* the icon, which is what looked bad.
        let below_y = anchor.pos.y + anchor.size.y + gap;
        let above_y = anchor.pos.y - gap - size.y;
        let fits_below = below_y + size.y <= screen.y - gap;
        let fits_above = above_y >= gap;
        let y = if fits_below {
            below_y
        } else if fits_above {
            above_y
        } else if screen.y - below_y >= anchor.pos.y - gap {
            below_y
        } else {
            above_y
        };
        // Position by writing the content widget's walk margin directly, like
        // makepad's own CalloutTooltip does — no need to spin up the script VM
        // (tokenize + parse + eval) just to set two numbers. The menu widgets
        // are custom types that deref to `View`, so we reach `walk` through
        // their concrete type; place_popup only ever targets these two.
        let margin = Inset { left: x, top: y, right: 0.0, bottom: 0.0 };
        // The menu got dropped below the icon when `y` sits past its top edge, so
        // point the callout up; otherwise it's above the icon, pointing down. The
        // triangle lines up with the anchor's horizontal centre (relative to the
        // menu's clamped left edge).
        let points_up = y >= anchor.pos.y;
        let callout_x = anchor.pos.x + anchor.size.x * 0.5 - x;
        let popup = self.ui.widget(cx, target);
        if let Some(mut menu) = popup.borrow_mut::<LauncherContextMenu>() {
            menu.walk.margin = margin;
            menu.set_callout(cx, points_up, callout_x);
        } else if let Some(mut menu) = popup.borrow_mut::<LauncherBackgroundMenu>() {
            menu.walk.margin = margin;
        }
    }

    /// Shows the long-press context menu for an app or widget, anchored to it.
    fn show_context_menu(
        &mut self,
        cx: &mut Cx,
        app_id: &MiniAppId,
        widget_instance: Option<u64>,
        home_instance: Option<u64>,
        source: MenuSource,
        anchor: Rect,
    ) {
        let Some(manifest) = self.app_state.registry.get(app_id) else {
            return;
        };
        let on_home = self.app_state.layout.pages.iter().any(|p| {
            p.items.iter().any(
                |it| matches!(&it.kind, PlacedKind::App { id, .. } if id == app_id),
            )
        });
        let info = format!(
            "{} · isolated Splash VM · network {}",
            if manifest.builtin { "built-in" } else { "user app" },
            if manifest.allow_net { "on" } else { "off" },
        );
        let context = MenuContext {
            app_id: app_id.clone(),
            widget_instance,
            home_instance,
            source,
            running: self.mini_app_screen(cx).is_running(app_id),
            on_home,
            has_widget: manifest.widget.is_some(),
            builtin: manifest.builtin,
            shortcuts: manifest.shortcuts.clone(),
            info,
        };
        let (glyph, name) = (manifest.icon.clone(), manifest.name.clone());
        let height = self
            .ui
            .launcher_context_menu(cx, ids!(context_menu_modal.content))
            .show(cx, &glyph, &name, context);
        self.place_popup(
            cx,
            ids!(context_menu_modal.content),
            // Reserve room for the callout triangle so the menu still clears the
            // icon once the triangle is added on the anchor-facing side.
            dvec2(MENU_WIDTH, height + MENU_CALLOUT_H),
            anchor,
        );
        self.ui.modal(cx, ids!(context_menu_modal)).open(cx);
        // A widget's menu also shows the Android resize indicator around it.
        self.home_pager(cx).set_resize_hint(cx, widget_instance);
    }

    fn close_context_menu(&mut self, cx: &mut Cx) {
        self.ui.modal(cx, ids!(context_menu_modal)).close(cx);
        self.home_pager(cx).set_resize_hint(cx, None);
    }

    fn close_background_menu(&mut self, cx: &mut Cx) {
        self.ui.modal(cx, ids!(background_menu_modal)).close(cx);
    }

    /// Opens the widget gallery listing every app that provides a widget, with a
    /// live preview + size chooser per widget.
    fn open_widget_picker(&mut self, cx: &mut Cx) {
        let entries: Vec<WidgetPickerEntry> = self
            .app_state
            .registry
            .iter()
            .filter_map(|m| {
                m.widget.as_ref().map(|w| WidgetPickerEntry {
                    app_id: m.id.clone(),
                    label: format!("{}  {}", m.icon, m.name),
                    source: w.source.clone(),
                    min_span: w.min_span,
                    default_span: w.default_span,
                })
            })
            .collect();
        let grid = self.app_state.layout.grid();
        self.ui
            .launcher_widget_picker(cx, ids!(widget_picker_modal.content))
            .show(cx, &entries, grid);
        self.ui.modal(cx, ids!(widget_picker_modal)).open(cx);
    }

    /// Builds the App Store row list from the store catalog (installable apps +
    /// seeded samples), marking each installed or not, and pushes it into the
    /// store widget. Every removable/installable app the launcher knows lives in
    /// the store catalog, so this single pass covers them all.
    fn refresh_app_store(&mut self, cx: &mut Cx) {
        let entries: Vec<StoreEntry> = crate::mini_apps::builtin::store_catalog()
            .into_iter()
            .map(|m| StoreEntry {
                installed: self.app_state.registry.contains(&m.id),
                subtitle: if m.widget.is_some() {
                    "Includes a widget".to_string()
                } else {
                    "Mini-app".to_string()
                },
                app_id: m.id,
                icon: m.icon,
                name: m.name,
            })
            .collect();
        self.ui
            .launcher_app_store(cx, ids!(app_store_modal.content))
            .show(cx, &entries);
    }

    /// Opens the App Store modal.
    fn open_app_store(&mut self, cx: &mut Cx) {
        self.refresh_app_store(cx);
        self.ui.modal(cx, ids!(app_store_modal)).open(cx);
    }

    /// Installs (or reinstalls) a store app: copies its manifest into the
    /// persisted user apps, registers it live, drops it onto the home screen, and
    /// lifts any prior uninstall tombstone. No-op if it's already installed or not
    /// in the store catalog.
    fn install_app(&mut self, cx: &mut Cx, app_id: &MiniAppId) {
        if self.app_state.registry.contains(app_id) {
            return;
        }
        let Some(manifest) = crate::mini_apps::builtin::store_catalog()
            .into_iter()
            .find(|m| &m.id == app_id)
        else {
            return;
        };
        self.app_state
            .layout
            .uninstalled_user_apps
            .retain(|id| id != app_id);
        self.app_state.layout.user_apps.push(manifest.clone());
        self.app_state.registry.insert(manifest);
        // Give it a home-screen icon so it's immediately visible.
        self.add_app_to_home(app_id);
        self.app_state.layout_dirty = true;
        cx.redraw_all();
    }

    /// Shows/hides the edit-mode management bar to match edit mode, and keeps
    /// its grid-size labels current.
    fn sync_edit_bar(&mut self, cx: &mut Cx) {
        let editing = self.app_state.edit_mode;
        if editing != self.edit_bar_shown {
            self.edit_bar_shown = editing;
            // Show immediately so it can grow from zero height; the collapse case
            // hides it once the animation reaches 0 (see advance_edit_bar_anim).
            if editing {
                self.ui.widget(cx, ids!(edit_bar)).set_visible(cx, true);
                // Pin the current (collapsed) height now so the first frame
                // doesn't flash the bar at full height before the anim starts.
                self.apply_edit_bar_height(cx);
            }
            self.edit_bar_frame = cx.new_next_frame();
        }
        if editing {
            let (cols, rows) = self.app_state.layout.grid();
            self.ui
                .label(cx, ids!(cols_label))
                .set_text(cx, &cols.to_string());
            self.ui
                .label(cx, ids!(rows_label))
                .set_text(cx, &rows.to_string());
        }
    }

    /// Sets the edit bar's height from the current reveal progress (ease-out).
    fn apply_edit_bar_height(&mut self, cx: &mut Cx) {
        let t = self.edit_bar_anim;
        let eased = 1.0 - (1.0 - t) * (1.0 - t);
        if let Some(mut bar) = self.ui.view(cx, ids!(edit_bar)).borrow_mut() {
            bar.walk.height = Size::Fixed(EDIT_BAR_HEIGHT * eased);
        }
    }

    /// Advances the edit-bar reveal animation one frame toward its target (grown
    /// when editing, collapsed otherwise), sliding the whole grid smoothly rather
    /// than snapping it when the bar appears/disappears.
    fn advance_edit_bar_anim(&mut self, cx: &mut Cx) {
        let target = if self.app_state.edit_mode { 1.0 } else { 0.0 };
        // ~28 frames (~0.47s) for a smooth, unhurried slide into/out of edit mode.
        let step = 0.035;
        if self.edit_bar_anim < target {
            self.edit_bar_anim = (self.edit_bar_anim + step).min(target);
        } else {
            self.edit_bar_anim = (self.edit_bar_anim - step).max(target);
        }
        self.apply_edit_bar_height(cx);
        cx.redraw_all();
        if self.edit_bar_anim != target {
            self.edit_bar_frame = cx.new_next_frame();
        } else if target == 0.0 {
            self.ui.widget(cx, ids!(edit_bar)).set_visible(cx, false);
        }
    }

    /// Applies a +/- step to the grid dimensions, reflowing items that no
    /// longer fit the smaller grid.
    fn step_grid(&mut self, cx: &mut Cx, dcols: i8, drows: i8) {
        let layout = &mut self.app_state.layout;
        let (cols, rows) = layout.grid();
        layout.cols = (cols as i8 + dcols).clamp(MIN_GRID_COLS as i8, MAX_GRID_COLS as i8) as u8;
        layout.rows = (rows as i8 + drows).clamp(MIN_GRID_ROWS as i8, MAX_GRID_ROWS as i8) as u8;
        if (layout.cols, layout.rows) == (cols, rows) {
            return;
        }
        layout.clamp_items_to_grid();
        self.app_state.layout_dirty = true;
        self.sync_edit_bar(cx);
        cx.redraw_all();
    }

    /// Cycles the backdrop veil through a few wallpaper tints, iOS-style.
    fn cycle_wallpaper(&mut self, cx: &mut Cx) {
        // Deep navy, warm plum, teal night, near-black: a subtle tint over the
        // animated silk, not a full repaint.
        const TINTS: [Vec4f; 4] = [
            Vec4f { x: 0.02, y: 0.027, z: 0.055, w: 0.094 },
            Vec4f { x: 0.09, y: 0.03, z: 0.07, w: 0.12 },
            Vec4f { x: 0.02, y: 0.06, z: 0.07, w: 0.11 },
            Vec4f { x: 0.0, y: 0.0, z: 0.0, w: 0.16 },
        ];
        self.wallpaper = (self.wallpaper + 1) % TINTS.len();
        let tint = TINTS[self.wallpaper];
        let mut veil = self.ui.view(cx, ids!(wallpaper_veil));
        script_apply_eval!(cx, veil, {
            draw_bg +: { color: #(tint) }
        });
        veil.redraw(cx);
    }

    /// Keeps the home screen hidden while the drawer or search overlay covers it,
    /// so its icons don't bleed through the translucent panel (Android/iOS-style).
    /// Only toggles on an actual state change to avoid re-dirtying every frame.
    fn sync_overlays(&mut self, cx: &mut Cx) {
        // Hide the whole home screen (grid, widgets AND dock) when an overlay
        // covers it — otherwise the home's glass widgets and dock render their
        // refraction overlay *in front of* the covering layer. The drawer is
        // included: it's a full-screen frosted panel, and leaving the home visible
        // behind it just bled the home's app icons through the glass as distracting
        // ghosts. The wallpaper (LauncherBackdrop) is a separate root view, so the
        // drawer still refracts it — a clean frosted glass over the wallpaper.
        let covered = self.search_overlay(cx).is_open()
            || self.mini_app_screen(cx).is_fully_open()
            || self.drawer(cx).is_open();
        if covered != self.home_hidden_for_drawer {
            self.home_hidden_for_drawer = covered;
            self.ui
                .widget(cx, ids!(home_screen))
                .set_visible(cx, !covered);
            cx.redraw_all();
        }
    }

    /// Whether the home pager is the frontmost interactive layer. When an
    /// overlay is up, the pager must not react to gestures meant for it.
    fn home_input_enabled(&mut self, cx: &mut Cx) -> bool {
        !self.drawer(cx).is_open()
            && !self.mini_app_screen(cx).is_showing()
            && !self.ui.modal(cx, ids!(context_menu_modal)).is_open()
            && !self.ui.modal(cx, ids!(background_menu_modal)).is_open()
            && !self.ui.modal(cx, ids!(widget_picker_modal)).is_open()
            && !self.ui.modal(cx, ids!(app_store_modal)).is_open()
            && !self.search_overlay(cx).is_open()
    }

    /// Opens the Spotlight-style search overlay.
    fn open_search(&mut self, cx: &mut Cx) {
        self.search_overlay(cx).open(cx);
    }

    /// Deletes the current home page — immediately if it's empty, otherwise after a
    /// confirmation modal. Shared by the background menu and the edit-bar "− Page".
    fn request_delete_current_page(&mut self, cx: &mut Cx) {
        let page = self.home_pager(cx).current_page_index();
        let count = self
            .app_state
            .layout
            .pages
            .get(page)
            .map_or(0, |p| p.items.len());
        if count == 0 {
            self.home_pager(cx).delete_page(cx, &mut self.app_state, page);
            cx.redraw_all();
        } else {
            self.pending_confirm = Some(PendingConfirm::DeletePage(page));
            self.ui
                .label(cx, ids!(confirm_title))
                .set_text(cx, "Delete Page?");
            self.ui
                .glass_button(cx, ids!(confirm_remove))
                .set_text(cx, "Delete");
            self.ui.label(cx, ids!(confirm_body)).set_text(
                cx,
                &format!(
                    "Delete this page and the {} item{} on it?",
                    count,
                    if count == 1 { "" } else { "s" }
                ),
            );
            self.ui.modal(cx, ids!(confirm_remove_modal)).open(cx);
        }
    }

    /// Adds an app icon to the first page with room.
    fn add_app_to_home(&mut self, app_id: &MiniAppId) {
        let layout = &mut self.app_state.layout;
        let grid = layout.grid();
        // A fresh instance so this can be an additional (duplicate) icon of the app.
        let instance = layout.alloc_instance();
        for page in &mut layout.pages {
            if let Some((col, row)) = page.first_fit(grid, 1, 1) {
                page.items.push(PlacedItem {
                    kind: PlacedKind::App { id: app_id.clone(), instance },
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
                kind: PlacedKind::App { id: app_id.clone(), instance },
                col: 0,
                row: 0,
            });
            layout.pages.push(page);
            self.app_state.layout_dirty = true;
        }
    }

    /// Places a new widget instance for the given app at `req_span`, on the page
    /// the user is looking at (else the first page with room, else a new page).
    /// The span is clamped to the widget's minimum and the grid.
    fn add_widget_to_home(&mut self, cx: &mut Cx, app_id: &MiniAppId, req_span: (u8, u8)) {
        let Some(spec) = self
            .app_state
            .registry
            .get(app_id)
            .and_then(|m| m.widget.clone())
        else {
            return;
        };
        // Prefer the page the user is currently looking at.
        let current = self.home_pager(cx).current_page_index();
        let layout = &mut self.app_state.layout;
        let grid = layout.grid();
        let cols = req_span.0.clamp(spec.min_span.0, grid.0);
        let rows = req_span.1.clamp(spec.min_span.1, grid.1);
        let span = (cols, rows);
        let instance = layout.alloc_instance();

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
        // Try the current page first, then any other page, then a fresh page.
        if let Some(page) = layout.pages.get_mut(current) {
            if let Some((col, row)) = page.first_fit(grid, span.0, span.1) {
                page.items.push(placed(col, row));
                self.app_state.layout_dirty = true;
                return;
            }
        }
        for page in &mut layout.pages {
            if let Some((col, row)) = page.first_fit(grid, span.0, span.1) {
                page.items.push(placed(col, row));
                self.app_state.layout_dirty = true;
                return;
            }
        }
        if layout.pages.len() < MAX_PAGES {
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
            .remove_items(|it| matches!(&it.kind, PlacedKind::App { id, .. } if id == app_id))
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
        if self.ui.modal(cx, ids!(confirm_remove_modal)).is_open() {
            // Dismiss the confirmation without acting — and without also falling
            // through to exit edit mode, which a bare edit-mode check would do.
            self.pending_confirm = None;
            self.ui.modal(cx, ids!(confirm_remove_modal)).close(cx);
            return true;
        }
        if self.ui.modal(cx, ids!(context_menu_modal)).is_open() {
            self.close_context_menu(cx);
            return true;
        }
        if self.ui.modal(cx, ids!(background_menu_modal)).is_open() {
            self.close_background_menu(cx);
            return true;
        }
        if self.ui.modal(cx, ids!(widget_picker_modal)).is_open() {
            // Reset first so the live-preview isolate is torn down (back/Escape
            // doesn't emit a Dismissed action the way a scrim tap does).
            self.ui
                .launcher_widget_picker(cx, ids!(widget_picker_modal.content))
                .reset(cx);
            self.ui.modal(cx, ids!(widget_picker_modal)).close(cx);
            return true;
        }
        if self.ui.modal(cx, ids!(app_store_modal)).is_open() {
            self.ui.modal(cx, ids!(app_store_modal)).close(cx);
            return true;
        }
        if self.search_overlay(cx).is_open() {
            self.search_overlay(cx).close(cx);
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
            } else if state == "search" {
                self.open_search(cx);
            } else if state == "confirmremove" {
                self.app_state.edit_mode = true;
                self.ui
                    .label(cx, ids!(confirm_body))
                    .set_text(cx, "Remove Calculator from the home screen?");
                self.ui.modal(cx, ids!(confirm_remove_modal)).open(cx);
            } else if state == "bgmenu" {
                self.place_popup(
                    cx,
                    ids!(background_menu_modal.content),
                    dvec2(232.0, 224.0),
                    Rect { pos: dvec2(210.0, 500.0), size: dvec2(1.0, 1.0) },
                );
                self.ui.modal(cx, ids!(background_menu_modal)).open(cx);
            } else if let Some(app_id) = state.strip_prefix("ctxmenu:") {
                // A realistic middle-row icon+label group anchor (see menu_action_for).
                let anchor = Rect {
                    pos: dvec2(186.0, 496.0),
                    size: dvec2(100.0, 80.0),
                };
                self.show_context_menu(cx, &app_id.to_string(), None, None, MenuSource::HomeIcon, anchor);
            } else if let Some(app_id) = state.strip_prefix("widgetmenu:") {
                // Place a 2x2 widget top-left and open its context menu so the
                // Android-style resize indicator is drawn around it.
                let mut page0 = HomePage::default();
                page0.items.push(PlacedItem {
                    kind: PlacedKind::Widget {
                        instance: 1,
                        app_id: app_id.to_string(),
                        cols: 2,
                        rows: 2,
                    },
                    col: 1,
                    row: 2,
                });
                self.app_state.layout.pages = vec![page0];
                cx.redraw_all();
                let anchor = Rect {
                    pos: dvec2(110.0, 560.0),
                    size: dvec2(160.0, 160.0),
                };
                self.show_context_menu(
                    cx,
                    &app_id.to_string(),
                    Some(1),
                    None,
                    MenuSource::HomeIcon,
                    anchor,
                );
            } else if let Some(app_id) = state.strip_prefix("widgetresize:") {
                // Show only the resize indicator around a widget (no menu), for
                // verifying the outline/handle hugs the card and stays on-screen when
                // the widget is flush against the right edge (col 2 = cols 2-3 of 4).
                let mut page0 = HomePage::default();
                page0.items.push(PlacedItem {
                    kind: PlacedKind::Widget {
                        instance: 1,
                        app_id: app_id.to_string(),
                        cols: 2,
                        rows: 2,
                    },
                    col: 2,
                    row: 2,
                });
                self.app_state.layout.pages = vec![page0];
                cx.redraw_all();
                self.home_pager(cx).set_resize_hint(cx, Some(1));
            } else if let Some(app_id) = state.strip_prefix("gallery:") {
                // Widget gallery jumped to the size/preview detail for one widget.
                self.open_widget_picker(cx);
                self.ui
                    .launcher_widget_picker(cx, ids!(widget_picker_modal.content))
                    .debug_open(cx, &app_id.to_string());
            } else if state == "store" {
                self.open_app_store(cx);
            } else if state == "gallery" {
                self.open_widget_picker(cx);
            } else if let Some(app_id) = state.strip_prefix("iwidget:") {
                // A single interactive widget at top-left (2x2), no indicators —
                // for exercising in-place widget interaction.
                let mut page0 = HomePage::default();
                page0.items.push(PlacedItem {
                    kind: PlacedKind::Widget {
                        instance: 1,
                        app_id: app_id.to_string(),
                        cols: 2,
                        rows: 2,
                    },
                    col: 0,
                    row: 0,
                });
                self.app_state.layout.pages = vec![page0];
                cx.redraw_all();
            } else if state == "bigwidgets" {
                // Large widget spans, for verifying content reflow on resize.
                let mut page0 = HomePage::default();
                page0.items.push(PlacedItem {
                    kind: PlacedKind::Widget {
                        instance: 1,
                        app_id: "clock".into(),
                        cols: 4,
                        rows: 2,
                    },
                    col: 0,
                    row: 0,
                });
                page0.items.push(PlacedItem {
                    kind: PlacedKind::Widget {
                        instance: 2,
                        app_id: "weather".into(),
                        cols: 4,
                        rows: 2,
                    },
                    col: 0,
                    row: 2,
                });
                self.app_state.layout.pages = vec![page0];
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
                    HomePagerAction::DragDrawer { progress } => {
                        self.drawer(cx).set_drag(cx, progress);
                    }
                    HomePagerAction::ReleaseDrawer { open } => {
                        self.drawer(cx).settle(cx, open);
                    }
                    HomePagerAction::OpenSearch => {
                        self.open_search(cx);
                    }
                    HomePagerAction::ShowContextMenu {
                        app_id,
                        widget_instance,
                        home_instance,
                        anchor,
                    } => {
                        let source = if widget_instance.is_some() {
                            MenuSource::HomeWidget
                        } else {
                            MenuSource::HomeIcon
                        };
                        self.show_context_menu(
                            cx,
                            &app_id,
                            widget_instance,
                            home_instance,
                            source,
                            anchor,
                        );
                    }
                    HomePagerAction::HidePopups => {
                        self.close_context_menu(cx);
                    }
                    HomePagerAction::ShowBackgroundMenu { abs } => {
                        self.place_popup(
                            cx,
                            ids!(background_menu_modal.content),
                            dvec2(232.0, 224.0),
                            Rect { pos: abs, size: dvec2(1.0, 1.0) },
                        );
                        self.ui.modal(cx, ids!(background_menu_modal)).open(cx);
                    }
                    HomePagerAction::PageChanged { position, count } => {
                        self.page_indicator(cx).set_state(cx, position, count);
                    }
                    HomePagerAction::RequestRemove { item, label } => {
                        self.pending_confirm = Some(PendingConfirm::RemoveItem(item));
                        self.ui.label(cx, ids!(confirm_title)).set_text(cx, "Remove?");
                        self.ui.glass_button(cx, ids!(confirm_remove)).set_text(cx, "Remove");
                        self.ui.label(cx, ids!(confirm_body)).set_text(
                            cx,
                            &format!("Remove {label} from the home screen?"),
                        );
                        self.ui.modal(cx, ids!(confirm_remove_modal)).open(cx);
                    }
                    HomePagerAction::None => (),
                }

                match widget_action.cast::<AppDrawerAction>() {
                    AppDrawerAction::OpenApp { app_id, from_rect } => {
                        self.drawer(cx).close(cx);
                        self.open_app(cx, &app_id, from_rect);
                    }
                    AppDrawerAction::ShowContextMenu { app_id, anchor } => {
                        self.show_context_menu(cx, &app_id, None, None, MenuSource::Drawer, anchor);
                    }
                    AppDrawerAction::DragOutApp { app_id, area, abs } => {
                        // Take the finger over to the pager FIRST (before the
                        // drawer's close-redraw can move the cell's area), then
                        // slide the drawer away — the same touch keeps dragging the
                        // app so it can be dropped anywhere on the home screen.
                        // Only slide the drawer away if the drag truly started; a
                        // stray long-press reported after the finger already lifted
                        // can't drag anything, so the drawer should stay put.
                        // A fresh instance so this drag-in becomes a distinct icon
                        // (duplicates of the same app are allowed).
                        let instance = self.app_state.layout.alloc_instance();
                        let pager = self.home_pager(cx);
                        let started = pager.begin_external_drag(
                            cx,
                            &self.app_state.layout,
                            app_id,
                            instance,
                            abs,
                            area,
                        );
                        if started {
                            self.drawer(cx).close(cx);
                        }
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
                        // The per-app "Add Widget to Home" shortcut uses the widget's
                        // default size (the gallery is where a size is chosen).
                        let span = self
                            .app_state
                            .registry
                            .get(&app_id)
                            .and_then(|m| m.widget.as_ref())
                            .map(|w| w.default_span)
                            .unwrap_or((2, 2));
                        self.add_widget_to_home(cx, &app_id, span);
                        cx.redraw_all();
                    }
                    ContextMenuAction::RemoveFromHome { app_id, instance } => {
                        self.close_context_menu(cx);
                        match instance {
                            // A specific home icon: remove just that one (duplicates
                            // of the same app stay).
                            Some(inst) => self.home_pager(cx).remove_by_key(
                                cx,
                                &mut self.app_state,
                                &ItemKey::App(inst),
                            ),
                            // No specific placement (drawer/dock menu): remove all.
                            None => self.remove_app_from_home(&app_id),
                        }
                        self.app_state.layout_dirty = true;
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

                match widget_action.cast::<DockAction>() {
                    DockAction::OpenApp { app_id, from_rect } => {
                        self.open_app(cx, &app_id, from_rect);
                    }
                    DockAction::ShowContextMenu { app_id, anchor } => {
                        self.show_context_menu(cx, &app_id, None, None, MenuSource::Drawer, anchor);
                    }
                    DockAction::None => (),
                }

                match widget_action.cast::<SearchOverlayAction>() {
                    SearchOverlayAction::OpenApp { app_id, from_rect } => {
                        self.search_overlay(cx).close(cx);
                        self.open_app(cx, &app_id, from_rect);
                    }
                    SearchOverlayAction::Dismissed => {
                        self.search_overlay(cx).close(cx);
                    }
                    SearchOverlayAction::None => (),
                }

                match widget_action.cast::<BackgroundMenuAction>() {
                    BackgroundMenuAction::EnterEditMode => {
                        self.close_background_menu(cx);
                        self.app_state.edit_mode = true;
                        cx.redraw_all();
                    }
                    BackgroundMenuAction::OpenSearch => {
                        self.close_background_menu(cx);
                        self.open_search(cx);
                    }
                    BackgroundMenuAction::OpenDrawer => {
                        self.close_background_menu(cx);
                        self.drawer(cx).open(cx);
                    }
                    BackgroundMenuAction::OpenAppStore => {
                        self.close_background_menu(cx);
                        self.open_app_store(cx);
                    }
                    BackgroundMenuAction::CycleWallpaper => {
                        self.close_background_menu(cx);
                        self.cycle_wallpaper(cx);
                    }
                    BackgroundMenuAction::DeletePage => {
                        self.close_background_menu(cx);
                        self.request_delete_current_page(cx);
                    }
                    BackgroundMenuAction::None => (),
                }

                match widget_action.cast::<WidgetPickerAction>() {
                    WidgetPickerAction::Add { app_id, span } => {
                        self.ui.modal(cx, ids!(widget_picker_modal)).close(cx);
                        self.add_widget_to_home(cx, &app_id, span);
                        cx.redraw_all();
                    }
                    WidgetPickerAction::None => (),
                }

                match widget_action.cast::<AppStoreAction>() {
                    AppStoreAction::Install(app_id) => {
                        self.install_app(cx, &app_id);
                        // Keep the store open and flip the row to "Remove".
                        self.refresh_app_store(cx);
                    }
                    AppStoreAction::Uninstall(app_id) => {
                        self.uninstall_app(cx, &app_id);
                        self.refresh_app_store(cx);
                    }
                    AppStoreAction::None => (),
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

        // Dismissing the remove-confirmation modal by tapping the scrim (or via a
        // route that emits its Dismissed action) must also drop the pending target,
        // so a later confirm can't act on a stale selection. The modal closes
        // itself; we just clear our side.
        if self
            .ui
            .modal(cx, ids!(confirm_remove_modal))
            .dismissed(actions)
        {
            self.pending_confirm = None;
        }

        // Dismissing a widget's context menu by tapping the scrim closes the modal
        // itself but never runs close_context_menu, which is what clears the
        // pager's resize_hint. Without this, the hint would stay set — freezing the
        // widget's interactivity and leaving a ghost resize frame. Clear it here.
        if self.ui.modal(cx, ids!(context_menu_modal)).dismissed(actions) {
            self.home_pager(cx).set_resize_hint(cx, None);
        }

        // Same for the widget gallery: a scrim/back dismissal must reset the picker
        // so its live preview Splash isolate is torn down (see reset()).
        if self.ui.modal(cx, ids!(widget_picker_modal)).dismissed(actions) {
            self.ui
                .launcher_widget_picker(cx, ids!(widget_picker_modal.content))
                .reset(cx);
        }

        // Confirmation modal buttons.
        if self
            .ui
            .glass_button(cx, ids!(confirm_cancel))
            .clicked(actions)
        {
            self.pending_confirm = None;
            self.ui.modal(cx, ids!(confirm_remove_modal)).close(cx);
        }
        if self
            .ui
            .glass_button(cx, ids!(confirm_remove))
            .clicked(actions)
        {
            match self.pending_confirm.take() {
                Some(PendingConfirm::RemoveItem(item)) => {
                    self.home_pager(cx)
                        .remove_by_key(cx, &mut self.app_state, &item);
                    self.app_state.layout_dirty = true;
                }
                Some(PendingConfirm::DeletePage(page)) => {
                    self.home_pager(cx).delete_page(cx, &mut self.app_state, page);
                }
                None => {}
            }
            self.ui.modal(cx, ids!(confirm_remove_modal)).close(cx);
        }

        // Edit-mode management bar actions.
        if self
            .ui
            .glass_button(cx, ids!(add_widget_button))
            .clicked(actions)
        {
            self.open_widget_picker(cx);
        }
        if self
            .ui
            .glass_button(cx, ids!(add_app_button))
            .clicked(actions)
        {
            self.open_app_store(cx);
        }
        if self
            .ui
            .glass_button(cx, ids!(wallpaper_button))
            .clicked(actions)
        {
            self.cycle_wallpaper(cx);
        }
        if self
            .ui
            .glass_button(cx, ids!(add_page_button))
            .clicked(actions)
        {
            if self.app_state.layout.pages.len() < MAX_PAGES {
                self.app_state.layout.pages.push(HomePage::default());
                self.app_state.layout_dirty = true;
                // Animate over to the newly-added last page (swiping through any
                // pages in between).
                let layout = self.app_state.layout.clone();
                let new_page = layout.pages.len() - 1;
                self.home_pager(cx).go_to_page(cx, &layout, new_page);
                cx.redraw_all();
            }
        }
        if self
            .ui
            .glass_button(cx, ids!(delete_page_button))
            .clicked(actions)
        {
            self.request_delete_current_page(cx);
        }
        if self.ui.glass_button(cx, ids!(col_minus)).clicked(actions) {
            self.step_grid(cx, -1, 0);
        }
        if self.ui.glass_button(cx, ids!(col_plus)).clicked(actions) {
            self.step_grid(cx, 1, 0);
        }
        if self.ui.glass_button(cx, ids!(row_minus)).clicked(actions) {
            self.step_grid(cx, 0, -1);
        }
        if self.ui.glass_button(cx, ids!(row_plus)).clicked(actions) {
            self.step_grid(cx, 0, 1);
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
        // Reclaim Splash isolates whose owning widget was dropped (e.g. a home
        // widget the user just deleted). Until an isolate is reclaimed its
        // `start_interval` timers keep firing into a now-missing widget subtree,
        // spamming "widget not found" errors. Cheap no-op when nothing's queued.
        makepad_widgets::widget_async::gc_dead_splash_isolates(cx);

        if self.edit_bar_frame.is_event(event).is_some() {
            self.advance_edit_bar_anim(cx);
        }

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
        self.sync_edit_bar(cx);
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
