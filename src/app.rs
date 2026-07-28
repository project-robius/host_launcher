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
        app_info::{AppInfoAction, AppInfoContext, LauncherAppInfoWidgetRefExt},
    },
    mini_apps::{
        builtin,
        mini_app_screen::{MiniAppScreenAction, MiniAppScreenRef, MiniAppScreenWidgetRefExt},
        registry::{
            AppRegistry, HomePage, LauncherLayout, MAX_GRID_COLS, MAX_GRID_ROWS, MAX_PAGES,
            MIN_GRID_COLS, MIN_GRID_ROWS, MiniAppId, MiniAppManifest, PlacedItem, PlacedKind,
            WidgetInstanceId,
        },
    },
    persistence,
    shared::expand_arrow::ExpandArrow,
};
use crate::generate::prefs::{self, AgentPrefs, Backend, KnobId};

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

                    app_info_modal := Modal{
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
                        content := LauncherAppInfo{}
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

                    // First-run AI setup: opened by the ✨ glyph or automatically
                    // when a generation fails for lack of a provider. Pasting a
                    // key writes a minimal octos config; the provider is inferred
                    // from the key's prefix.
                    setup_modal := Modal{
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
                            width: 330
                            height: Fit
                            flow: Down
                            glass.Panel{
                                width: Fill
                                height: Fit
                                flow: Down
                                spacing: 8
                                padding: Inset{top: 22, bottom: 16, left: 22, right: 22}
                                Label{
                                    text: "Set up AI generation"
                                    draw_text +: {
                                        color: #ffffff
                                        text_style: theme.font_bold{font_size: 17}
                                    }
                                }
                                setup_body := Label{
                                    width: Fill
                                    text: "Paste an LLM provider API key. It's saved to octos's own config (~/.octos), never in this app."
                                    draw_text +: {
                                        color: #xc8d6f0
                                        text_style: theme.font_regular{font_size: 13}
                                    }
                                }
                                setup_key_input := LauncherTextInput{
                                    height: 40
                                    empty_text: "sk-ant-…  /  sk-…  /  AIza…"
                                }
                                setup_detected := Label{
                                    width: Fill
                                    text: ""
                                    draw_text +: {
                                        color: #x9dccff
                                        text_style: theme.font_bold{font_size: 12}
                                    }
                                }
                                glass.Caption{text: "OTHER WAYS"}
                                Label{
                                    width: Fill
                                    text: "• no key at all: install Ollama and `ollama pull qwen2.5-coder` — auto-detected\n• export ANTHROPIC_API_KEY (or OPENAI_/GEMINI_/…) in your shell\n• `octos auth login -p anthropic` stores a key in your keychain\n• `octos init` for models, endpoints and fallbacks"
                                    draw_text +: {
                                        color: #x9fb0cc
                                        text_style: theme.font_regular{font_size: 11.5}
                                    }
                                }
                                View{width: Fill, height: 12}
                                View{
                                    width: Fill
                                    height: Fit
                                    flow: Right
                                    spacing: 10
                                    align: Align{x: 1.0, y: 0.5}
                                    setup_cancel := glass.GlassButton{
                                        text: "Cancel"
                                        height: 36
                                        padding: Inset{left: 18, right: 18}
                                        draw_text +: { text_style: theme.font_bold{font_size: 13} }
                                    }
                                    setup_save := glass.GlassButtonProminent{
                                        text: "Save"
                                        height: 36
                                        padding: Inset{left: 22, right: 22}
                                        draw_text +: { text_style: theme.font_bold{font_size: 13} }
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
    /// The dock's on-screen rect, refreshed each event by the app. The pager
    /// reads it so a drag released over the dock drops into the dock instead of
    /// onto the grid.
    pub dock_rect: Rect,
    /// While a drag hovers the dock, the slot it would land in — mirrored from the
    /// pager each event. The dock shuffles its icons aside to open a gap there and
    /// outlines it, the same feedback the grid gives with its drop-target cell.
    pub dock_drop: Option<usize>,
    /// On-screen rect of the floating create bar / agent console (zero while
    /// edit mode hides it). The pager ignores presses starting inside it, so
    /// taps on the bar can't fall through to the icons it overlays.
    pub create_rect: Rect,
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
            dock_rect: Rect::default(),
            dock_drop: None,
            create_rect: Rect::default(),
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
    /// The in-flight AI "create app" generation, if any (drives the create bar).
    #[rust]
    generation: Option<crate::generate::pipeline::Generation>,
    /// Armed by "Modify App…": the next create-bar submit modifies this app
    /// instead of creating a new one.
    #[rust]
    pending_modify: Option<MiniAppId>,
    /// Whether the console's output area is collapsed away, leaving just the
    /// status line. Sticky across generations within a session.
    #[rust]
    activity_collapsed: bool,
    /// Whether the console should be on screen at all (bar is busy). Tracked
    /// separately from `generation` so the busy UI (incl. the genbusy debug
    /// state) and the console can never disagree.
    #[rust]
    activity_active: bool,
    /// Tallest the console's output has been this run, applied back as a floor
    /// on its height. The box grows with the log but must never shrink — a
    /// jumping panel is unreadable, and the code tail is a rolling window that
    /// would otherwise pull the bottom up as it churns.
    #[rust]
    console_floor: f64,
    /// Whether the console is still following the tail. True until the user
    /// scrolls or drags inside it — after that, yanking them back to the
    /// bottom on every agent event would make reading impossible.
    #[rust]
    console_follow: bool,
    /// Resets the create bar to idle a beat after a success/failure flash.
    #[rust]
    create_reset_timer: Timer,
    /// Periodic stall check for a live-but-silent agent (no events to wake us).
    #[rust]
    generation_watchdog: Timer,
    /// A result flash that fired while edit mode hid the create bar, replayed
    /// when the bar comes back so the user actually sees it.
    #[rust]
    pending_create_flash: Option<String>,
    /// Model/effort/thinking for the next generation, chosen in the bar's
    /// options row. Persisted; seeded from the environment on a first run.
    #[rust]
    agent_prefs: AgentPrefs,
    /// Whether the console is showing a COMPLETED run — kept on screen until
    /// the user presses outside the bar, rather than vanishing on a timer.
    #[rust]
    console_finished: bool,
    /// Whether the options row is open. Sticky once shown: clicking one of the
    /// controls takes key focus off the prompt, and hiding on focus-loss meant
    /// the row vanished out from under the click that opened it. Cleared when
    /// the composer is done with (submit, cancel, edit mode).
    #[rust]
    create_options_open: bool,
}

/// The action the shared confirmation modal will carry out on "confirm".
#[derive(Clone, Debug)]
enum PendingConfirm {
    /// Remove a single placed item (its edit-mode × badge was tapped).
    RemoveItem(ItemKey),
    /// Drop a favorite from the dock (its edit-mode × badge was tapped).
    RemoveFromDock(MiniAppId),
    /// Delete a whole home page (and its contents) by index.
    DeletePage(usize),
}

/// Natural (fully-revealed) height of the edit-mode management bar. The reveal
/// animation grows/shrinks the bar's height between 0 and this.
const EDIT_BAR_HEIGHT: f64 = 77.0;

/// How many favorites the dock holds. Dropping onto a full dock leaves the icon
/// on the home grid rather than silently discarding it.
pub const MAX_DOCK_ITEMS: usize = 5;

/// How much of the screen the agent console may grow to cover before it starts
/// scrolling instead. The bar floats over the grid, so this is really "how much
/// of the home screen a running generation is allowed to hide".
const CONSOLE_MAX_FRACTION: f64 = 0.62;

/// Clearance the console leaves above the dock when it grows.
const CONSOLE_DOCK_GAP: f64 = 30.0;

/// What the console opens at, before its first line has drawn — one line's
/// worth, so it grows into place rather than shrinking into it.
const CONSOLE_START_HEIGHT: f64 = 22.0;


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
        crate::is_fresh_env()
    }

    /// Builds the registry (built-ins + surviving user apps) and the home layout,
    /// either restored from disk or freshly seeded.
    fn init_state(&mut self) {
        // Saved picks win; on a first run the launcher's own environment seeds
        // them, so `CLAUDE_CODE_EFFORT_LEVEL=max ./scripts/run_with_claude.sh`
        // shows up pre-selected instead of being silently overridden.
        self.agent_prefs = if Self::is_fresh_run() {
            AgentPrefs::from_env()
        } else {
            persistence::load_agent_prefs().unwrap_or_else(AgentPrefs::from_env)
        };

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
            None => {
                // No (or corrupt) layout.json — but apps live in their OWN
                // dirs now, so recover them even when placements were lost.
                // This keeps them in the drawer/registry AND in taken_app_ids,
                // so a regeneration can't silently overwrite an orphan's dir
                // and inherit its app_data jail. (Placements are genuinely
                // gone; the user re-adds from the drawer.)
                let mut layout = Self::default_layout();
                if !Self::is_fresh_run() {
                    let recovered = persistence::load_user_apps();
                    if !recovered.is_empty() {
                        log!("Recovered {} app(s) from disk after layout loss", recovered.len());
                        layout.user_apps = recovered;
                    }
                }
                layout
            }
        };
        // Give every placed item a unique instance id — migrates layouts saved
        // before app icons carried instances (whose `instance` all default to 0).
        layout.renumber_instances();

        // Seed the deletable sample apps unless the user has uninstalled them.
        for sample in builtin::user_sample_apps() {
            let uninstalled = layout.uninstalled_user_apps.contains(&sample.id);
            let already = layout.user_apps.iter().any(|a| a.id == sample.id);
            if !uninstalled && !already {
                // Written through to its apps/<id>/ dir so future boots load
                // it like any other user app.
                if let Err(e) = persistence::save_user_app(&sample) {
                    error!("couldn't persist sample app '{}': {e}", sample.id);
                }
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
            if layout.dock.len() >= MAX_DOCK_ITEMS {
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
            dock_rect: Rect::default(),
            dock_drop: None,
            create_rect: Rect::default(),
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
        if let Err(e) = persistence::save_user_app(&manifest) {
            error!("couldn't persist installed app '{}': {e}", manifest.id);
        }
        self.app_state.layout.user_apps.push(manifest.clone());
        self.app_state.registry.insert(manifest);
        // Give it a home-screen icon so it's immediately visible.
        self.add_app_to_home(app_id);
        self.app_state.layout_dirty = true;
        cx.redraw_all();
    }

    // -----------------------------------------------------------------------
    // The AI "create app" bar
    // -----------------------------------------------------------------------

    /// Every id a freshly generated app must not collide with: live registry,
    /// persisted user apps, and uninstall tombstones (capturing a tombstoned
    /// id would resurrect the store app's identity on reinstall).
    fn taken_app_ids(&self) -> Vec<MiniAppId> {
        let mut taken: Vec<MiniAppId> =
            self.app_state.registry.iter().map(|m| m.id.clone()).collect();
        for m in &self.app_state.layout.user_apps {
            taken.push(m.id.clone());
        }
        taken.extend(self.app_state.layout.uninstalled_user_apps.iter().cloned());
        taken
    }

    /// Kicks off an AI generation for the typed request: spawns the ACP agent
    /// process and flips the bar into its busy state. No-op while one is
    /// already running (the bar's input is hidden then anyway), and while
    /// editing (the bar is hidden, but its hidden input can still hold key
    /// focus — a Return there must not start an invisible generation).
    fn start_generation(&mut self, cx: &mut Cx, request: String) {
        let request = request.trim().to_string();
        if request.is_empty() || self.generation.is_some() || self.app_state.edit_mode {
            return;
        }
        // A leftover flash-reset timer must not fire mid-generation and flip
        // the bar back to idle under us.
        cx.stop_timer(self.create_reset_timer);
        match crate::generate::pipeline::Generation::start(request, self.taken_app_ids(), self.agent_prefs.clone()) {
            Ok(generation) => {
                self.set_create_bar_busy(cx, generation.status());
                self.ui.text_input(cx, ids!(create_input)).set_text(cx, "");
                self.generation = Some(generation);
                // A silent-but-alive agent produces no events to wake us, so
                // poll for the stall verdict on a timer.
                self.generation_watchdog = cx.start_interval(15.0);
            }
            Err(reason) => self.flash_create_bar(cx, &reason),
        }
    }

    /// Cancels the in-flight generation (bar's Stop): tells the agent to stop,
    /// then drops the pipeline — which kills the agent process. A stale click
    /// that lands after the generation already finished must NOT touch the bar
    /// (it would eat the result flash).
    fn cancel_generation(&mut self, cx: &mut Cx) {
        if let Some(mut generation) = self.generation.take() {
            generation.cancel();
            cx.stop_timer(self.generation_watchdog);
            self.set_create_bar_idle(cx);
        }
    }

    /// Arms the create bar for a refine: the next submit modifies `app_id`.
    /// The bar's hint tells the user what they're typing a change FOR; an
    /// empty submit or picking another app's Refine re-arms cleanly.
    fn arm_modify(&mut self, cx: &mut Cx, app_id: &MiniAppId) {
        // While a generation runs, the bar is busy (status + Stop); arming
        // would flip it idle — hiding the only cancel affordance — and the
        // submit would be dropped anyway. Refuse, like start_* do.
        if self.generation.is_some() {
            return;
        }
        let Some(name) = self.app_state.registry.get(app_id).map(|m| m.name.clone()) else {
            return;
        };
        // The bar has to be VISIBLE for a focused input to make sense: edit
        // mode hides it, and the drawer/search cover it. All three are places
        // the menu can be opened from.
        if self.app_state.edit_mode {
            self.app_state.edit_mode = false;
            cx.redraw_all();
        }
        self.drawer(cx).close(cx);
        self.search_overlay(cx).close(cx);
        self.pending_modify = Some(app_id.clone());
        self.set_create_bar_idle(cx);
        let input = self.ui.text_input(cx, ids!(create_input));
        // Prefill "✏️ Weather: " and drop the caret after it, so the user just
        // types the change. The prefix is stripped on submit (and is only a
        // hint — deleting it falls back to normal intent detection).
        let prefix = modify_prefix(&name);
        input.set_text(cx, &prefix);
        self.sync_create_send(cx, &prefix);
        self.open_agent_options(cx);
        input.set_empty_text(cx, "Create an app…".to_string());
        input.set_cursor(
            cx,
            makepad_widgets::makepad_draw::text::selection::Cursor {
                index: prefix.len(),
                prefer_next_row: false,
            },
            false,
        );
        self.ui.widget(cx, ids!(create_input)).set_key_focus(cx);
        self.ui.widget(cx, ids!(create_bar)).redraw(cx);
    }

    /// Decides what a submitted create-bar line means: `Some((app, request))`
    /// to modify an installed app, or `None` to create a new one.
    ///
    /// Two ways to land on a modification: the bar was ARMED from "Modify
    /// App…" (its "✏️ Name: " prefix is stripped off the request), or the
    /// typed text itself reads like an edit of an installed app ("make the
    /// weather app show animations"). If an armed prefix was deleted, the
    /// text alone decides — so clearing the bar and typing something new
    /// creates, as the visible text implies.
    fn resolve_submit(&self, text: &str, armed: Option<MiniAppId>) -> Option<(MiniAppId, String)> {
        if let Some(app_id) = armed {
            if let Some(name) = self.app_state.registry.get(&app_id).map(|m| m.name.clone()) {
                if let Some(rest) = text.trim_start().strip_prefix(&modify_prefix(&name)) {
                    return Some((app_id, rest.trim().to_string()));
                }
                // Tolerate a trimmed/partly-edited prefix ("✏️ Weather:").
                let bare = format!("{PENCIL} {name}:");
                if let Some(rest) = text.trim_start().strip_prefix(&bare) {
                    return Some((app_id, rest.trim().to_string()));
                }
            }
        }
        // Nothing armed (or the prefix is gone): read the request itself.
        let installed: Vec<(MiniAppId, String)> = self
            .app_state
            .registry
            .iter()
            .map(|m| (m.id.clone(), m.name.clone()))
            .collect();
        match crate::generate::intent::classify(text, &installed) {
            crate::generate::intent::Intent::Modify(app_id) => {
                Some((app_id, text.trim().to_string()))
            }
            crate::generate::intent::Intent::Create => None,
        }
    }

    /// Kicks off a modification generation for a chosen app.
    fn start_modify(&mut self, cx: &mut Cx, app_id: &MiniAppId, request: String) -> bool {
        let request = request.trim().to_string();
        if request.is_empty() || self.generation.is_some() || self.app_state.edit_mode {
            // Bail WITHOUT disarming — the bar keeps its "✏️ Name: " prefix so
            // the state the user sees still matches what a submit would do.
            return false;
        }
        // The app may have been uninstalled between arming and submitting.
        let Some(base) = self.app_state.registry.get(app_id).cloned() else {
            self.flash_create_bar(cx, "That app is no longer installed");
            return true;
        };
        cx.stop_timer(self.create_reset_timer);
        match crate::generate::pipeline::Generation::start_refine(request, base, self.agent_prefs.clone()) {
            Ok(generation) => {
                self.set_create_bar_busy(cx, generation.status());
                self.ui.text_input(cx, ids!(create_input)).set_text(cx, "");
                self.generation = Some(generation);
                self.generation_watchdog = cx.start_interval(15.0);
            }
            Err(reason) => self.flash_create_bar(cx, &reason),
        }
        true
    }

    /// Archives an app's CURRENT state into its version history, so whatever
    /// replaces it can be reverted. `note` is why it changed (the request, or
    /// a marker). Best-effort: a history write failing must not block the
    /// modification the user asked for.
    fn snapshot_current(&mut self, app_id: &MiniAppId, note: &str) {
        let Some(current) = self.app_state.registry.get(app_id).cloned() else {
            return;
        };
        let version = crate::mini_apps::versions::version_of(
            &current,
            note,
            crate::mini_apps::versions::now_unix(),
            utc_offset_secs(),
        );
        if let Err(e) = persistence::snapshot_version(&current, version) {
            error!("couldn't snapshot a version of '{app_id}': {e}");
        }
    }

    /// Gathers everything the App Info page shows and opens it.
    fn open_app_info(&mut self, cx: &mut Cx, app_id: &MiniAppId) {
        let Some(context) = self.app_info_context(cx, app_id) else {
            return;
        };
        self.ui
            .launcher_app_info(cx, ids!(app_info_modal.content))
            .show(cx, context);
        self.ui.modal(cx, ids!(app_info_modal)).open(cx);
    }

    /// Re-renders the page in place after an action that changed what it
    /// displays (force stop, clear data), so the numbers/state stay honest.
    fn refresh_app_info(&mut self, cx: &mut Cx, app_id: &MiniAppId) {
        if !self.ui.modal(cx, ids!(app_info_modal)).is_open() {
            return;
        }
        if let Some(context) = self.app_info_context(cx, app_id) {
            self.ui
                .launcher_app_info(cx, ids!(app_info_modal.content))
                .show(cx, context);
        }
    }

    fn app_info_context(&mut self, cx: &mut Cx, app_id: &MiniAppId) -> Option<AppInfoContext> {
        let manifest = self.app_state.registry.get(app_id)?.clone();
        // A built-in with a persisted copy has been modified by the user.
        let overridden = manifest.builtin
            && self
                .app_state
                .layout
                .user_apps
                .iter()
                .any(|a| a.id == *app_id);
        let (mut home_icons, mut home_widgets) = (0, 0);
        for page in &self.app_state.layout.pages {
            for item in &page.items {
                if item.app_id() != app_id {
                    continue;
                }
                match item.kind {
                    PlacedKind::App { .. } => home_icons += 1,
                    PlacedKind::Widget { .. } => home_widgets += 1,
                }
            }
        }
        Some(AppInfoContext {
            app_id: app_id.clone(),
            name: manifest.name.clone(),
            icon: manifest.icon.clone(),
            builtin: manifest.builtin,
            overridden,
            running: self.mini_app_screen(cx).is_running(app_id),
            allow_net: manifest.allow_net,
            has_widget: manifest.widget.is_some(),
            home_icons,
            home_widgets,
            in_dock: self.app_state.layout.dock.contains(app_id),
            data_bytes: persistence::app_data_bytes(app_id),
            code_bytes: manifest.source.len() as u64
                + manifest
                    .widget
                    .as_ref()
                    .map(|w| w.source.len() as u64)
                    .unwrap_or(0),
            versions: persistence::list_versions(app_id),
            utc_offset_secs: utc_offset_secs(),
        })
    }

    /// Restores an archived version: snapshots the current state first (so the
    /// restore itself is undoable), then swaps the old source + identity back
    /// in and restarts the app so the change is live.
    fn restore_version(&mut self, cx: &mut Cx, app_id: &MiniAppId, stamp: &str) {
        self.ui.modal(cx, ids!(app_info_modal)).close(cx);
        let Some(mut manifest) = self.app_state.registry.get(app_id).cloned() else {
            return;
        };
        let Some(source) = persistence::load_version_source(app_id, stamp) else {
            self.flash_create_bar(cx, "That version is missing");
            return;
        };
        let Some(version) = persistence::list_versions(app_id)
            .into_iter()
            .find(|v| v.stamp == stamp)
        else {
            self.flash_create_bar(cx, "That version is missing");
            return;
        };
        self.snapshot_current(app_id, "Before restore");
        manifest.source = source;
        manifest.name = version.name.clone();
        manifest.icon = version.icon.clone();
        manifest.tint = version.tint;
        let name = manifest.name.clone();
        self.write_app_through(cx, manifest);
        // The bar belongs to a running generation while one is in flight —
        // flashing over it would hide the only Stop button.
        if self.generation.is_none() {
            self.flash_create_bar(cx, &format!("{name} restored ✓"));
        }
    }

    /// Swaps an app's manifest in place everywhere it's cached: the persisted
    /// user apps (a built-in gains an override here), the live registry, and
    /// the on-disk `apps/<id>/` files — then restarts the app and rebuilds the
    /// icon/tile widgets that baked in its old identity or source. Shared by
    /// an AI modification and a version restore.
    fn write_app_through(&mut self, cx: &mut Cx, manifest: MiniAppManifest) {
        let id = manifest.id.clone();
        if let Some(slot) = self
            .app_state
            .layout
            .user_apps
            .iter_mut()
            .find(|m| m.id == id)
        {
            *slot = manifest.clone();
        } else {
            // A built-in modified for the first time: its override joins
            // user_apps and shadows the stock manifest from now on (the
            // manifest keeps builtin: true, so it stays non-uninstallable —
            // version history is how you get the original back).
            self.app_state.layout.user_apps.push(manifest.clone());
        }
        if let Err(e) = persistence::save_user_app(&manifest) {
            error!("couldn't persist app '{id}': {e}");
        }
        self.app_state.registry.insert(manifest);
        // The running instance holds the OLD script; restart so the next open
        // evaluates the new one.
        self.mini_app_screen(cx).force_stop(cx, &id);
        // Cached icon widgets bake in name/glyph/tint — rebuild them so the
        // grid and dock show the new identity, not the old one.
        self.home_pager(cx)
            .refresh_app_icons(cx, &self.app_state.layout, &id);
        // ...and cached WIDGET tiles keep running the OLD source — drop them so
        // they rebuild from the new script (and rebind the sandbox).
        self.home_pager(cx)
            .drop_app_widget_tiles(cx, &self.app_state.layout, &id);
        makepad_widgets::widget_async::gc_dead_splash_isolates(cx);
        if let Some(mut dock) = self
            .ui
            .widget(cx, ids!(dock))
            .borrow_mut::<crate::launcher::dock::LauncherDock>()
        {
            dock.refresh_icon(cx, &id);
        }
        self.app_state.layout_dirty = true;
        cx.redraw_all();
    }

    /// Applies a finished refine: swap the manifest in place (registry + the
    /// persisted user apps), then force-stop the app so its next open runs the
    /// new script instead of the kept-alive old VM.
    fn install_refined(&mut self, cx: &mut Cx, manifest: MiniAppManifest, note: String) {
        let id = manifest.id.clone();
        let name = manifest.name.clone();
        // The app may have been uninstalled while the agent worked; applying
        // the result would resurrect it (alongside its uninstall tombstone —
        // a state nothing else can produce). Drop the result instead.
        if !self.app_state.registry.contains(&id) {
            self.flash_create_bar(cx, "That app is no longer installed");
            return;
        }
        // Snapshot what's being replaced FIRST, so this change is revertible.
        self.snapshot_current(&id, &note);
        self.write_app_through(cx, manifest);
        self.flash_create_bar(cx, &format!("{name} updated ✓"));
    }

    /// Feeds queued agent events through the pipeline and reflects the result
    /// in the create bar. Called on every event; cheap when nothing is queued.
    fn advance_generation(&mut self, cx: &mut Cx) {
        use crate::generate::pipeline::GenOutcome;
        let Some(generation) = &mut self.generation else {
            return;
        };
        let request = generation.request().to_string();
        match generation.advance(cx) {
            GenOutcome::Working => {
                let status = generation.status().to_string();
                // The console's detail: the WHOLE trail (it scrolls) plus the
                // live code tail. No last-N window — dropping older lines both
                // loses the history and makes the box shrink under you.
                let log = generation.activity().join("\n");
                let stream = generation.stream_tail().to_string();
                let label = self.ui.label(cx, ids!(create_status));
                if label.text() != status {
                    label.set_text(cx, &status);
                }
                if self.activity_active && !self.activity_collapsed {
                    let mut grew = false;
                    let log_label = self.ui.label(cx, ids!(activity_log));
                    if log_label.text() != log {
                        log_label.set_text(cx, &log);
                        grew = true;
                    }
                    let stream_label = self.ui.label(cx, ids!(activity_stream));
                    if stream_label.text() != stream {
                        stream_label.set_text(cx, &stream);
                        grew = true;
                    }
                    // Follow the tail like a terminal — until the user
                    // scrolls back, at which point they're reading and we leave
                    // them alone (see `console_follow`).
                    if grew && self.console_follow {
                        self.scroll_console_to_end(cx);
                    }
                }
            }
            GenOutcome::Ready { manifest, refine_of } => {
                self.generation = None;
                cx.stop_timer(self.generation_watchdog);
                if refine_of.is_some() {
                    self.install_refined(cx, *manifest, request);
                } else {
                    self.install_generated(cx, *manifest);
                }
            }
            GenOutcome::Failed(reason) => {
                self.generation = None;
                cx.stop_timer(self.generation_watchdog);
                self.flash_create_bar(cx, &reason);
                // Setup-class failures (no provider, agent binary missing) get
                // the guided fix instead of just an error flash.
                if reason.contains("No LLM provider") || reason.contains("isn't runnable") {
                    self.open_setup_modal(cx);
                }
            }
        }
    }

    /// Installs a freshly generated app: persists it with the user apps,
    /// registers it live, and drops its icon onto the home screen.
    fn install_generated(&mut self, cx: &mut Cx, mut manifest: MiniAppManifest) {
        // The id was minted against a snapshot taken when the generation
        // started; the world may have moved (store installs, another
        // generation). Re-unique it against the LIVE taken set — which also
        // covers uninstall tombstones, so a generated app can never capture a
        // removed store app's identity (and be silently resurrected later).
        let taken = self.taken_app_ids();
        if taken.iter().any(|t| t == &manifest.id) {
            manifest.id = crate::generate::pipeline::unique_id(&manifest.name, &taken);
        }
        let id = manifest.id.clone();
        let name = manifest.name.clone();
        if let Err(e) = persistence::save_user_app(&manifest) {
            error!("couldn't persist generated app '{id}': {e}");
        }
        self.app_state.layout.user_apps.push(manifest.clone());
        self.app_state.registry.insert(manifest);
        self.add_app_to_home(&id);
        self.app_state.layout_dirty = true;
        self.flash_create_bar(cx, &format!("{name} added ✓"));
        cx.redraw_all();
    }

    /// Opens the provider-setup modal (✨ tap, or automatically after a
    /// setup-class generation failure).
    fn open_setup_modal(&mut self, cx: &mut Cx) {
        self.ui
            .text_input(cx, ids!(setup_key_input))
            .set_text(cx, "");
        self.ui.label(cx, ids!(setup_detected)).set_text(cx, "");
        self.ui.modal(cx, ids!(setup_modal)).open(cx);
    }

    /// Live feedback while a key is typed/pasted into the setup modal.
    fn update_setup_detected(&mut self, cx: &mut Cx, key: &str) {
        let label = self.ui.label(cx, ids!(setup_detected));
        if key.trim().is_empty() {
            label.set_text(cx, "");
            return;
        }
        match crate::generate::setup::provider_for_key(key) {
            Some(p) => label.set_text(cx, &format!("Detected: {p}")),
            None => label.set_text(cx, "Unrecognized key format"),
        }
    }

    /// Saves the pasted key as a minimal octos config and closes the modal.
    fn save_setup_key(&mut self, cx: &mut Cx) {
        use crate::generate::setup;
        let key = self.ui.text_input(cx, ids!(setup_key_input)).text();
        let key = key.trim().to_string();
        let label = self.ui.label(cx, ids!(setup_detected));
        if key.is_empty() {
            label.set_text(cx, "Paste a key first");
            return;
        }
        let Some(provider) = setup::provider_for_key(&key) else {
            label.set_text(
                cx,
                "Unrecognized key — use `octos init` for custom providers",
            );
            return;
        };
        match setup::write_min_config(&setup::config_write_dir(), provider, &key) {
            Ok(()) => {
                self.ui.modal(cx, ids!(setup_modal)).close(cx);
                self.flash_create_bar(cx, &format!("{provider} ready — try creating an app"));
            }
            Err(e) => label.set_text(cx, &e),
        }
    }

    /// Reveals the Send button only when the prompt has something in it, so
    /// the idle bar stays a clean single line until you start typing.
    fn sync_create_send(&mut self, cx: &mut Cx, text: &str) {
        // Both, so the button's own flag matches what's on screen — a widget
        // hidden only by its parent still reports itself visible.
        let show = !text.trim().is_empty();
        self.ui.widget(cx, ids!(create_send_wrap)).set_visible(cx, show);
        self.ui.widget(cx, ids!(create_send)).set_visible(cx, show);
        self.ui.widget(cx, ids!(create_bar)).redraw(cx);
    }

    /// The option rows, and the segmented control inside each. Separate ids
    /// per row because GlassSegmented's labels live in the DSL, so each knob
    /// has its own instance rather than one reused template.
    const OPTION_IDS: [&'static [LiveId]; 3] = [ids!(opt_0), ids!(opt_1), ids!(opt_2)];
    const SEGMENT_IDS: [&'static [LiveId]; 3] =
        [ids!(opt_0.ao_seg_0), ids!(opt_1.ao_seg_1), ids!(opt_2.ao_seg_2)];

    /// Fills the options row from the ACTIVE backend's knobs — which controls
    /// exist is the backend's call, not ours, because a knob nothing reads is
    /// worse than a missing one. Also decides whether the row shows at all:
    /// it's for composing, so it rides the prompt's focus/content, and a
    /// backend with no knobs (a foreign ACP agent) never shows it.
    /// Opens the options row (idempotent).
    fn open_agent_options(&mut self, cx: &mut Cx) {
        if !self.create_options_open {
            self.create_options_open = true;
            self.sync_agent_options(cx, true);
        }
    }

    /// Closes it — the composer is finished with.
    fn close_agent_options(&mut self, cx: &mut Cx) {
        if self.create_options_open {
            self.create_options_open = false;
            self.sync_agent_options(cx, false);
        }
    }

    fn sync_agent_options(&mut self, cx: &mut Cx, reveal: bool) {
        let backend = Backend::detect();
        let knobs = backend.knobs();
        let show = reveal && !knobs.is_empty();
        self.ui.widget(cx, ids!(create_options)).set_visible(cx, show);
        if show {
            self.ui
                .label(cx, ids!(create_backend))
                .set_text(cx, &backend.display_name());
        }
        // Rows are addressed by KnobId, never by position: the segmented
        // controls carry their labels in the DSL, so a backend that offers
        // effort but no model still has to land in the effort row.
        for (slot, id) in Self::OPTION_IDS.iter().enumerate() {
            let Some(knob) = knobs.iter().find(|k| k.id.row() == slot) else {
                self.ui.widget(cx, id).set_visible(cx, false);
                continue;
            };
            let index = knob.index_of(self.agent_prefs.get(knob.id));
            self.ui.widget(cx, id).set_visible(cx, show);
            self.ui.label(cx, &[*id, ids!(ao_label)].concat()).set_text(cx, knob.label);
            // The effort row has two ladders declared (with and without
            // xhigh); show whichever matches what the runtime accepts.
            let seg_id: &[LiveId] = if knob.id == KnobId::Effort {
                // Pick by CONTENT, not by count: octos's ladder is also four
                // levels but has no xhigh, and offering it there would write a
                // value octos's own enum rejects.
                let extended = knob
                    .options
                    .iter()
                    .any(|(_, v)| v == prefs::CLAUDE_EFFORT_XHIGH.1);
                self.ui
                    .widget(cx, ids!(opt_1.ao_seg_1))
                    .set_visible(cx, !extended);
                self.ui
                    .widget(cx, ids!(opt_1.ao_seg_1x))
                    .set_visible(cx, extended);
                if extended { ids!(opt_1.ao_seg_1x) } else { ids!(opt_1.ao_seg_1) }
            } else {
                Self::SEGMENT_IDS[slot]
            };
            if let Some(mut seg) = self.ui.widget(cx, seg_id).borrow_mut::<GlassSegmented>() {
                seg.selected = index;
            }
        }
        self.ui.widget(cx, ids!(create_bar)).redraw(cx);
    }

    /// Records a pick from one of the dropdowns and persists it, so the
    /// choice survives a restart rather than resetting every launch.
    fn pick_agent_option(&mut self, cx: &mut Cx, slot: usize, index: usize) {
        let knobs = Backend::detect().knobs();
        let Some(knob) = knobs.iter().find(|k| k.id.row() == slot) else {
            return;
        };
        self.agent_prefs.set(knob.id, knob.value_at(index));
        let _ = persistence::save_agent_prefs(&self.agent_prefs);
        self.ui.widget(cx, ids!(create_bar)).redraw(cx);
    }

    /// Matches the console's output area to the busy + collapse state, and
    /// points the chevron the right way. Hidden entirely in edit mode.
    fn sync_activity_panel(&mut self, cx: &mut Cx) {
        // The console is the bar's busy state; hiding it drops the bar back to
        // the one-line status. The chevron lives in the ✨'s slot while the
        // agent works, so exactly one of the two is ever on screen.
        let active = (self.activity_active || self.console_finished) && !self.app_state.edit_mode;
        self.ui
            .widget(cx, ids!(create_output))
            .set_visible(cx, active && !self.activity_collapsed);
        self.ui.widget(cx, ids!(create_glyph)).set_visible(cx, !active);
        // Both, so the button's own flag matches what's on screen — a widget
        // hidden only by its parent still reports itself visible.
        self.ui.widget(cx, ids!(create_toggle_wrap)).set_visible(cx, active);
        self.ui.widget(cx, ids!(create_toggle)).set_visible(cx, active);
        if let Some(mut arrow) = self
            .ui
            .widget(cx, ids!(create_arrow))
            .borrow_mut::<ExpandArrow>()
        {
            arrow.set_is_open(cx, !self.activity_collapsed, Animate::Yes);
        }
        self.ui.widget(cx, ids!(create_bar)).redraw(cx);
    }

    /// Height of a widget's last draw, or None if it hasn't drawn one — reading
    /// the rect of an undrawn area logs a "mark/sweep" error and returns zero,
    /// which is indistinguishable from a genuinely empty widget.
    fn drawn_height(&mut self, cx: &mut Cx, area: Area) -> Option<f64> {
        area.is_valid(cx).then(|| area.rect(cx).size.y)
    }

    /// How tall the console's content wants to be — the inner `console_body`,
    /// which is `Fit` and free to overrun the viewport. None until it's drawn.
    fn console_content_height(&mut self, cx: &mut Cx) -> Option<f64> {
        let body = self.ui.widget(cx, ids!(console_body)).area();
        self.drawn_height(cx, body)
    }

    /// Pins the console to its last line. The target is COMPUTED, not a big
    /// sentinel: `set_scroll_pos` clamps only once the view has built its
    /// scroll bars, and before that it writes `layout.scroll` raw — an
    /// over-large value there doesn't mean "the end", it throws the log a
    /// million points off screen.
    fn scroll_console_to_end(&mut self, cx: &mut Cx) {
        let out = self.ui.widget(cx, ids!(create_output)).area();
        let (Some(view_h), Some(content_h)) =
            (self.drawn_height(cx, out), self.console_content_height(cx))
        else {
            return;
        };
        let end = (content_h - view_h).max(0.0);
        self.ui
            .view(cx, ids!(create_output))
            .set_scroll_pos(cx, dvec2(0.0, end));
    }

    /// Writes the console's height. Straight onto the walk rather than through
    /// `script_apply_eval!` — this runs per event, and an eval body has none of
    /// the DSL's `use`s in scope, so bare `Fit`/`FitBound` there resolve to
    /// nothing and the height silently fails to apply (SPLASH_FINDINGS #8).
    fn set_console_height(&mut self, cx: &mut Cx, height: Option<f64>) {
        if let Some(mut out) = self.ui.view(cx, ids!(create_output)).borrow_mut() {
            out.walk.height = Size::Fixed(height.unwrap_or(CONSOLE_START_HEIGHT));
        }
        self.ui.widget(cx, ids!(create_bar)).redraw(cx);
    }

    /// Sizes the console from its CONTENT, ratcheting upward only. A scrolling
    /// view can't be `Fit` — it takes whatever height it's offered — so the
    /// height is driven from here: grow with the log, stop at the cap, and
    /// never come back down (a box that shrinks under the text you're reading
    /// is worse than one that's briefly too big).
    fn sync_console_size(&mut self, cx: &mut Cx) {
        if !self.activity_active || self.activity_collapsed {
            return;
        }
        let Some(content) = self.console_content_height(cx) else {
            return;
        };
        // Grow until the console is a hair above the dock — the real limit is
        // "don't cover the dock", not an abstract fraction of the screen.
        let out_top = self.ui.widget(cx, ids!(create_output)).area().rect(cx).pos.y;
        let dock_top = self.app_state.dock_rect.pos.y;
        let cap = if dock_top > out_top {
            dock_top - out_top - CONSOLE_DOCK_GAP
        } else {
            self.ui.widget(cx, ids!(create_layer)).area().rect(cx).size.y * CONSOLE_MAX_FRACTION
        };
        let want = content.min(cap).max(self.console_floor);
        if want <= self.console_floor + 0.5 {
            return;
        }
        self.console_floor = want;
        self.set_console_height(cx, Some(want));
    }

    /// Busy state: the prompt's space becomes the agent console — status line
    /// and stop button over the activity log and the streamed code tail.
    fn set_create_bar_busy(&mut self, cx: &mut Cx, status: &str) {
        self.ui.widget(cx, ids!(create_idle)).set_visible(cx, false);
        self.ui.widget(cx, ids!(create_busy)).set_visible(cx, true);
        // The options belong to the composer, not the console.
        self.close_agent_options(cx);
        self.ui.label(cx, ids!(create_status)).set_text(cx, status);
        self.ui.widget(cx, ids!(create_cancel)).set_visible(cx, true);
        // NOT `status` — the header already says that, and echoing it makes
        // the console open on the same sentence twice. This matches the trail's
        // real first entry, which the first pipeline tick overwrites anyway.
        self.ui.label(cx, ids!(activity_log)).set_text(cx, "Starting agent…");
        self.ui.label(cx, ids!(activity_stream)).set_text(cx, "");
        self.activity_active = true;
        // A new run starts from the composer's height and follows its own tail.
        self.console_floor = 0.0;
        self.console_follow = true;
        self.set_console_height(cx, None);
        self.ui.view(cx, ids!(create_output)).set_scroll_pos(cx, dvec2(0.0, 0.0));
        self.sync_activity_panel(cx);
        self.ui.widget(cx, ids!(create_bar)).redraw(cx);
    }

    /// Idle state: just the input.
    fn set_create_bar_idle(&mut self, cx: &mut Cx) {
        self.ui.widget(cx, ids!(create_idle)).set_visible(cx, true);
        // Whatever the prompt still holds decides whether Send comes back.
        let text = self.ui.text_input(cx, ids!(create_input)).text();
        self.sync_create_send(cx, &text);
        self.ui.widget(cx, ids!(create_busy)).set_visible(cx, false);
        self.activity_active = false;
        self.console_finished = false;
        self.sync_activity_panel(cx);
        self.ui.widget(cx, ids!(create_bar)).redraw(cx);
    }

    /// Shows a short-lived result message ("Timer added ✓" / an error) in the
    /// bar (no stop button), then resets to idle after a beat. If edit mode is
    /// hiding the bar, the flash is held and replayed when the bar returns —
    /// otherwise a failure that lands mid-edit would vanish unseen.
    fn flash_create_bar(&mut self, cx: &mut Cx, msg: &str) {
        if self.app_state.edit_mode {
            self.pending_create_flash = Some(msg.to_string());
            return;
        }
        self.ui.widget(cx, ids!(create_idle)).set_visible(cx, false);
        self.ui.widget(cx, ids!(create_busy)).set_visible(cx, true);
        self.ui.label(cx, ids!(create_status)).set_text(cx, msg);
        self.ui.widget(cx, ids!(create_cancel)).set_visible(cx, false);
        // The run is over, but its output stays: what the agent did is worth
        // reading after the fact, and a panel that erases itself three seconds
        // after finishing takes the explanation with it. The log keeps its
        // final line; dismissal is the user's call (a press outside the bar).
        let log = self.ui.label(cx, ids!(activity_log));
        let done = format!("{}\n— {msg}", log.text());
        log.set_text(cx, done.trim_start_matches('\n'));
        self.console_finished = true;
        self.sync_activity_panel(cx);
        self.scroll_console_to_end(cx);
        self.ui.widget(cx, ids!(create_bar)).redraw(cx);
    }

    /// Shows/hides the edit-mode management bar to match edit mode, and keeps
    /// its grid-size labels current.
    fn sync_edit_bar(&mut self, cx: &mut Cx) {
        let editing = self.app_state.edit_mode;
        if editing != self.edit_bar_shown {
            self.edit_bar_shown = editing;
            // The create bar yields the top of the screen to the edit bar —
            // and its reserved slot in the column goes with it, so the grid
            // reclaims the space.
            self.ui.widget(cx, ids!(create_bar)).set_visible(cx, !editing);
            self.ui.widget(cx, ids!(create_slot)).set_visible(cx, !editing);
            if editing {
                self.close_agent_options(cx);
            }
            // ...and takes the agent console with it.
            self.sync_activity_panel(cx);
            // Replay a result flash that landed while the bar was hidden.
            if !editing {
                if let Some(msg) = self.pending_create_flash.take() {
                    self.flash_create_bar(cx, &msg);
                }
            }
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
        // `is_showing`, not `is_fully_open`: during the open/close zoom the home
        // used to stay visible so the app animated over real content, but a
        // glass widget's refraction overlay renders in FRONT of the covering
        // layer, so widgets appeared on top of the app for the length of the
        // animation. Widgets are background content, like app icons — the zoom
        // now plays over the wallpaper instead.
        let covered = self.search_overlay(cx).is_open()
            || self.mini_app_screen(cx).is_showing()
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
            && !self.ui.modal(cx, ids!(app_info_modal)).is_open()
            && !self.ui.modal(cx, ids!(setup_modal)).is_open()
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
        // If the bar was armed to modify THIS app, disarm it — otherwise it
        // keeps a "✏️ <Name>: " prefix for something that no longer exists.
        if self.pending_modify.as_ref() == Some(app_id) {
            self.pending_modify = None;
            self.ui.text_input(cx, ids!(create_input)).set_text(cx, "");
        }
        // Kill any live widget tiles for this app and reclaim their isolates
        // BEFORE deleting the data dir, or a widget timer could fire against a
        // removed jail. (force_stop above only tore down the app-screen host.)
        self.home_pager(cx)
            .drop_app_widget_tiles(cx, &self.app_state.layout, app_id);
        makepad_widgets::widget_async::gc_dead_splash_isolates(cx);
        self.app_state.layout.user_apps.retain(|a| &a.id != app_id);
        // OS convention: uninstalling deletes the app's code dir AND its data.
        persistence::remove_user_app(app_id);
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
        if self.ui.modal(cx, ids!(setup_modal)).is_open() {
            self.ui.modal(cx, ids!(setup_modal)).close(cx);
            return true;
        }
        if self.ui.modal(cx, ids!(app_info_modal)).is_open() {
            self.ui.modal(cx, ids!(app_info_modal)).close(cx);
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
            } else if state == "longprompt" {
                // Screenshot the expanded multi-line prompt.
                self.ui.text_input(cx, ids!(create_input)).set_text(
                    cx,
                    "a habit tracker with:\n\
                     - three habits I can rename\n\
                     - a 7-day streak grid per habit\n\
                     - tap a day to toggle it done\n\
                     - a running total at the top\n\
                     - everything saved between launches",
                );
                self.ui.widget(cx, ids!(create_input)).set_key_focus(cx);
                let text = self.ui.text_input(cx, ids!(create_input)).text();
                self.sync_create_send(cx, &text);
                self.open_agent_options(cx);
            } else if let Some(app_id) = state.strip_prefix("modify:") {
                // Screenshot the prefilled+focused create bar.
                self.arm_modify(cx, &app_id.to_string());
            } else if let Some(app_id) = state.strip_prefix("appinfo:") {
                // Screenshot the version list with a couple of fake entries.
                // Unlike the other debug states this WRITES files, so it's
                // limited to throwaway runs — never a real profile.
                let app_id = app_id.to_string();
                if !Self::is_fresh_run() {
                    error!("HOST_LAUNCHER_DEBUG_STATE=history: needs HOST_LAUNCHER_FRESH=1 (it writes snapshots)");
                } else if let Some(manifest) = self.app_state.registry.get(&app_id).cloned() {
                    let now = crate::mini_apps::versions::now_unix();
                    for (ago, note) in [
                        (60 * 9, "make the notes app show a word count"),
                        (60 * 60 * 26, "add a dark theme"),
                    ] {
                        let v = crate::mini_apps::versions::version_of(
                            &manifest,
                            note,
                            now - ago,
                            utc_offset_secs(),
                        );
                        let _ = persistence::snapshot_version(&manifest, v);
                    }
                }
                self.open_app_info(cx, &app_id);
            } else if state == "confirmremove" {
                self.app_state.edit_mode = true;
                self.ui
                    .label(cx, ids!(confirm_body))
                    .set_text(cx, "Remove Calculator from the home screen?");
                self.ui.modal(cx, ids!(confirm_remove_modal)).open(cx);
            } else if state == "genbusy" {
                // The create bar mid-generation (status + cancel), no agent.
                self.set_create_bar_busy(cx, "Writing the app…");
            } else if state == "genlog" {
                // A long-running generation: the console filled past its cap,
                // for eyeballing the scroll behavior and the height ceiling.
                self.set_create_bar_busy(cx, "Fixing the app (try 1)…");
                let log = (1 ..= 24)
                    .map(|i| match i % 4 {
                        0 => format!("🔧 Read splash_guide.md ({i})"),
                        1 => format!("Agent connected — writing the app ({i})"),
                        2 => format!("⚠ line {i}: expected `}}` to close the block"),
                        _ => format!("Sending errors back (repair {i})"),
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                self.ui.label(cx, ids!(activity_log)).set_text(cx, &log);
                self.ui.label(cx, ids!(activity_stream)).set_text(
                    cx,
                    "    let total = habits.map(|h| h.done.len()).sum()\n\
                         label.set_text(cx, &format!(\"{total} done\"))\n\
                     }",
                );
            } else if state == "setup" {
                self.open_setup_modal(cx);
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
        // The AI create bar: return submits the request (a create, or the
        // armed refine), Stop cancels the in-flight generation.
        // The Send button only exists while there's something to send.
        if let Some(text) = self.ui.text_input(cx, ids!(create_input)).changed(actions) {
            self.sync_create_send(cx, &text);
            if !text.trim().is_empty() {
                self.open_agent_options(cx);
            }
        }
        // Focusing the prompt opens the options; nothing closes them until the
        // composer is done, so a click on a control can't dismiss its own row.
        let uid = self.ui.text_input(cx, ids!(create_input)).widget_uid();
        if actions
            .filter_widget_actions_cast::<TextInputAction>(uid)
            .any(|a| matches!(a, TextInputAction::KeyFocus))
        {
            self.open_agent_options(cx);
        }
        for slot in 0 .. Self::SEGMENT_IDS.len() {
            let seg = self.ui.glass_segmented(cx, Self::SEGMENT_IDS[slot]);
            if seg.changed(actions) {
                let index = seg.selected();
                self.pick_agent_option(cx, slot, index);
            }
        }
        // The extended effort ladder is a second control in the same row.
        let extended = self.ui.glass_segmented(cx, ids!(opt_1.ao_seg_1x));
        if extended.changed(actions) {
            let index = extended.selected();
            self.pick_agent_option(cx, KnobId::Effort.row(), index);
        }
        let submitted = self
            .ui
            .text_input(cx, ids!(create_input))
            .returned(actions)
            .map(|(text, _)| text)
            .or_else(|| {
                // A plain Button now (see create_bar.rs: a glass button's lens
                // would hide the icon), so it must be read as one — a
                // glass_button cast silently never fires.
                self.ui
                    .button(cx, ids!(create_send))
                    .clicked(actions)
                    .then(|| self.ui.text_input(cx, ids!(create_input)).text())
            });
        if let Some(text) = submitted {
            // Only disarm once we know the submit is going somewhere: a bail
            // (busy/edit-mode) must leave the bar armed with its prefix intact.
            let armed = self.pending_modify.clone();
            match self.resolve_submit(&text, armed) {
                Some((app_id, request)) => {
                    if self.start_modify(cx, &app_id, request) {
                        self.pending_modify = None;
                    }
                }
                None => {
                    self.pending_modify = None;
                    self.start_generation(cx, text);
                }
            }
        }
        if self.ui.glass_button(cx, ids!(create_cancel)).clicked(actions) {
            self.cancel_generation(cx);
        }

        // Provider setup modal: ✨ opens it; pasting a key live-detects the
        // provider; Save writes a minimal octos config.
        if self.ui.button(cx, ids!(create_glyph)).clicked(actions) {
            self.open_setup_modal(cx);
        }
        if let Some(key) = self.ui.text_input(cx, ids!(setup_key_input)).changed(actions) {
            self.update_setup_detected(cx, &key);
        }
        if self.ui.glass_button(cx, ids!(setup_save)).clicked(actions) {
            self.save_setup_key(cx);
        }
        if self.ui.glass_button(cx, ids!(setup_cancel)).clicked(actions) {
            self.ui.modal(cx, ids!(setup_modal)).close(cx);
        }

        // The chevron in the ✨'s slot hides/shows the output. Re-showing it
        // keeps the height it had reached (console_floor is untouched), so it
        // comes back exactly as you left it.
        if self.ui.button(cx, ids!(create_toggle)).clicked(actions) {
            self.activity_collapsed = !self.activity_collapsed;
            self.sync_activity_panel(cx);
        }

        for action in actions {
            if let Some(widget_action) = action.as_widget_action() {
                match widget_action.cast::<HomePagerAction>() {
                    HomePagerAction::DropIntoDock { app_id, index } => {
                        let dock = &mut self.app_state.layout.dock;
                        // Guard against a duplicate if the app was already docked.
                        dock.retain(|id| id != &app_id);
                        if dock.len() < MAX_DOCK_ITEMS {
                            let at = index.min(dock.len());
                            dock.insert(at, app_id);
                        } else {
                            // Dock is full — keep the icon rather than lose it.
                            self.add_app_to_home(&app_id);
                        }
                        self.app_state.layout_dirty = true;
                        cx.redraw_all();
                    }
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
                            None,
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
                    ContextMenuAction::Modify(app_id) => {
                        self.close_context_menu(cx);
                        self.arm_modify(cx, &app_id);
                    }
                    ContextMenuAction::AppInfo(app_id) => {
                        self.close_context_menu(cx);
                        self.open_app_info(cx, &app_id);
                    }
                    ContextMenuAction::Uninstall(app_id) => {
                        self.close_context_menu(cx);
                        self.uninstall_app(cx, &app_id);
                    }
                    ContextMenuAction::None => (),
                }

                match widget_action.cast::<AppInfoAction>() {
                    AppInfoAction::Close => {
                        self.ui.modal(cx, ids!(app_info_modal)).close(cx);
                    }
                    AppInfoAction::Open(app_id) => {
                        self.ui.modal(cx, ids!(app_info_modal)).close(cx);
                        let from = Rect {
                            pos: dvec2(180.0, 400.0),
                            size: dvec2(56.0, 56.0),
                        };
                        self.open_app(cx, &app_id, from);
                    }
                    AppInfoAction::Modify(app_id) => {
                        self.ui.modal(cx, ids!(app_info_modal)).close(cx);
                        self.arm_modify(cx, &app_id);
                    }
                    AppInfoAction::ForceStop(app_id) => {
                        self.mini_app_screen(cx).force_stop(cx, &app_id);
                        self.refresh_app_info(cx, &app_id);
                    }
                    AppInfoAction::ClearData(app_id) => {
                        crate::persistence::clear_app_data(&app_id);
                        // The running instance may hold open handles/state from
                        // the data we just deleted; restart it clean.
                        self.mini_app_screen(cx).force_stop(cx, &app_id);
                        self.refresh_app_info(cx, &app_id);
                    }
                    AppInfoAction::Uninstall(app_id) => {
                        self.ui.modal(cx, ids!(app_info_modal)).close(cx);
                        self.uninstall_app(cx, &app_id);
                    }
                    AppInfoAction::Restore { app_id, stamp } => {
                        self.restore_version(cx, &app_id, &stamp);
                    }
                    AppInfoAction::None => (),
                }

                match widget_action.cast::<DockAction>() {
                    DockAction::OpenApp { app_id, from_rect } => {
                        self.open_app(cx, &app_id, from_rect);
                    }
                    DockAction::ShowContextMenu { app_id, anchor } => {
                        self.show_context_menu(cx, &app_id, None, None, MenuSource::Drawer, anchor);
                    }
                    DockAction::DragOutApp { app_id, area, abs } => {
                        // Lift it out of the dock and let the same touch keep
                        // dragging it over the home grid (dropping it back on the
                        // dock re-inserts it, which is how reordering works).
                        let instance = self.app_state.layout.alloc_instance();
                        // Remember the slot so an invalid drop can put it back.
                        let slot = self
                            .app_state
                            .layout
                            .dock
                            .iter()
                            .position(|id| id == &app_id);
                        let pager = self.home_pager(cx);
                        let started = pager.begin_external_drag(
                            cx,
                            &self.app_state.layout,
                            app_id.clone(),
                            instance,
                            abs,
                            area,
                            slot,
                        );
                        if started {
                            // The long press that lifted the icon also opened its
                            // shortcut menu; sliding out of it into a drag has to
                            // take the menu with it, exactly as a grid icon's does.
                            self.close_context_menu(cx);
                            self.app_state.layout.dock.retain(|id| id != &app_id);
                            self.app_state.layout_dirty = true;
                            cx.redraw_all();
                        }
                    }
                    DockAction::RemoveFromDock(app_id) => {
                        // Same confirm-first flow as a home icon's × badge.
                        let name = self
                            .app_state
                            .registry
                            .get(&app_id)
                            .map(|m| m.name.clone())
                            .unwrap_or_else(|| app_id.clone());
                        self.pending_confirm = Some(PendingConfirm::RemoveFromDock(app_id));
                        self.ui.label(cx, ids!(confirm_title)).set_text(cx, "Remove?");
                        self.ui
                            .glass_button(cx, ids!(confirm_remove))
                            .set_text(cx, "Remove");
                        self.ui
                            .label(cx, ids!(confirm_body))
                            .set_text(cx, &format!("Remove {name} from the dock?"));
                        self.ui.modal(cx, ids!(confirm_remove_modal)).open(cx);
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

        // NOTE: makepad's Modal::dismissed() can't be relied on for cleanup here —
        // it emits its Dismissed action under the content widget's uid but checks
        // the modal's own uid, so it never matches. (The pager clears a leaked
        // resize_hint itself; see its reconcile in handle_event.) Instead, tear
        // down the gallery's live-preview isolate whenever its modal is closed but
        // the picker is still parked on the detail stage — covering a scrim tap,
        // which closes the modal without an in-panel Back/Add.
        if !self.ui.modal(cx, ids!(widget_picker_modal)).is_open() {
            let picker = self
                .ui
                .launcher_widget_picker(cx, ids!(widget_picker_modal.content));
            if picker.is_showing_detail() {
                picker.reset(cx);
            }
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
                Some(PendingConfirm::RemoveFromDock(app_id)) => {
                    self.app_state.layout.dock.retain(|id| id != &app_id);
                    self.app_state.layout_dirty = true;
                    cx.redraw_all();
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

        // Drive the in-flight app generation, if any. Its agent events arrive
        // from a reader thread with a SignalToUI wakeup, so at least one event
        // reaches here per batch; the drain itself is cheap when idle.
        self.advance_generation(cx);
        if self.create_reset_timer.is_event(event).is_some() {
            self.set_create_bar_idle(cx);
        }
        // A live-but-silent agent emits no events at all, so stall detection
        // needs its own clock.
        if self.generation_watchdog.is_event(event).is_some()
            && self.generation.as_ref().is_some_and(|g| g.is_stalled())
        {
            self.generation = None;
            cx.stop_timer(self.generation_watchdog);
            self.flash_create_bar(cx, "The agent stopped responding");
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
        // ...and where the dock is, so a drag released over it drops into the dock.
        self.app_state.dock_rect = self.ui.widget(cx, ids!(dock)).area().rect(cx);
        // ...and where the create bar is. It FLOATS over the grid now, so the
        // pager (an overlay sibling, which also sees the event) must ignore
        // presses that land on it — otherwise typing in an expanded prompt
        // would double as tapping the icons underneath.
        self.app_state.create_rect = if self.app_state.edit_mode {
            Rect::default()
        } else {
            self.ui.widget(cx, ids!(create_bar)).area().rect(cx)
        };
        // A press outside the bar closes the options row. Inside — including on
        // the controls themselves — must not, which is why this tests the bar's
        // whole rect rather than the prompt's focus.
        if self.create_options_open {
            let outside = |abs: DVec2| {
                let r = self.app_state.create_rect;
                r.size.x <= 0.0 || !r.contains(abs)
            };
            let dismissed = match event {
                Event::MouseDown(e) => outside(e.abs),
                Event::TouchUpdate(e) => e.touches.iter().any(|t| outside(t.abs)),
                _ => false,
            };
            if dismissed {
                self.close_agent_options(cx);
            }
        }
        // A press outside also dismisses a finished run's output.
        if self.console_finished {
            let outside = |abs: DVec2| {
                let r = self.app_state.create_rect;
                r.size.x <= 0.0 || !r.contains(abs)
            };
            let dismissed = match event {
                Event::MouseDown(e) => outside(e.abs),
                Event::TouchUpdate(e) => e.touches.iter().any(|t| outside(t.abs)),
                _ => false,
            };
            if dismissed {
                self.console_finished = false;
                self.set_create_bar_idle(cx);
            }
        }
        // A wheel or a press inside the console means the user is reading back
        // through the run; stop dragging them to the bottom on every new line.
        if self.activity_active && self.console_follow {
            let over = |abs: DVec2| {
                let r = self.ui.widget(cx, ids!(create_output)).area().rect(cx);
                r.size.x > 0.0 && r.contains(abs)
            };
            let interacted = match event {
                Event::Scroll(e) => over(e.abs),
                Event::MouseDown(e) => over(e.abs),
                Event::TouchUpdate(e) => e.touches.iter().any(|t| over(t.abs)),
                _ => false,
            };
            if interacted {
                self.console_follow = false;
            }
        }
        // Size the console to its content, upward only (see sync_console_size).
        self.sync_console_size(cx);
        let mut scope = Scope::with_data(&mut self.app_state);
        self.ui.handle_event(cx, event, &mut scope);

        // Publish the slot a drag hovering the dock would land in, so the dock can
        // open a gap there. Read after the UI ran, since the pager only works it
        // out while handling the move that got there.
        let dock_drop = self.home_pager(cx).dock_hover();
        if dock_drop != self.app_state.dock_drop {
            self.app_state.dock_drop = dock_drop;
            self.ui.widget(cx, ids!(dock)).redraw(cx);
        }

        // The drawer can open/close from its own gestures (swipe up/down); keep
        // the home screen's visibility in sync after the UI has handled events.
        self.sync_overlays(cx);
        self.sync_edit_bar(cx);
    }
}

/// The pencil that marks the create bar as "editing an existing app".
pub const PENCIL: &str = "✏️";

/// The bar prefill for modifying `name`, e.g. `✏️ Weather: `.
pub fn modify_prefix(name: &str) -> String {
    format!("{PENCIL} {name}: ")
}

/// The local timezone's offset from UTC in seconds. std::time can't provide it,
/// so we ask `date +%z` on unix; elsewhere we fall back to 0 (UTC).
pub fn utc_offset_secs() -> i64 {
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
