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
        import_modal::{ImportModalAction, LauncherImportModalWidgetRefExt},
        providers_page::{
            LauncherProvidersPageWidgetRefExt, ProviderEntry, ProviderState, ProvidersAction,
            ProvidersContext,
        },
        source_modal::{
            LauncherSourceModalWidgetRefExt, SourceContext, SourceModalAction,
        },
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

                    // The source viewer sits above App Info: you open it FROM
                    // that page and go back to it, so it can't replace it.
                    source_modal := Modal{
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
                        content := LauncherSourceModal{}
                    }

                    providers_modal := Modal{
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
                        content := LauncherProvidersPage{}
                    }

                    import_modal := Modal{
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
                        content := LauncherImportModal{}
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
    /// The docked pane's rect while split-screen pick mode is choosing the
    /// second app (zero otherwise). Home-layer widgets — pager, dock, drawer,
    /// search — ignore presses inside it, so taps on the docked app can't
    /// fall through to the launcher content it covers.
    pub split_block_rect: Rect,
    /// Whether the pager must skip drawing widget tiles. True while ANY
    /// mini-app pane exists (fullscreen, split, pick, or mid-animation).
    /// Widget tiles are glass: the whole tile subtree renders in the tile's
    /// own overlay draw list, which composites ABOVE the entire main pass —
    /// so a visible tile ALWAYS floats in front of an app pane no matter the
    /// widget-tree order. Split-pick keeps the home screen live beside the
    /// docked pane, so hiding the whole home (the fullscreen-era fix) is not
    /// available there; the tiles alone get out of the way instead. Same
    /// contract as commit 6108456: widgets are background content, never in
    /// front of an open app.
    pub hide_widget_tiles: bool,
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
            split_block_rect: Rect::default(),
            hide_widget_tiles: false,
        }
    }
}

/// A generation worth offering again after it failed. Retrying is only useful
/// when the failure was the *agent's* — a cancel or a refusal isn't something
/// a second identical run fixes, and a missing provider gets the setup modal.
#[derive(Clone, Debug)]
pub(crate) struct FailedRun {
    request: String,
    /// Set when the run modified an existing app rather than creating one.
    refine_of: Option<MiniAppId>,
    /// Effort to switch to first, when the backend has a higher rung than
    /// what's currently picked. `None` means retry exactly as before.
    escalate: Option<String>,
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
    /// Whether the create bar is currently hidden (edit mode, or split-screen
    /// pick mode). Tracked separately from `edit_bar_shown` — pick mode hides
    /// the bar without bringing up the edit bar.
    #[rust]
    create_bar_hidden: bool,
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
    /// Transcript length already painted into the console, and when. The
    /// transcript is retained in full and only grows, so these throttle the
    /// repaint: a Label re-lays out ALL of its text on every change, and doing
    /// that per streamed token once a run has produced tens of KB eats the
    /// frame budget.
    #[rust]
    console_painted_len: usize,
    #[rust]
    console_painted_at: Option<std::time::Instant>,
    /// The run's text, which App owns and the console widget renders. Kept
    /// here rather than read back off widgets: the console is a virtualized
    /// list now, so most of a run isn't a widget at any given moment and
    /// there is nothing to read back from.
    #[rust]
    console_trail: String,
    #[rust]
    console_stream: String,
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
    /// The last generation that failed in a way worth trying again, kept so
    /// the bar's Retry can re-run it without the request being retyped.
    #[rust]
    failed_run: Option<FailedRun>,
    /// The app a finished run produced (created or modified), so the console's
    /// Open button knows what to open.
    #[rust]
    finished_app: Option<MiniAppId>,
    /// Whether the composer is showing its one-line label rather than the field.
    #[rust]
    prompt_collapsed: bool,
    /// Event passes still owed to "put the caret back in the prompt".
    ///
    /// A countdown rather than a flag: focus has to be *retried* (see the
    /// consumer for why a one-shot silently fails), but the retry can't be
    /// gated on the composer being expanded — right after "New prompt" it
    /// isn't, and won't be until focus lands and opens the options row. A
    /// bounded countdown gives focus plenty of passes to stick without
    /// leaving a flag armed forever if the user goes somewhere else.
    #[rust]
    prompt_focus_tries: u8,
    /// The anchor the open context menu was placed against, and the panel
    /// height used to place it. Kept so the placement can be REDONE from the
    /// menu's measured height once it has drawn: `show()` can only estimate,
    /// and an over-estimate pushed an above-the-icon menu visibly away from it.
    #[rust]
    menu_anchor: Option<Rect>,
    #[rust]
    menu_placed_h: f64,
    /// Provider whose key the Providers page is currently asking for.
    #[rust]
    provider_pending: Option<String>,
    /// Error/hint shown under that key field.
    #[rust]
    provider_status: String,
    /// HOST_LAUNCHER_DEBUG_STATE=zorder only: fires once, mid-run, to re-create
    /// a widget tile the way an install does.
    #[rust]
    zorder_repro: Timer,
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
    /// Abandon the generation that's in flight.
    ///
    /// Confirmed because it is not recoverable: the turn's work is thrown
    /// away, the agent is killed mid-write, and the tokens are already spent.
    /// Stop also sits where Open and Retry appear moments later, so a
    /// mis-timed tap on a nearly-finished run destroyed it.
    StopGeneration,
}

/// Natural (fully-revealed) height of the edit-mode management bar. The reveal
/// animation grows/shrinks the bar's height between 0 and this.
const EDIT_BAR_HEIGHT: f64 = 77.0;

/// How many favorites the dock holds. Dropping onto a full dock leaves the icon
/// on the home grid rather than silently discarding it.
pub const MAX_DOCK_ITEMS: usize = 5;

/// Height the prompt field is pinned to when the composer folds: one line of
/// text plus its own 10/10 padding. The draft underneath keeps its full
/// layout and the field clips the rest (see `set_prompt_collapsed`).
///
/// Sized so the idle face rests at exactly `create_head`'s floor (50): this
/// field plus the 6px `create_prompt` puts around it. The bar swaps between
/// the two faces in place, so a mismatch makes it change size the moment a run
/// starts or ends. Both numbers measured off the rendered tree, and it must
/// stay a WHOLE number of lines — the field clips its overflow, so a value
/// between lines slices the last one through the middle of its glyphs.
const COLLAPSED_PROMPT_H: f64 = 44.0;

/// The field's floor when expanded, straight from its DSL `Fit` bounds.
const PROMPT_MIN_H: f64 = 40.0;

/// Event passes allowed for "put the caret back in the prompt" to take effect.
/// Generous — it costs one `has_key_focus` check per pass and stops the moment
/// focus sticks — but bounded, so a user who walks away doesn't leave the app
/// grabbing at a field forever.
const PROMPT_FOCUS_TRIES: u8 = 30;

/// How much of the screen the agent console may grow to cover before it starts
/// scrolling instead. The bar floats over the grid, so this is really "how much
/// of the home screen a running generation is allowed to hide".
const CONSOLE_MAX_FRACTION: f64 = 0.62;

/// Clearance the create bar leaves above the dock when the console grows.
/// Measured from the BAR's bottom edge, not the console's — the finished-run
/// footer draws below the console (see `sync_console_size`).
const CONSOLE_DOCK_GAP: f64 = 25.0;

/// What the console opens at, before its first line has drawn — one line's
/// worth, so it grows into place rather than shrinking into it.
const CONSOLE_START_HEIGHT: f64 = 22.0;

/// How often the retained transcript is pushed into the console while a run
/// streams — fast enough to read as live, slow enough not to redo O(run) work
/// per streamed token.
///
/// It was load-bearing when the console was one `Label`: a Label re-lays out
/// ALL of its text on every change, so painting per token re-laid tens of KB
/// many times a second. The list ended that — only visible lines lay out.
///
/// What's left is the handover itself: each paint clones the whole transcript,
/// splits it into a `String` per line, and diffs that against the previous
/// lines. Three O(transcript) passes, for an update nobody can perceive at
/// more than a few a second. Cheap to raise or drop if the console ever feels
/// laggy — nothing depends on the exact value.
const CONSOLE_REPAINT: std::time::Duration = std::time::Duration::from_millis(120);


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

        // Drop placements nothing can be done with: apps that no longer
        // exist, and zero-span ghosts. Silent — neither can be drawn, tapped
        // or removed, so there's nothing to tell the user about.
        layout.prune_unusable(|id| registry.contains(id));
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
            split_block_rect: Rect::default(),
            hide_widget_tiles: false,
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
                kind: PlacedKind::App { id: id.to_string(), instance: 0, cols: 1, rows: 1 },
                col,
                row,
            });
        }
        let mut page1 = HomePage::default();
        page1.items.push(PlacedItem {
            kind: PlacedKind::App { id: "counter".into(), instance: 0, cols: 1, rows: 1 },
            col: 0,
            row: 2,
        });
        page1.items.push(PlacedItem {
            kind: PlacedKind::App { id: "stopwatch".into(), instance: 0, cols: 1, rows: 1 },
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

    /// Opens a mini-app (fullscreen, or into a split pane when one is being
    /// picked/showing — the screen routes internally), recording it as
    /// most-recently-used.
    fn open_app(&mut self, cx: &mut Cx, app_id: &MiniAppId, from_rect: Rect) {
        let Some(manifest) = self.app_state.registry.get(app_id).cloned() else {
            error!("BUG: tried to open unknown app {app_id}");
            return;
        };
        // Already running in a home tile? Expand THAT instance rather than
        // starting a second one beside it — one app, one live isolate, whose
        // presentation just changes (the same contract the ⤢ button uses).
        if !self.mini_app_screen(cx).is_showing() {
            let live = self.home_pager(cx).live_app_tiles();
            let mine = self.app_state.layout.pages.iter().flat_map(|p| &p.items).find_map(|it| {
                match &it.kind {
                    PlacedKind::App { id, instance, .. }
                        if id == app_id && live.contains(instance) =>
                    {
                        Some(*instance)
                    }
                    _ => None,
                }
            });
            if let Some(instance) = mine {
                if let Some(host) = self.home_pager(cx).lend_app_host(cx, instance) {
                    self.stamp_recents(app_id);
                    self.mini_app_screen(cx).adopt_host(cx, app_id, host, from_rect);
                    cx.redraw_all();
                    return;
                }
            }
        }
        self.stamp_recents(app_id);
        self.mini_app_screen(cx).open_app(cx, &manifest, from_rect);
    }

    /// Sets a home app placement's span, creating or tearing down its live
    /// host as it crosses the 1x1 line. A 1x1 app is a plain icon; anything
    /// bigger runs for real in the cells it claims.
    fn resize_home_app(&mut self, cx: &mut Cx, instance: WidgetInstanceId, span: (u8, u8)) {
        let mut changed = false;
        for page in &mut self.app_state.layout.pages {
            for it in &mut page.items {
                if let PlacedKind::App { instance: i, cols, rows, .. } = &mut it.kind {
                    if *i == instance && (*cols, *rows) != span {
                        *cols = span.0;
                        *rows = span.1;
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            return;
        }
        if span == (1, 1) {
            // Back to an icon: the app stops running here.
            self.return_expanded_app(cx);
            self.home_pager(cx).drop_app_host(cx, instance);
            makepad_widgets::widget_async::gc_dead_splash_isolates(cx);
        }
        self.app_state.layout_dirty = true;
        cx.redraw_all();
    }

    /// Pulls an expanded home app out of fullscreen and back into its cells.
    fn return_expanded_app(&mut self, cx: &mut Cx) {
        if self.mini_app_screen(cx).adopted().is_none() {
            return;
        }
        self.mini_app_screen(cx).release_adopted(cx);
        self.home_pager(cx).reclaim_app_host(cx);
        cx.redraw_all();
    }

    fn stamp_recents(&mut self, app_id: &MiniAppId) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.app_state.layout.recents.insert(app_id.clone(), now);
        self.app_state.layout_dirty = true;
    }

    /// Docks `app_id` to a pane and enters split-screen pick mode, where the
    /// home screen stays live to choose the second app.
    fn enter_split_pick(&mut self, cx: &mut Cx, app_id: &MiniAppId, from_rect: Option<Rect>) {
        let Some(manifest) = self.app_state.registry.get(app_id).cloned() else {
            error!("BUG: tried to split-pick unknown app {app_id}");
            return;
        };
        // Picking needs a LIVE home screen: a running generation keeps
        // `composer_expanded` true (home input off) with its console hidden
        // by pick mode — a dead end where no second app can be chosen. Refuse
        // like `arm_modify` does; a finished console is simply dismissed, and
        // edit mode exits the same way it does for a modify.
        if self.generation.is_some() {
            return;
        }
        if self.console_finished {
            self.dismiss_console(cx);
        }
        self.close_agent_options(cx);
        if self.app_state.edit_mode {
            self.app_state.edit_mode = false;
            cx.redraw_all();
        }
        self.stamp_recents(app_id);
        let screen = self.ui.window(cx, ids!(main_window)).get_inner_size(cx);
        self.mini_app_screen(cx)
            .enter_split_pick(cx, &manifest, from_rect, screen);
        cx.redraw_all();
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
        // Split needs room for two panes along the window's longer side.
        let screen = self.ui.window(cx, ids!(main_window)).get_inner_size(cx);
        let can_split =
            screen.x.max(screen.y) - 8.0 >= crate::mini_apps::mini_app_screen::MIN_SPLIT_SPAN;
        // A tall window splits top/bottom; the menu icon should show the
        // divider the split will actually have.
        let split_horizontal = screen.y >= screen.x;
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
            can_split,
            split_horizontal,
            // Offered only for an app actually running in its cells.
            home_app_span: home_instance.filter(|i| {
                self.app_state.layout.pages.iter().flat_map(|p| &p.items).any(|it| {
                    matches!(&it.kind, PlacedKind::App { instance, .. } if instance == i)
                        && it.is_live()
                })
            }),
        };
        let (glyph, name) = (manifest.icon.clone(), manifest.name.clone());
        let height = self
            .ui
            .launcher_context_menu(cx, ids!(context_menu_modal.content))
            .show(cx, &glyph, &name, context);
        // First pass uses the estimate so the menu appears in the right place
        // immediately; the measured height corrects it on the next frame.
        self.menu_anchor = Some(anchor);
        self.menu_placed_h = height + MENU_CALLOUT_H;
        self.place_popup(
            cx,
            ids!(context_menu_modal.content),
            // Reserve room for the callout triangle so the menu still clears the
            // icon once the triangle is added on the anchor-facing side.
            dvec2(MENU_WIDTH, height + MENU_CALLOUT_H),
            anchor,
        );
        self.ui.modal(cx, ids!(context_menu_modal)).open(cx);
        // The menu also shows the Android resize indicator around the item —
        // for an app icon as well as a widget, since resizing an icon past
        // 1x1 is how you set the app running on the home screen.
        self.home_pager(cx)
            .set_resize_hint(cx, widget_instance.or(home_instance));
    }

    fn close_context_menu(&mut self, cx: &mut Cx) {
        self.ui.modal(cx, ids!(context_menu_modal)).close(cx);
        self.menu_anchor = None;
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
        // Catalog first, then anything the user uninstalled that the catalog
        // can't supply — their own generated and imported apps. Without the
        // second half, uninstalling one destroyed the only copy and the store
        // had nothing to offer back.
        let catalog = crate::mini_apps::builtin::store_catalog();
        let archived = self
            .app_state
            .layout
            .archived_user_apps
            .iter()
            .filter(|a| !catalog.iter().any(|m| m.id == a.id))
            .cloned()
            .collect::<Vec<_>>();
        let entries: Vec<StoreEntry> = catalog
            .into_iter()
            .chain(archived)
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
        // The catalog, or the user's own archive — an app they generated or
        // imported and later uninstalled is only in the second.
        let Some(manifest) = crate::mini_apps::builtin::store_catalog()
            .into_iter()
            .find(|m| &m.id == app_id)
            .or_else(|| {
                self.app_state
                    .layout
                    .archived_user_apps
                    .iter()
                    .find(|a| &a.id == app_id)
                    .cloned()
            })
        else {
            return;
        };
        self.app_state.layout.archived_user_apps.retain(|a| &a.id != app_id);
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
    // Export / import
    // -----------------------------------------------------------------------

    /// Opens Import App with whatever the exchange folder currently holds.
    /// Re-listed on every open: the folder is the user's, and files appear in
    /// it while the launcher runs.
    fn open_import_modal(&mut self, cx: &mut Cx) {
        let dir = crate::mini_apps::bundle::exchange_dir();
        let entries = crate::mini_apps::bundle::list_importable();
        self.ui
            .launcher_import_modal(cx, ids!(import_modal.content))
            .show(cx, &entries, &dir.display().to_string());
        self.ui.modal(cx, ids!(import_modal)).open(cx);
    }

    /// Writes a shareable bundle for `app_id` and copies it to the clipboard,
    /// then reports the file beside the button. Both at once on purpose: the
    /// file is for handing over a folder, the clipboard is for pasting into a
    /// chat, and there's no way to know which one you meant.
    fn export_app(&mut self, cx: &mut Cx, app_id: &MiniAppId) {
        let Some(manifest) = self.app_state.registry.get(app_id).cloned() else {
            return;
        };
        let page = self.ui.launcher_app_info(cx, ids!(app_info_modal.content));
        match crate::mini_apps::bundle::write_export(&manifest) {
            Ok(path) => {
                cx.copy_to_clipboard(&crate::mini_apps::bundle::to_text(&manifest));
                let file = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                page.set_export_hint(cx, &format!("copied · {file}"));
            }
            Err(e) => {
                error!("couldn't export '{app_id}': {e}");
                page.set_export_hint(cx, "export failed");
            }
        }
    }

    /// Installs an app from bundle text (a file's contents, or a paste).
    ///
    /// Imported source is a stranger's code, so it goes through the same
    /// compile check a generated app does before it's allowed onto the home
    /// screen — the sandbox contains it at runtime, but a script that doesn't
    /// even parse should fail here, with a reason, rather than as a broken
    /// tile later.
    fn import_app(&mut self, cx: &mut Cx, text: &str) {
        let modal = self.ui.launcher_import_modal(cx, ids!(import_modal.content));
        let mut manifest = match crate::mini_apps::bundle::parse(text) {
            Ok(m) => m,
            Err(e) => return modal.set_status(cx, &e),
        };
        let errors = crate::generate::pipeline::validate_splash(cx, &manifest.source);
        if let Some(first) = errors.first() {
            return modal.set_status(cx, &format!("that app doesn't compile: {first}"));
        }
        let taken = self.taken_app_ids();
        if taken.iter().any(|t| t == &manifest.id) {
            manifest.id = crate::generate::pipeline::unique_id(&manifest.name, &taken);
        }
        let id = manifest.id.clone();
        let name = manifest.name.clone();
        if let Err(e) = persistence::save_user_app(&manifest) {
            return modal.set_status(cx, &format!("couldn't save it: {e}"));
        }
        // An import resurrects an id the user had uninstalled only if it
        // reuses that id — and `unique_id` already moved it aside if so. Still
        // lift the tombstone for the id we actually took, or a later store
        // sync could hide the freshly imported app.
        self.app_state
            .layout
            .uninstalled_user_apps
            .retain(|t| t != &id);
        self.app_state.layout.user_apps.push(manifest.clone());
        self.app_state.registry.insert(manifest);
        self.add_app_to_home(&id);
        self.app_state.layout_dirty = true;
        self.ui.modal(cx, ids!(import_modal)).close(cx);
        self.flash_create_bar(cx, &format!("{name} imported ✓"));
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
        if request.is_empty() || self.generation.is_some() || self.composer_suppressed(cx) {
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
                self.generation_watchdog = cx.start_interval(1.0);
            }
            Err(reason) => self.report_start_failure(cx, &reason),
        }
    }

    /// A generation that never got off the ground. The bar has room for the
    /// headline only, so anything the user has to ACT on — install octos, add
    /// a key — also opens the page that explains it and can be acted on. The
    /// in-run failure path does the same for the same reason; a message that
    /// names a fix with nowhere to perform it is where people get stuck.
    fn report_start_failure(&mut self, cx: &mut Cx, reason: &str) {
        self.flash_create_bar(cx, reason);
        if crate::generate::blocker().is_some() {
            self.open_providers(cx);
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
        // A split pick also hides the bar (and its half-docked app would sit
        // over the composer); modifying an app means leaving split setup.
        if self.mini_app_screen(cx).is_showing() {
            self.mini_app_screen(cx).close_to_home(cx);
        }
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
        if request.is_empty() || self.generation.is_some() || self.composer_suppressed(cx) {
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
                self.generation_watchdog = cx.start_interval(1.0);
            }
            Err(reason) => self.report_start_failure(cx, &reason),
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

    /// Shows a mini-app's Splash source over the App Info page. Read from the
    /// registry rather than disk: a built-in has no file, and a modified app's
    /// current source is what's loaded, not what's archived.
    fn open_source_view(&mut self, cx: &mut Cx, app_id: &MiniAppId) {
        let Some(manifest) = self.app_state.registry.get(app_id).cloned() else {
            return;
        };
        let lines = manifest.source.lines().count();
        let context = SourceContext {
            name: manifest.name.clone(),
            subtitle: format!(
                "{} · {} lines · {}",
                manifest.id,
                lines,
                if manifest.builtin { "built-in" } else { "generated" }
            ),
            source: manifest.source.clone(),
        };
        self.ui
            .launcher_source_modal(cx, ids!(source_modal.content))
            .show(cx, context);
        self.ui.modal(cx, ids!(source_modal)).open(cx);
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
        // ...and cached home tiles (widgets AND apps running on the grid) keep
        // running the OLD source — drop them so they rebuild from the new
        // script (and rebind the sandbox).
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
        self.finished_app = Some(id);
        self.sync_console_buttons(cx);
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
                // With the run clock in it this changes every second, which is
                // the point: a status line that never moves reads as hung.
                let status = generation.status_line();
                // The console's detail: the WHOLE trail, then EVERYTHING the
                // agent has emitted. No windowing anywhere — dropping older
                // lines both loses the history and makes the box shrink under
                // you, and the console is the only record of what happened.
                let log = generation.activity().join("\n");
                let stream_len = generation.transcript().len();
                let label = self.ui.label(cx, ids!(create_status));
                if label.text() != status {
                    label.set_text(cx, &status);
                }
                if self.activity_active && !self.activity_collapsed {
                    let mut grew = self.console_trail != log;
                    if grew {
                        self.console_trail = log;
                    }
                    // Still throttled, though the list made it far cheaper:
                    // only the visible lines are laid out now, so the cost is
                    // the window rather than the run. Splitting the transcript
                    // into lines is not free either, and doing that per
                    // streamed token buys nothing a human can see. Compared by
                    // LENGTH because it only ever grows.
                    let due = self
                        .console_painted_at
                        .is_none_or(|t| t.elapsed() >= CONSOLE_REPAINT);
                    if stream_len != self.console_painted_len && due {
                        self.console_stream = generation.transcript().to_string();
                        self.console_painted_len = stream_len;
                        self.console_painted_at = Some(std::time::Instant::now());
                        grew = true;
                    }
                    // Following the tail is the LIST's business now: it tails
                    // when it was already at the bottom and leaves you alone
                    // when it wasn't — and picks it back up the moment you
                    // scroll down to the end again, which the old latch never
                    // did (see `LauncherAgentConsole::set_lines`).
                    if grew {
                        self.sync_console(cx);
                    }
                }
            }
            GenOutcome::Ready { manifest, refine_of } => {
                self.paint_transcript(cx);
                self.generation = None;
                cx.stop_timer(self.generation_watchdog);
                if refine_of.is_some() {
                    self.install_refined(cx, *manifest, request);
                } else {
                    self.install_generated(cx, *manifest);
                }
            }
            GenOutcome::Failed(reason) => {
                self.paint_transcript(cx);
                let refine_of = self.generation.as_ref().and_then(|g| g.refine_target().cloned());
                self.generation = None;
                cx.stop_timer(self.generation_watchdog);
                // Setup-class failures (no provider, agent binary missing) get
                // the guided fix instead of just an error flash.
                let setup_class = reason.contains("No LLM provider")
                    || reason.contains("isn't runnable")
                    || reason.contains("isn't installed")
                    // Whatever wording a Blocker uses, it is by definition a
                    // setup problem — matching on prose would rot the moment
                    // the copy changes.
                    || crate::generate::blocker().is_some();
                let retryable = !setup_class
                    && reason != "Cancelled"
                    && !reason.starts_with("The agent declined");
                self.flash_create_bar(cx, &reason);
                if retryable {
                    let prefs = &self.agent_prefs;
                    let escalate = Backend::detect()
                        .top_effort()
                        .filter(|top| prefs.effort.as_deref() != Some(top.as_str()));
                    self.failed_run = Some(FailedRun { request, refine_of, escalate });
                    self.sync_console_buttons(cx);
                }
                if setup_class {
                    self.open_providers(cx);
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
        self.finished_app = Some(id);
        self.sync_console_buttons(cx);
        cx.redraw_all();
    }

    /// Opens the AI Providers page — the one place to add a key, switch
    /// provider, replace a key or forget one.
    fn open_providers(&mut self, cx: &mut Cx) {
        self.provider_pending = None;
        self.provider_status.clear();
        self.ui
            .launcher_providers_page(cx, ids!(providers_modal.content))
            .reset_key_field(cx);
        self.sync_providers(cx);
        self.ui.modal(cx, ids!(providers_modal)).open(cx);
    }

    /// Re-reads what's configured and repaints the page. Called after every
    /// change, because every change is a write to octos's config that the
    /// list is a view of.
    fn sync_providers(&mut self, cx: &mut Cx) {
        use crate::generate::providers;
        let configured = providers::list();
        let mut entries: Vec<ProviderEntry> = configured
            .iter()
            .map(|p| ProviderEntry {
                id: p.id.clone(),
                label: p.label.clone(),
                detail: p.detail(),
                state: if p.external() {
                    ProviderState::External
                } else if p.active {
                    ProviderState::Active
                } else {
                    ProviderState::Configured
                },
                forgettable: p.editable(),
                is_default: p.is_default,
                // Only a real, credentialled octos provider can BE the
                // default. An agent command is set by the environment, so a
                // star there would promise a change this page can't make.
                can_default: !p.external(),
            })
            .collect();
        // Then everything else we know how to set up, so adding one is a tap
        // on the provider you want rather than a guess about key formats.
        for spec in providers::CATALOG {
            if entries.iter().any(|e| e.id == spec.id) {
                continue;
            }
            entries.push(ProviderEntry {
                id: spec.id.to_string(),
                label: spec.label.to_string(),
                detail: format!("Not set up · key looks like {}", spec.hint),
                state: ProviderState::Available,
                forgettable: false,
                is_default: false,
                // Nothing to default to until it has a key.
                can_default: false,
            });
        }
        let pending = self.provider_pending.clone().map(|id| {
            let hint = providers::spec(&id).map(|s| s.hint).unwrap_or("");
            let label = providers::label_for(&id);
            (id, format!("Key for {label}  ({hint})"))
        });
        let config_path = crate::generate::providers::config_display_path();
        self.ui
            .launcher_providers_page(cx, ids!(providers_modal.content))
            .show(cx, ProvidersContext {
                entries,
                pending,
                status: self.provider_status.clone(),
                config_path,
                runtime: crate::generate::runtime(&self.agent_prefs).summary(),
                note: providers::agent_command()
                    .map(|_| {
                        "Started with HOST_LAUNCHER_AGENT_CMD, so that agent is in charge — \
                         keys below are saved but stay dormant until you launch without it."
                            .to_string()
                    })
                    .unwrap_or_default(),
            });
    }

    /// Applies a Providers page action and re-renders.
    fn handle_providers_action(&mut self, cx: &mut Cx, action: ProvidersAction) {
        use crate::generate::providers;
        match action {
            ProvidersAction::Close => {
                self.ui.modal(cx, ids!(providers_modal)).close(cx);
                return;
            }
            ProvidersAction::EnterKey(id) => {
                self.provider_pending = Some(id);
                self.provider_status.clear();
                self.ui
                    .launcher_providers_page(cx, ids!(providers_modal.content))
                    .reset_key_field(cx);
                self.sync_providers(cx);
                self.ui
                    .launcher_providers_page(cx, ids!(providers_modal.content))
                    .focus_key_field(cx);
                return;
            }
            ProvidersAction::CancelEntry => {
                self.provider_pending = None;
                self.provider_status.clear();
            }
            // "Use" switches for THIS SESSION and writes nothing: trying a
            // provider must not quietly rewrite what you start with tomorrow.
            // Making it permanent is the star, right next to it.
            ProvidersAction::Use(id) => {
                providers::set_session(&id);
                self.provider_status.clear();
                self.flash_create_bar(
                    cx,
                    &format!("Using {} for now", providers::label_for(&id)),
                );
            }
            ProvidersAction::MakeDefault(id) => match providers::set_active(&id) {
                Ok(()) => {
                    // Making something the default also drops any session pick
                    // — otherwise the page would show a star on one row and
                    // "In use" on another with no way to reconcile them.
                    providers::clear_session();
                    self.provider_status.clear();
                    self.flash_create_bar(
                        cx,
                        &format!("{} is now the default", providers::label_for(&id)),
                    );
                }
                Err(e) => self.provider_status = e,
            },
            ProvidersAction::Forget(id) => match providers::forget(&id) {
                Ok(()) => self.provider_status.clear(),
                Err(e) => self.provider_status = e,
            },
            ProvidersAction::Save(key) => {
                let Some(id) = self.provider_pending.clone() else {
                    return;
                };
                match providers::save_key(&id, &key) {
                    Ok(()) => {
                        self.provider_pending = None;
                        self.provider_status.clear();
                        self.ui
                            .launcher_providers_page(cx, ids!(providers_modal.content))
                            .reset_key_field(cx);
                        self.flash_create_bar(
                            cx,
                            &format!("{} ready — try creating an app", providers::label_for(&id)),
                        );
                    }
                    Err(e) => self.provider_status = e,
                }
            }
            ProvidersAction::None => return,
        }
        // The picks belong to the backend that was active; a switch changes
        // which knobs even exist.
        self.sync_agent_options(cx, self.create_options_open);
        self.sync_providers(cx);
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
            self.set_prompt_collapsed(cx, false);
            self.sync_agent_options(cx, true);
        }
    }

    /// Collapses the composer to one line, or lets it grow again.
    ///
    /// Clamps the height of the CLIPPING WRAPPER, not the field: the draft and
    /// its hard line breaks stay exactly as typed, and only what's visible
    /// changes. (Going single-line instead doesn't collapse anything —
    /// `is_multiline` only disables soft wrapping, so real newlines still
    /// break.)
    fn set_prompt_collapsed(&mut self, cx: &mut Cx, collapsed: bool) {
        // ONE widget, always the real field. Nothing is hidden or swapped, so
        // the draft, the caret, the selection and key focus are never disturbed
        // — which is what every previous approach here got wrong.
        //
        // Collapsing pins the field's HEIGHT to one line. It does NOT touch
        // `max_lines`, which is what this used to do: clamping the row count
        // re-lays the text out, and the laid-out text is what turns a click
        // into a caret position — but it is only rebuilt at draw time, so the
        // press that re-focused the composer was resolved against the folded
        // one-line layout while the expanded one was on screen. Caret and
        // drag-selection both landed on the wrong text, and no amount of
        // ordering fixed it: within one event pass there is no `Cx2d` to
        // re-lay out with. A height clamp leaves the layout alone entirely and
        // lets the field clip its own overflow.
        //
        // Walk written directly rather than through `script_apply_eval!`: this
        // runs per event, and an eval body has none of the DSL's `use`s in
        // scope, so a bare `Fit` there resolves to nothing (SPLASH_FINDINGS #8).
        let height = if collapsed {
            Size::Fixed(COLLAPSED_PROMPT_H)
        } else {
            // The DSL's own bounds, restored: grow with the draft, cap at 75%
            // of the screen and scroll internally past that.
            Size::Fit {
                min: Some(FitBound::Abs(PROMPT_MIN_H)),
                max: Some(FitBound::Rel { base: Base::Full, factor: 0.75 }),
            }
        };
        self.ui.text_input(cx, ids!(create_input)).set_height(cx, height);
        if collapsed {
            // The scroll offset outlives the blur, so a draft last edited near
            // its end would fold showing whichever line the caret had reached
            // instead of its first.
            self.ui.text_input(cx, ids!(create_input)).scroll_to_top(cx);
        }
        self.prompt_collapsed = collapsed;
        self.ui.widget(cx, ids!(create_bar)).redraw(cx);
    }

    /// Closes it — the composer is finished with.
    fn close_agent_options(&mut self, cx: &mut Cx) {
        if self.create_options_open {
            self.create_options_open = false;
            self.sync_agent_options(cx, false);
        }
        // Everything folds away together: a one-line pill is the bar's resting
        // state, and leaving it tall reads as a panel that half-closed.
        self.set_prompt_collapsed(cx, true);
    }

    fn sync_agent_options(&mut self, cx: &mut Cx, reveal: bool) {
        let backend = Backend::detect();
        let knobs = backend.knobs();
        // Shown whenever the composer is focused, even for a backend with no
        // knobs at all: the row also carries the backend name and the way in
        // to the Providers page, which must never become unreachable.
        let show = reveal;
        self.ui.widget(cx, ids!(create_options)).set_visible(cx, show);
        if show {
            self.ui
                .label(cx, ids!(create_backend))
                .set_text(cx, &format!("Using {}", backend.display_name()));
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
                // Pick by CONTENT, not by count: three ladders share this row
                // and two of them are the same length. `xhigh` marks the
                // probed Claude ladder; a missing `medium` marks Kimi's, whose
                // rungs really are low/high/max.
                let has = |v: &str| knob.options.iter().any(|(_, o)| o == v);
                let extended = has(prefs::CLAUDE_EFFORT_XHIGH.1);
                let kimi = !extended && !has("medium");
                self.ui
                    .widget(cx, ids!(opt_1.ao_seg_1))
                    .set_visible(cx, !extended && !kimi);
                self.ui
                    .widget(cx, ids!(opt_1.ao_seg_1x))
                    .set_visible(cx, extended);
                self.ui
                    .widget(cx, ids!(opt_1.ao_seg_1k))
                    .set_visible(cx, kimi);
                if extended {
                    ids!(opt_1.ao_seg_1x)
                } else if kimi {
                    ids!(opt_1.ao_seg_1k)
                } else {
                    ids!(opt_1.ao_seg_1)
                }
            } else {
                Self::SEGMENT_IDS[slot]
            };
            // set_selected, NOT `seg.selected = index`: the field is public but
            // the pill is drawn from a private `sel_pos` that only follows it
            // via the click animation. Writing the field left the control
            // showing Default while reporting High — and a click on High was
            // then ignored as "already selected", which is what made most
            // clicks look like they did nothing.
            // set_selected, NOT `seg.selected = index`: the field is public but
            // the pill is drawn from a private `sel_pos` that only follows it
            // through the click animation. Writing the field left every control
            // showing "Default" while reporting the saved pick — and a click on
            // the segment it really held was then ignored as "already
            // selected", which is what made most clicks do nothing.
            self.ui.glass_segmented(cx, seg_id).set_selected(cx, index);
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

    /// The buttons a finished run offers: Retry (it failed and re-running
    /// might help), Open (it succeeded — go straight into the app), and New prompt
    /// (put the composer back). Exactly one of Retry/Open can apply, and both
    /// sit in Stop's slot, so they're decided together.
    fn sync_console_buttons(&mut self, cx: &mut Cx) {
        let retry = self.ui.glass_button(cx, ids!(create_retry));
        match &self.failed_run {
            Some(_) => {
                // Always just "Retry". A retry still escalates the effort rung
                // when there's one left (see `retry_failed_run`) — that's a
                // detail of what the button does, not a different button.
                retry.set_text(cx, "Retry");
                retry.set_visible(cx, true);
            }
            None => retry.set_visible(cx, false),
        }
        // Open needs the app to still exist: a run can finish and the user can
        // uninstall it from the drawer before pressing anything here.
        let openable = self
            .finished_app
            .as_ref()
            .is_some_and(|id| self.app_state.registry.contains(id));
        self.ui
            .glass_button(cx, ids!(create_open))
            .set_visible(cx, openable);
        // The footer only makes sense once the run is over — mid-run, Stop is
        // the way out.
        let finished = self.console_finished
            && !self.activity_collapsed
            && !self.composer_suppressed(cx);
        self.ui.widget(cx, ids!(create_footer)).set_visible(cx, finished);
    }

    /// Puts the composer back and forgets the finished run (the console's
    /// "New prompt" button, and Open on the way into a generated app).
    /// Deliberately does NOT touch the prompt's text. Dismissal happens on a
    /// press anywhere outside the bar, which is an ordinary way to look away
    /// from it — losing a typed draft to that is indefensible. Clearing is
    /// what "New prompt" is for, and only that (see the `create_done` handler).
    fn dismiss_console(&mut self, cx: &mut Cx) {
        self.console_finished = false;
        self.failed_run = None;
        self.finished_app = None;
        self.set_create_bar_idle(cx);
    }

    /// Console's Open: dismiss the panel, then zoom into the app the run just
    /// produced. The zoom starts from the bar itself — the app grows out of the
    /// panel that made it, which reads better than flying in from a corner.
    fn open_finished_app(&mut self, cx: &mut Cx) {
        let Some(app_id) = self.finished_app.clone() else {
            return;
        };
        let from_rect = self.app_state.create_rect;
        self.dismiss_console(cx);
        if self.app_state.registry.contains(&app_id) {
            self.open_app(cx, &app_id, from_rect);
        }
    }

    /// Runs the failed request again, first nudging effort to the top rung if
    /// there was one left. Consumes the failed run either way: the retry
    /// records its own failure if it fails too, and a stale click after the
    /// bar moved on must not start a surprise generation.
    fn retry_failed_run(&mut self, cx: &mut Cx) {
        let Some(run) = self.failed_run.take() else {
            return;
        };
        self.sync_console_buttons(cx);
        if let Some(effort) = run.escalate {
            self.agent_prefs.set(KnobId::Effort, Some(effort));
            let _ = persistence::save_agent_prefs(&self.agent_prefs);
            // Keep the visible control in step — the pick persists past this
            // run, so the row must not still read what it said before.
            self.sync_agent_options(cx, self.create_options_open);
        }
        match run.refine_of {
            Some(app_id) => {
                self.start_modify(cx, &app_id, run.request);
            }
            None => self.start_generation(cx, run.request),
        }
    }

    /// Matches the console's output area to the busy + collapse state, and
    /// points the chevron the right way. Hidden entirely in edit mode.
    fn sync_activity_panel(&mut self, cx: &mut Cx) {
        // The console is the bar's busy state; hiding it drops the bar back to
        // the one-line status. The chevron lives in the ✨'s slot while the
        // agent works, so exactly one of the two is ever on screen.
        let active =
            (self.activity_active || self.console_finished) && !self.composer_suppressed(cx);
        self.ui
            .widget(cx, ids!(create_output))
            .set_visible(cx, active && !self.activity_collapsed);
        self.ui.widget(cx, ids!(create_glyph)).set_visible(cx, !active);
        // Both, so the button's own flag matches what's on screen — a widget
        // hidden only by its parent still reports itself visible.
        self.ui.widget(cx, ids!(create_toggle_wrap)).set_visible(cx, active);
        self.ui.widget(cx, ids!(create_toggle)).set_visible(cx, active);
        // The spinner tracks the RUN, not the console: it goes the moment the
        // run ends, even though the output stays up to be read.
        //
        // `console_finished` has to be checked too — `flash_create_bar` never
        // clears `activity_active` (only `set_create_bar_idle` does, on
        // dismissal), so a finished console kept spinning as though the agent
        // were still working.
        let spinning =
            self.activity_active && !self.console_finished && !self.composer_suppressed(cx);
        self.ui
            .widget(cx, ids!(create_spinner))
            .set_visible(cx, spinning);
        if let Some(mut arrow) = self
            .ui
            .widget(cx, ids!(create_arrow))
            .borrow_mut::<ExpandArrow>()
        {
            arrow.set_is_open(cx, !self.activity_collapsed, Animate::Yes);
        }
        // The finished-run footer follows the console it belongs to (collapsing
        // the output must take the footer with it).
        self.sync_console_buttons(cx);
        self.ui.widget(cx, ids!(create_bar)).redraw(cx);
    }


    /// How many lines the run has produced. Replaces measuring the content:
    /// a virtualized list has no measurable height — only the visible window
    /// exists — so "does this need more room?" is a question about the run,
    /// not about a laid-out box.
    fn console_line_count(&mut self, cx: &mut Cx) -> usize {
        let console = self.ui.widget(cx, ids!(create_output));
        console
            .borrow::<crate::launcher::agent_console::LauncherAgentConsole>()
            .map(|c| c.line_count())
            .unwrap_or(0)
    }

    /// Jumps the console to its newest line, and re-arms tailing with it.
    fn scroll_console_to_end(&mut self, cx: &mut Cx) {
        let console = self.ui.widget(cx, ids!(create_output));
        if let Some(mut console) =
            console.borrow_mut::<crate::launcher::agent_console::LauncherAgentConsole>()
        {
            console.scroll_to_end(cx);
        }
    }

    /// Writes the console's height. Straight onto the walk rather than through
    /// `script_apply_eval!` — this runs per event, and an eval body has none of
    /// the DSL's `use`s in scope, so bare `Fit`/`FitBound` there resolve to
    /// nothing and the height silently fails to apply (SPLASH_FINDINGS #8).
    fn set_console_height(&mut self, cx: &mut Cx, height: Option<f64>) {
        let console = self.ui.widget(cx, ids!(create_output));
        if let Some(mut console) =
            console.borrow_mut::<crate::launcher::agent_console::LauncherAgentConsole>()
        {
            console.set_height(cx, height.unwrap_or(CONSOLE_START_HEIGHT));
        }
        self.ui.widget(cx, ids!(create_bar)).redraw(cx);
    }

    /// Pushes the run's text into the console list.
    ///
    /// Splitting into lines here is what makes virtualization possible: the
    /// list materializes only the lines on screen, so a 5,000-line run costs
    /// the same to scroll as a 20-line one. As one `Label` it cost a full
    /// re-layout of every byte on every change.
    fn sync_console(&mut self, cx: &mut Cx) {
        use crate::launcher::agent_console::{ConsoleLine, ConsoleLineKind, LauncherAgentConsole};
        let mut lines: Vec<ConsoleLine> = Vec::new();
        for text in self.console_trail.lines() {
            lines.push(ConsoleLine { kind: ConsoleLineKind::Trail, text: text.to_string() });
        }
        for text in self.console_stream.lines() {
            lines.push(ConsoleLine { kind: ConsoleLineKind::Stream, text: text.to_string() });
        }
        let console = self.ui.widget(cx, ids!(create_output));
        if let Some(mut console) = console.borrow_mut::<LauncherAgentConsole>() {
            console.set_lines(cx, lines);
        }
    }

    /// Flushes the run's full transcript into the console, ignoring the
    /// repaint throttle.
    ///
    /// Called when a run ENDS. The throttle means the last streamed chunk is
    /// usually still unpainted at that moment, and the generation is about to
    /// be dropped — so without this the console would be permanently missing
    /// its final stretch, which is exactly the part that says how the run
    /// turned out.
    fn paint_transcript(&mut self, cx: &mut Cx) {
        let Some(generation) = self.generation.as_ref() else {
            return;
        };
        let transcript = generation.transcript().to_string();
        if transcript.is_empty() {
            return;
        }
        self.console_painted_len = transcript.len();
        self.console_painted_at = Some(std::time::Instant::now());
        self.console_stream = transcript;
        self.sync_console(cx);
    }

    /// Sizes the console from its CONTENT, ratcheting upward only. A scrolling
    /// view can't be `Fit` — it takes whatever height it's offered — so the
    /// height is driven from here: the moment the content outgrows the box,
    /// go to the FULL cap, and never come back down (a box that shrinks under
    /// the text you're reading is worse than one that's briefly too big).
    fn sync_console_size(&mut self, cx: &mut Cx) {
        // `console_finished` counts: a run that fails early (no provider, agent
        // not runnable) writes its whole story at once and then stops being
        // "active", which left the box at its opening one-line height with the
        // message scrolling inside it — unreadable, and exactly when you most
        // need to read it.
        if (!self.activity_active && !self.console_finished) || self.activity_collapsed {
            return;
        }
        // More than the opening line means it needs the room.
        let content_outgrew = self.console_line_count(cx) > 1;
        // Grow until the console is a hair above the dock — the real limit is
        // "don't cover the dock", not an abstract fraction of the screen.
        let out = self.ui.widget(cx, ids!(create_output)).area().rect(cx);
        let out_top = out.pos.y;
        let dock_top = self.app_state.dock_rect.pos.y;
        // Everything the BAR draws below the console: the finished-run footer
        // ("New prompt"), its spacing, and the bar's own bottom padding.
        //
        // Measured, not assumed, and subtracted from the cap — otherwise the
        // gap is only kept below the OUTPUT, and the footer hangs past it into
        // the dock. Measured at 54px with the footer showing, which put the
        // bar's bottom 24px BELOW the top of the dock, overlapping it. The
        // footer comes and goes with the run, so this is re-read each time
        // rather than baked in as a constant.
        let bar = self.app_state.create_rect;
        let below_console = if bar.size.y > 0.0 && out.size.y > 0.0 {
            ((bar.pos.y + bar.size.y) - (out_top + out.size.y)).max(0.0)
        } else {
            0.0
        };
        let cap = if dock_top > out_top {
            dock_top - out_top - CONSOLE_DOCK_GAP - below_console
        } else {
            self.ui.widget(cx, ids!(create_layer)).area().rect(cx).size.y * CONSOLE_MAX_FRACTION
        };
        let cap = cap.max(CONSOLE_START_HEIGHT);
        // All the way to the cap the moment the content doesn't fit, rather
        // than creeping up a line at a time behind it. A console that grows in
        // step with its output spends the whole run one line too short, with
        // the text you want scrolling past the bottom edge — and it reflows
        // under you on every chunk. One jump to full height, once.
        //
        // The cap is a hard limit, not a preference, so it clamps the floor
        // back DOWN as well. It has to: the dock's position isn't known until
        // the dock has drawn, so a console filled before that (a run started
        // at launch) sizes itself against the fallback fraction — measured 620
        // against a real cap of 588, which put the box 2px INTO the dock and
        // left it there, since the floor never came down.
        let want = if content_outgrew { cap } else { self.console_floor.min(cap) };
        if (want - self.console_floor).abs() < 0.5 {
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
        self.failed_run = None;
        self.finished_app = None;
        self.sync_console_buttons(cx);
        // NOT `status` — the header already says that, and echoing it makes
        // the console open on the same sentence twice. This matches the trail's
        // real first entry, which the first pipeline tick overwrites anyway.
        self.console_trail = "Starting agent…".to_string();
        self.console_stream = String::new();
        self.sync_console(cx);
        self.activity_active = true;
        // A new run starts from the composer's height, follows its own tail,
        // and OPEN — a console left folded by the previous run (the chevron,
        // or a press outside) must not swallow this one's output.
        self.activity_collapsed = false;
        self.console_floor = 0.0;
        self.console_painted_len = 0;
        self.console_painted_at = None;
        self.set_console_height(cx, None);
        self.ui.view(cx, ids!(create_output)).set_scroll_pos(cx, dvec2(0.0, 0.0));
        self.sync_activity_panel(cx);
        self.ui.widget(cx, ids!(create_bar)).redraw(cx);
    }

    /// Idle state: just the input.
    fn set_create_bar_idle(&mut self, cx: &mut Cx) {
        self.ui.widget(cx, ids!(create_idle)).set_visible(cx, true);
        self.failed_run = None;
        self.finished_app = None;
        self.sync_console_buttons(cx);
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
    /// A long message cut down for the one-line status strip, on a word
    /// boundary with an ellipsis so it reads as shortened rather than as a
    /// message that stops mid-word. The full text always survives in the log.
    fn headline_of(msg: &str) -> String {
        const MAX: usize = 110;
        let one_line = msg.replace('\n', " ");
        if one_line.chars().count() <= MAX {
            return one_line;
        }
        let cut: String = one_line.chars().take(MAX).collect();
        // Back up to the last space, unless that would gut the line.
        let trimmed = match cut.rfind(' ') {
            Some(at) if at > MAX / 2 => &cut[.. at],
            _ => cut.as_str(),
        };
        format!("{}…", trimmed.trim_end_matches([' ', ',', '.', ';', ':']))
    }

    fn flash_create_bar(&mut self, cx: &mut Cx, msg: &str) {
        if self.app_state.edit_mode {
            self.pending_create_flash = Some(msg.to_string());
            return;
        }
        self.ui.widget(cx, ids!(create_idle)).set_visible(cx, false);
        self.ui.widget(cx, ids!(create_busy)).set_visible(cx, true);
        // The strip is one line; the log below it is not. Shorten only what
        // has to be short — truncating the message before it reaches the log
        // is how the actionable half of an API error got thrown away.
        self.ui.label(cx, ids!(create_status)).set_text(cx, &Self::headline_of(msg));
        self.ui.widget(cx, ids!(create_cancel)).set_visible(cx, false);
        // Whatever this flash reports, it supersedes the previous run's
        // offers; the failure path re-arms Retry right after calling us.
        self.failed_run = None;
        self.finished_app = None;
        // The run is over, but its output stays: what the agent did is worth
        // reading after the fact, and a panel that erases itself three seconds
        // after finishing takes the explanation with it. The log keeps its
        // final line; dismissal is the user's call (a press outside the bar).
        let done = format!("{}\n— {msg}", self.console_trail);
        self.console_trail = done.trim_start_matches('\n').to_string();
        self.sync_console(cx);
        self.console_finished = true;
        self.sync_console_buttons(cx);
        self.sync_activity_panel(cx);
        // Grow to fit what was just written, before anyone tries to read it.
        self.sync_console_size(cx);
        self.scroll_console_to_end(cx);
        self.ui.widget(cx, ids!(create_bar)).redraw(cx);
    }

    /// Shows/hides the edit-mode management bar to match edit mode, and keeps
    /// its grid-size labels current.
    fn sync_edit_bar(&mut self, cx: &mut Cx) {
        let editing = self.app_state.edit_mode;
        // The create bar yields to the edit bar AND to split-screen pick mode
        // (which needs the home half uncluttered); its reserved slot in the
        // column goes with it, so the grid reclaims the space.
        let hide_bar = self.composer_suppressed(cx);
        if hide_bar != self.create_bar_hidden {
            self.create_bar_hidden = hide_bar;
            self.ui.widget(cx, ids!(create_bar)).set_visible(cx, !hide_bar);
            self.ui.widget(cx, ids!(create_slot)).set_visible(cx, !hide_bar);
            if hide_bar {
                self.close_agent_options(cx);
            }
            // ...and takes the agent console with it.
            self.sync_activity_panel(cx);
            // Replay a result flash that landed while the bar was hidden.
            if !hide_bar {
                if let Some(msg) = self.pending_create_flash.take() {
                    self.flash_create_bar(cx, &msg);
                }
            }
        }
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
        // `is_showing`, not `is_fully_open`: during the open/close zoom the home
        // used to stay visible so the app animated over real content, but a
        // glass widget's refraction overlay renders in FRONT of the covering
        // layer, so widgets appeared on top of the app for the length of the
        // animation. Widgets are background content, like app icons — the zoom
        // now plays over the wallpaper instead.
        // `covers_home`, not `is_showing`: split-screen pick mode docks one
        // app to a pane and needs the home screen live beside it to choose
        // the second app from.
        // ...and not while ZOOMING between home and an app: with the widget
        // tiles independently suppressed whenever a pane exists (they were
        // the reason zooms played over bare wallpaper — their glass floats
        // above everything), the home screen can sit behind an open/close
        // zoom again, which is what makes the animation read as the app
        // shrinking back INTO the launcher. In-place split animations are
        // not zooms; home stays hidden behind a settled-looking split.
        let mini = self.mini_app_screen(cx);
        let covered = self.search_overlay(cx).is_open()
            || (mini.covers_home() && !mini.is_zooming())
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
        // An expanded composer owns the next press wherever it lands: it should
        // fold away, not fold away AND open whatever icon happened to be
        // underneath. Computed after this event is dispatched, so it gates the
        // NEXT one — which is exactly the press that dismisses.
        !self.composer_expanded()
            && !self.drawer(cx).is_open()
            // Pick mode leaves home interactive beside the docked pane; the
            // pane itself is fenced off via `split_block_rect`.
            && !self.mini_app_screen(cx).covers_home()
            && !self.ui.modal(cx, ids!(context_menu_modal)).is_open()
            && !self.ui.modal(cx, ids!(background_menu_modal)).is_open()
            && !self.ui.modal(cx, ids!(widget_picker_modal)).is_open()
            && !self.ui.modal(cx, ids!(app_store_modal)).is_open()
            && !self.ui.modal(cx, ids!(import_modal)).is_open()
            && !self.ui.modal(cx, ids!(providers_modal)).is_open()
            && !self.ui.modal(cx, ids!(app_info_modal)).is_open()
            && !self.ui.modal(cx, ids!(source_modal)).is_open()
            && !self.search_overlay(cx).is_open()
    }

    /// Whether the create bar is showing more than its resting one-line pill —
    /// the options row, or a run's console.
    fn composer_expanded(&self) -> bool {
        self.create_options_open || self.activity_active || self.console_finished
    }

    /// Whether the create bar (and its console) is off screen entirely: edit
    /// mode owns the top of the screen, and split-screen pick mode needs the
    /// home half uncluttered — the bar is an overlay sibling of the docked
    /// pane, so leaving it up would let presses land on both.
    fn composer_suppressed(&mut self, cx: &mut Cx) -> bool {
        self.app_state.edit_mode || self.mini_app_screen(cx).is_picking()
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
                    kind: PlacedKind::App { id: app_id.clone(), instance, cols: 1, rows: 1 },
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
                kind: PlacedKind::App { id: app_id.clone(), instance, cols: 1, rows: 1 },
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
        // Kill any live home tiles for this app — widgets and apps running on
        // the grid alike — and reclaim their isolates BEFORE deleting the data
        // dir, or a timer could fire against a removed jail. (force_stop above
        // only tore down the app-screen host.)
        self.home_pager(cx)
            .drop_app_widget_tiles(cx, &self.app_state.layout, app_id);
        makepad_widgets::widget_async::gc_dead_splash_isolates(cx);
        // Archive it first, unless the store can already hand it back. An app
        // the user generated or imported exists nowhere else — the catalog has
        // never heard of it — so removing the manifest here is what made
        // uninstall permanent and unrecoverable.
        let in_catalog = crate::mini_apps::builtin::store_catalog()
            .iter()
            .any(|m| &m.id == app_id);
        if !in_catalog {
            if let Some(manifest) = self.app_state.layout.user_apps.iter().find(|a| &a.id == app_id)
            {
                let manifest = manifest.clone();
                self.app_state.layout.archived_user_apps.retain(|a| &a.id != app_id);
                self.app_state.layout.archived_user_apps.push(manifest);
            }
        }
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
        if self.ui.modal(cx, ids!(providers_modal)).is_open() {
            self.ui.modal(cx, ids!(providers_modal)).close(cx);
            return true;
        }
        if self.ui.modal(cx, ids!(import_modal)).is_open() {
            self.ui.modal(cx, ids!(import_modal)).close(cx);
            return true;
        }
        // Before App Info: the source viewer opens on top of it, and back
        // should peel off one layer at a time.
        if self.ui.modal(cx, ids!(source_modal)).is_open() {
            self.ui.modal(cx, ids!(source_modal)).close(cx);
            return true;
        }
        if self.ui.modal(cx, ids!(providers_modal)).is_open() {
            self.ui.modal(cx, ids!(providers_modal)).close(cx);
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
        // While picking a split partner the drawer may be open OVER the home
        // half; back should peel that off before abandoning the pick.
        if self.mini_app_screen(cx).is_picking() && self.drawer(cx).is_open() {
            self.drawer(cx).close(cx);
            return true;
        }
        // One split-aware step: divider menu, pick-cancel, focused pane, or
        // the fullscreen app — whichever is topmost.
        if self.mini_app_screen(cx).handle_back(cx) {
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

        // The composer starts folded. This is done from here rather than left
        // to the DSL because the fold is a HEIGHT clamp now, and the field's
        // DSL height is its expanded shape — the one place that records "grow
        // with the draft, cap at 75% of the screen". Encoding the folded height
        // there instead would put the two states in two different files.
        self.set_prompt_collapsed(cx, true);

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
            } else if let Some(spec) = state.strip_prefix("homeapp:") {
                // Run an app directly on the home screen: homeapp:clock or
                // homeapp:clock,3,2 to pick the span. Grows the app's existing
                // icon, adding one first if it isn't on home.
                let (app_id, span) = match spec.split(',').collect::<Vec<_>>()[..] {
                    [id] => (id.to_string(), (2u8, 2u8)),
                    [id, c, r] => (
                        id.to_string(),
                        (c.parse().unwrap_or(2), r.parse().unwrap_or(2)),
                    ),
                    _ => (spec.to_string(), (2, 2)),
                };
                let existing = self.app_state.layout.pages.iter().flat_map(|p| &p.items).find_map(
                    |it| match &it.kind {
                        PlacedKind::App { id, instance, .. } if *id == app_id => Some(*instance),
                        _ => None,
                    },
                );
                let instance = match existing {
                    Some(i) => Some(i),
                    None => {
                        self.add_app_to_home(&app_id);
                        self.app_state
                            .layout
                            .pages
                            .iter()
                            .flat_map(|p| &p.items)
                            .find_map(|it| match &it.kind {
                                PlacedKind::App { id, instance, .. } if *id == app_id => {
                                    Some(*instance)
                                }
                                _ => None,
                            })
                    }
                };
                match instance {
                    Some(i) => self.resize_home_app(cx, i, span),
                    None => error!("HOST_LAUNCHER_DEBUG_STATE=homeapp: unknown app {app_id}"),
                }
            } else if let Some(pair) = state.strip_prefix("split:") {
                // Jump straight into a split: split:clock,calculator. Runs the
                // normal pick → open route so it exercises the real code path.
                if let Some((a, b)) = pair.split_once(',') {
                    self.enter_split_pick(cx, &a.to_string(), None);
                    let from = Rect {
                        pos: dvec2(180.0, 640.0),
                        size: dvec2(56.0, 56.0),
                    };
                    self.open_app(cx, &b.to_string(), from);
                } else {
                    error!("HOST_LAUNCHER_DEBUG_STATE=split: needs two ids, split:<a>,<b>");
                }
            } else if let Some(app_id) = state.strip_prefix("pick:") {
                // Split-entry pick mode: one app docked, home live below.
                self.enter_split_pick(cx, &app_id.to_string(), None);
            } else if state == "validate" {
                // Compile-check every known app's Splash source (and its
                // widget's) in throwaway isolates, print a report, and exit.
                // The only way to "build" .splash files from the command line,
                // so app edits can be checked without driving the full UI.
                // Includes the store catalog: dice/tip ship in the binary but
                // aren't installed until bought.
                let mut manifests: Vec<_> = self.app_state.registry.iter().cloned().collect();
                for m in crate::mini_apps::builtin::store_catalog() {
                    if !manifests.iter().any(|have| have.id == m.id) {
                        manifests.push(m);
                    }
                }
                let mut failures = 0usize;
                for m in &manifests {
                    let errors = makepad_widgets::splash::validate_splash_body(
                        cx,
                        &m.source,
                        m.allow_net,
                    );
                    for e in &errors {
                        eprintln!("VALIDATE FAIL {}: {}", m.id, e);
                    }
                    failures += errors.len();
                    if let Some(widget) = &m.widget {
                        let errors = makepad_widgets::splash::validate_splash_body(
                            cx,
                            &widget.source,
                            m.allow_net,
                        );
                        for e in &errors {
                            eprintln!("VALIDATE FAIL {} (widget): {}", m.id, e);
                        }
                        failures += errors.len();
                    }
                }
                eprintln!(
                    "VALIDATE DONE: {} app(s), {} error(s)",
                    manifests.len(),
                    failures
                );
                std::process::exit(if failures == 0 { 0 } else { 1 });
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
            } else if let Some(app_id) = state.strip_prefix("source:") {
                // App Info with the source viewer already open on top.
                let app_id = app_id.to_string();
                self.open_app_info(cx, &app_id);
                self.open_source_view(cx, &app_id);
            } else if state == "zorder" {
                // Repro for the glass z-order bug: fill the console so it
                // covers the grid, THEN force the clock widget's tile to be
                // re-created. A fresh tile claims a fresh overlay draw-list
                // slot, which lands after the bar's — and a glass surface
                // composites in slot order, so the widget draws on top of the
                // console. (An install + redraw_all does the same thing.)
                self.set_create_bar_busy(cx, "Writing the app…");
                let log = (1 ..= 24)
                    .map(|i| format!("Agent connected — writing the app ({i})"))
                    .collect::<Vec<_>>()
                    .join("\n");
                self.console_trail = log;
                self.sync_console(cx);
                // The drop has to land AFTER a frame has been drawn — that's
                // what an install does (redraw_all with the console already up),
                // and doing it before the first draw just recreates the tile in
                // the original order.
                self.zorder_repro = cx.start_timeout(2.0);
            } else if state == "collapsed" {
                // The reported case: a long single-line draft with the composer
                // dismissed. The line must be fully visible, not clipped.
                self.ui
                    .text_input(cx, ids!(create_input))
                    .set_text(cx, "lksjdflkaj sdlkfj alskdjf lskdjf alskdjf");
                let text = self.ui.text_input(cx, ids!(create_input)).text();
                self.sync_create_send(cx, &text);
                // Focus it like a real user would before typing: the caret is
                // what the one-line height is measured from.
                self.ui.widget(cx, ids!(create_input)).set_key_focus(cx);
                self.open_agent_options(cx);
                // Collapse only after frames have drawn — the one-line height is
                // measured from the laid-out field, which doesn't exist yet at
                // startup (reusing the zorder repro's timer).
                self.zorder_repro = cx.start_timeout(2.0);
            } else if state == "errcollapse" {
                // The reported case: a long API error, then a press outside.
                // The bar must fold to ONE line like the draft does — a status
                // that wraps made the "collapsed" bar taller than the composer.
                self.flash_create_bar(
                    cx,
                    "You've reached your usage limit for this billing cycle. Your quota \
                     will be refreshed in the next cycle. To continue now, upgrade your \
                     plan or wait for the reset.",
                );
                self.failed_run = Some(FailedRun {
                    request: "a pomodoro timer".to_string(),
                    refine_of: None,
                    escalate: None,
                });
                self.sync_console_buttons(cx);
                self.activity_collapsed = true;
                self.sync_activity_panel(cx);
            } else if state == "opts" {
                // Saved picks restored into the controls. The pill must land on
                // Sonnet 5 / High / On — if it sits on Default instead, the
                // control is lying about its selection and clicking the segment
                // you want will do nothing (see GlassSegmented::set_selected).
                self.agent_prefs.model = Some("claude-sonnet-5".to_string());
                self.agent_prefs.effort = Some("high".to_string());
                self.agent_prefs.thinking = Some("on".to_string());
                self.ui.text_input(cx, ids!(create_input)).set_text(cx, "a fitness tracker");
                let text = self.ui.text_input(cx, ids!(create_input)).text();
                self.sync_create_send(cx, &text);
                self.open_agent_options(cx);
            } else if state == "gendone" {
                // A finished, successful run: the console with Open + New prompt.
                self.set_create_bar_busy(cx, "Writing the app…");
                self.console_trail = "Agent connected — writing the app\n\
                     🔧 Read splash_guide.md\n\
                     Validating with the Splash parser\n\
                     Compiles clean — installing"
                    .to_string();
                self.sync_console(cx);
                self.flash_create_bar(cx, "Calculator added ✓");
                self.finished_app = Some("calculator".to_string());
                self.sync_console_buttons(cx);
            } else if state == "genfail" {
                // A run that ran out of repairs, with the inline Retry. The
                // escalation is forced on so the "harder" label is visible
                // even on a backend with no effort knob.
                self.set_create_bar_busy(cx, "Fixing the app (try 2)…");
                self.console_trail = "Agent connected — writing the app\n\
                     🔧 Read splash_guide.md\n\
                     ⚠ line 14: expected `}` to close the block\n\
                     Sending errors back (repair 2)"
                    .to_string();
                self.sync_console(cx);
                self.failed_run = Some(FailedRun {
                    request: "a pomodoro timer".to_string(),
                    refine_of: None,
                    escalate: Some("max".to_string()),
                });
                self.flash_create_bar(cx, "The generated app kept failing to compile");
            } else if state == "import" {
                // Import App with a couple of exported files to list. WRITES
                // to the exchange folder, so it's fresh-run only.
                if !Self::is_fresh_run() {
                    error!("HOST_LAUNCHER_DEBUG_STATE=import: needs HOST_LAUNCHER_FRESH=1");
                } else {
                    for id in ["calculator", "todo"] {
                        if let Some(m) = self.app_state.registry.get(id).cloned() {
                            let _ = crate::mini_apps::bundle::write_export(&m);
                        }
                    }
                }
                self.open_import_modal(cx);
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
                self.console_trail = log;
                self.console_stream = "    let total = habits.map(|h| h.done.len()).sum()\n\
                     label.set_text(cx, &format!(\"{total} done\"))\n\
                 }"
                    .to_string();
                self.sync_console(cx);
            } else if state == "setup" || state == "providers" {
                self.open_providers(cx);
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
            // `blocker()` rather than `any_configured()`: a saved key with no
            // octos to run it LOOKS configured and generates nothing, so the
            // stricter question — can a run actually happen? — is the one to
            // ask at the moment the user reaches for the prompt.
            if crate::generate::blocker().is_none() {
                self.open_agent_options(cx);
            } else {
                self.open_providers(cx);
            }
        }
        for slot in 0 .. Self::SEGMENT_IDS.len() {
            let seg = self.ui.glass_segmented(cx, Self::SEGMENT_IDS[slot]);
            if seg.changed(actions) {
                let index = seg.selected();
                self.pick_agent_option(cx, slot, index);
            }
        }
        // The other two effort ladders are separate controls in the same row.
        for alt in [ids!(opt_1.ao_seg_1x), ids!(opt_1.ao_seg_1k)] {
            let seg = self.ui.glass_segmented(cx, alt);
            if seg.changed(actions) {
                let index = seg.selected();
                self.pick_agent_option(cx, KnobId::Effort.row(), index);
            }
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
            self.pending_confirm = Some(PendingConfirm::StopGeneration);
            self.ui.glass_button(cx, ids!(confirm_remove)).set_text(cx, "Stop");
            self.ui.label(cx, ids!(confirm_body)).set_text(
                cx,
                "Stop the agent? What it has written so far is discarded.",
            );
            self.ui.modal(cx, ids!(confirm_remove_modal)).open(cx);
        }
        if self.ui.glass_button(cx, ids!(create_retry)).clicked(actions) {
            self.retry_failed_run(cx);
        }
        if self.ui.glass_button(cx, ids!(create_open)).clicked(actions) {
            self.open_finished_app(cx);
        }
        if self.ui.glass_button(cx, ids!(create_done)).clicked(actions) {
            // The ONE place the prompt is cleared by a dismissal. "New
            // prompt" is an explicit "start me over" — every other way out of
            // the console (a press outside, Open, losing focus) leaves
            // whatever the user typed exactly where they left it.
            self.ui.text_input(cx, ids!(create_input)).set_text(cx, "");
            // The one thing that throws the run's output away. Everything else
            // — a press outside, the chevron, Open, losing focus — keeps the
            // whole transcript so it can still be scrolled back through.
            self.console_trail = String::new();
            self.console_stream = String::new();
            let console = self.ui.widget(cx, ids!(create_output));
            if let Some(mut console) =
                console.borrow_mut::<crate::launcher::agent_console::LauncherAgentConsole>()
            {
                console.clear(cx);
            }
            self.console_painted_len = 0;
            self.console_painted_at = None;
            self.dismiss_console(cx);
            // Hand the caret straight back: "New prompt" means the user is
            // about to type the next one, and making them tap the field first
            // is a step for nothing.
            //
            self.prompt_focus_tries = PROMPT_FOCUS_TRIES;
            // The options row is opened HERE, not left to the field's
            // `KeyFocus` action. The field usually never lost focus in the
            // first place — the user typed the prompt that produced this
            // console — so re-focusing it emits nothing, and a composer that
            // relied on that action stayed folded shut with a live caret in
            // an invisible one-line field.
            self.open_agent_options(cx);
        }

        if self.ui.glass_button(cx, ids!(create_providers)).clicked(actions) {
            self.open_providers(cx);
        }

        // The ✨ is the other way in to the same page.
        if self.ui.button(cx, ids!(create_glyph)).clicked(actions) {
            self.open_providers(cx);
        }

        for action in actions {
            if let SourceModalAction::Close = action.as_widget_action().cast() {
                self.ui.modal(cx, ids!(source_modal)).close(cx);
            }
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
                    HomePagerAction::ExpandAppTile { instance, app_id, from_rect } => {
                        // Hand the RUNNING widget to the app screen — same
                        // isolate, same state, just a bigger rect. If the app
                        // screen is busy with something else, ignore it rather
                        // than yanking the user out of that.
                        if !self.mini_app_screen(cx).is_showing() {
                            if let Some(host) = self.home_pager(cx).lend_app_host(cx, instance) {
                                self.stamp_recents(&app_id);
                                self.mini_app_screen(cx)
                                    .adopt_host(cx, &app_id, host, from_rect);
                                cx.redraw_all();
                            }
                        }
                    }
                    HomePagerAction::ShrinkAppTile { instance } => {
                        self.resize_home_app(cx, instance, (1, 1));
                    }
                    HomePagerAction::ReturnAppTile { instance } => {
                        let _ = instance;
                        self.return_expanded_app(cx);
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
                        // Not while a split pick has a pane docked over the
                        // grid — jiggle mode under an app makes no sense.
                        if !self.mini_app_screen(cx).is_picking() {
                            self.app_state.edit_mode = true;
                        }
                        cx.redraw_all();
                    }
                    ContextMenuAction::SplitScreen(app_id) => {
                        let anchor = self.menu_anchor;
                        self.close_context_menu(cx);
                        self.drawer(cx).close(cx);
                        self.search_overlay(cx).close(cx);
                        self.enter_split_pick(cx, &app_id, anchor);
                    }
                    ContextMenuAction::ShrinkToIcon(instance) => {
                        self.close_context_menu(cx);
                        self.resize_home_app(cx, instance, (1, 1));
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
                    AppInfoAction::ViewSource(app_id) => {
                        self.open_source_view(cx, &app_id);
                    }
                    AppInfoAction::Export(app_id) => {
                        self.export_app(cx, &app_id);
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
                        // Not while a split pick has a pane docked over the grid.
                        if !self.mini_app_screen(cx).is_picking() {
                            self.app_state.edit_mode = true;
                        }
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
                    BackgroundMenuAction::ImportApp => {
                        self.close_background_menu(cx);
                        self.open_import_modal(cx);
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

                let providers_action = widget_action.cast::<ProvidersAction>();
                if !matches!(providers_action, ProvidersAction::None) {
                    self.handle_providers_action(cx, providers_action);
                }

                match widget_action.cast::<ImportModalAction>() {
                    ImportModalAction::InstallFile(path) => {
                        match std::fs::read_to_string(&path) {
                            Ok(text) => self.import_app(cx, &text),
                            Err(e) => self
                                .ui
                                .launcher_import_modal(cx, ids!(import_modal.content))
                                .set_status(cx, &format!("can't read that file: {e}")),
                        }
                    }
                    ImportModalAction::InstallText(text) => self.import_app(cx, &text),
                    ImportModalAction::OpenFolder => {
                        let dir = crate::mini_apps::bundle::exchange_dir();
                        let _ = std::fs::create_dir_all(&dir);
                        cx.open_url(&format!("file://{}", dir.display()), OpenUrlInPlace::No);
                    }
                    ImportModalAction::None => (),
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
                    // If what just closed was a home tile's app on loan, put
                    // it back in its cells still running.
                    self.return_expanded_app(cx);
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
                Some(PendingConfirm::StopGeneration) => {
                    // Only if it's still running: the run can finish while the
                    // sheet is up, and cancelling then would tear down a
                    // console the user is reading.
                    if self.generation.is_some() {
                        self.cancel_generation(cx);
                    }
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
        // CodeView, for the App Info → View source popup.
        makepad_code_editor::script_mod(vm);
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
        if self.zorder_repro.is_event(event).is_some() {
            if std::env::var("HOST_LAUNCHER_DEBUG_STATE").as_deref() == Ok("collapsed") {
                self.close_agent_options(cx);
            } else {
                let layout = self.app_state.layout.clone();
                self.home_pager(cx).drop_app_widget_tiles(cx, &layout, &"clock".to_string());
                cx.redraw_all();
            }
        }
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
        // ...and whether the drawer or search overlay covers a pick in
        // progress. Both draw BELOW the mini-app screen (an app opened from
        // the drawer must zoom on top of it), so while one is up the docked
        // sliver stops drawing and fencing — the covering layer is
        // effectively frontmost and every app in it is pickable.
        let pick_obscured =
            self.drawer(cx).is_open() || self.search_overlay(cx).is_open();
        self.mini_app_screen(cx).set_pick_obscured(cx, pick_obscured);
        // ...and where home's page-indicator dots are, so the pick hint can
        // park in the gap just above them instead of floating over the grid.
        let dots = self.ui.widget(cx, ids!(page_indicator)).area().rect(cx);
        self.mini_app_screen(cx).set_hint_anchor(cx, dots);
        // ...and where the split-pick docked sliver is, so home-layer widgets
        // ignore presses that land on the app peeking over them.
        self.app_state.split_block_rect = self.mini_app_screen(cx).pick_block_rect();
        // ...and whether widget tiles must stay out of the frame entirely. A
        // glass tile composites its whole subtree ABOVE the main pass, so any
        // visible tile floats in front of an app pane; while a pane exists
        // (including pick mode, where home stays visible) the tiles don't
        // draw. Flips pair with a full repaint: a tile that stops drawing
        // leaves its overlay draw list behind until a full pass flushes it.
        // ...except during open/close ZOOMS: the moving window draws in its
        // own overlay ABOVE the tiles' glass, so the launcher — widgets and
        // all — can sit behind the animation the whole way.
        let hide_tiles = self.mini_app_screen(cx).is_showing()
            && !self.mini_app_screen(cx).is_zooming();
        if hide_tiles != self.app_state.hide_widget_tiles {
            self.app_state.hide_widget_tiles = hide_tiles;
            cx.redraw_all();
        }
        // ...and where the create bar is. It FLOATS over the grid now, so the
        // pager (an overlay sibling, which also sees the event) must ignore
        // presses that land on it — otherwise typing in an expanded prompt
        // would double as tapping the icons underneath.
        // Deferred prompt focus, retried until it actually STICKS.
        //
        // A one-shot `set_key_focus` is not enough: the caret only draws when
        // the field holds focus AND its blink animator is running
        // ((1.0 - blink) * focus in the shader), and a focus set on the frame
        // the field is revealed can land before the widget can run that
        // bookkeeping — so it silently didn't take, which is why the caret and
        // selection came and went. Ask via the widget (so its own focus/blink
        // handling runs) and keep asking until `has_key_focus` confirms it.
        if self.prompt_focus_tries > 0 {
            self.prompt_focus_tries -= 1;
            let area = self.ui.widget(cx, ids!(create_input)).area();
            if !area.is_empty() {
                // `take_key_focus`, NOT the generic `set_key_focus`, and asked
                // for even when the field already holds focus — which, after a
                // run, it usually does: it was never unfocused, only hidden
                // behind the console. Setting key focus it already has
                // dispatches no KeyFocus hit, so the caret and selection
                // animators stay parked wherever the last focus-lost left
                // them, and the field comes back typable with nothing drawn in
                // it. `take_key_focus` turns those animators on itself.
                self.ui.text_input(cx, ids!(create_input)).take_key_focus(cx);
                self.prompt_focus_tries = 0;
            }
        }
        // Re-place the context menu from its MEASURED height. `show()` returns
        // an estimate that is deliberately generous, and for a menu placed
        // ABOVE its icon every extra pixel of that estimate pushes it further
        // from the icon — which is exactly what it looked like.
        if let Some(anchor) = self.menu_anchor {
            let measured = self.ui.widget(cx, ids!(context_menu_modal.content)).area().rect(cx);
            if measured.size.y > 1.0 && (measured.size.y - self.menu_placed_h).abs() > 0.5 {
                self.menu_placed_h = measured.size.y;
                self.place_popup(
                    cx,
                    ids!(context_menu_modal.content),
                    // The measured rect already contains the callout triangle
                    // (it's a child of the panel), so it is NOT added again.
                    dvec2(MENU_WIDTH, measured.size.y),
                    anchor,
                );
            }
        }
        self.app_state.create_rect = if self.composer_suppressed(cx) {
            Rect::default()
        } else {
            self.ui.widget(cx, ids!(create_bar)).area().rect(cx)
        };
        // A press outside the bar closes the options row. Inside — including on
        // the controls themselves — must not, which is why this tests the bar's
        // whole rect rather than the prompt's focus.
        // Same guard as the console below: a hidden bar has an empty rect, and
        // that must not read as "the press was outside".
        if self.create_options_open && self.app_state.create_rect.size.x > 0.0 {
            let outside = |abs: DVec2| !self.app_state.create_rect.contains(abs);
            let dismissed = match event {
                Event::MouseDown(e) => outside(e.abs),
                Event::TouchUpdate(e) => e.touches.iter().any(|t| outside(t.abs)),
                _ => false,
            };
            if dismissed {
                self.close_agent_options(cx);
            }
        }
        // A press outside COLLAPSES a finished run's output. It does not
        // dismiss it and it does not clear anything: pressing outside is how
        // you look at something else, not how you say "I'm done with this".
        // The whole bar folds to its one-line resting state, the log and the
        // Retry/Open offers survive, and the chevron (or reopening the bar)
        // brings it all back. Only "New prompt" tears it down — see `create_done`.
        //
        // Guarded on the bar actually being on screen: an empty rect means
        // it's hidden (edit mode, an open app, the drawer), and treating
        // "hidden" as "everything is outside" collapsed a finished run on the
        // user's next tap anywhere, before they ever saw the result.
        if self.console_finished
            && !self.activity_collapsed
            && self.app_state.create_rect.size.x > 0.0
        {
            let outside = |abs: DVec2| !self.app_state.create_rect.contains(abs);
            let pressed_outside = match event {
                Event::MouseDown(e) => outside(e.abs),
                Event::TouchUpdate(e) => e.touches.iter().any(|t| outside(t.abs)),
                _ => false,
            };
            if pressed_outside {
                self.activity_collapsed = true;
                self.sync_activity_panel(cx);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The status strip is one line, so long messages have to be shortened —
    /// but shortened so it READS as shortened. The old behaviour cut at a
    /// fixed character count, which landed mid-word and looked like the
    /// message itself had been lost.
    #[test]
    fn a_long_status_is_cut_at_a_word_with_an_ellipsis() {
        let long = "You've reached your usage limit for this month and will regain \
                    access when the quota resets on the first of next month, sorry";
        let out = App::headline_of(long);
        assert!(out.ends_with('…'), "{out}");
        assert!(out.chars().count() <= 111, "{} chars", out.chars().count());
        // Cut at a space: the last word is whole, not sliced.
        let body = out.trim_end_matches('…');
        assert!(long.starts_with(body), "{body} is not a prefix of the message");
        assert!(!body.ends_with(' '));
    }

    /// A message that fits is left exactly alone — no stray ellipsis on a
    /// perfectly complete sentence.
    #[test]
    fn a_short_status_is_untouched() {
        assert_eq!(App::headline_of("Cancelled"), "Cancelled");
        // Newlines would break the single-line strip's layout.
        assert_eq!(App::headline_of("two\nlines"), "two lines");
    }
}
