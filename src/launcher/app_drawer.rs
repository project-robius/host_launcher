//! The Android-style app drawer: swiping up on the home screen slides a
//! full-height panel over it, listing every installed app in a grid, with a
//! sort button that toggles between alphabetical and most-recently-used order.

use makepad_widgets::*;
use makepad_widgets::makepad_platform::event::TouchState;

use crate::{app::AppState, mini_apps::registry::MiniAppId};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.DrawerItemBase = #(DrawerItem::register_widget(vm))

    mod.widgets.DrawerItem = set_type_default() do mod.widgets.DrawerItemBase{
        width: Fill
        height: Fill
        flow: Down
        spacing: 5
        // Centre the icon+label within each equal-width cell. With 4 Fill cells this
        // puts the 4-column group's centre exactly at the drawer's centre (symmetric
        // left/right margins) regardless of drawer width.
        align: Align{x: 0.5, y: 0.5}
        d_tile := LauncherIconTile{
            d_glyph := LauncherIconGlyph{}
        }
        d_name := LauncherIconName{}
    }

    mod.widgets.AppDrawerBase = #(AppDrawer::register_widget(vm))

    mod.widgets.AppDrawer = set_type_default() do mod.widgets.AppDrawerBase{
        width: Fill
        height: Fill
        flow: Overlay

        // A full-bleed liquid-glass sheet over the wallpaper (the home is hidden
        // behind it, see sync_overlays): white-tinted frosted glass like the menus.
        // It runs off the sides and bottom of the screen (negative margins) so only
        // the rounded TOP edge with the grab bar is visible. The whole drawer is
        // drawn into an overlay (see draw_walk) so this glass draws INLINE and the
        // chrome/list below paint crisply ON TOP of its lens instead of under it.
        drawer_glass := glass.LensSurface{
            width: Fill
            height: Fill
            margin: Inset{left: -6, right: -6, bottom: -60}
            draw_bg +: {
                corner_radius: 28.0
                tint_color: #xf8fbff
                blur_level: 0.5
                tint_alpha: 0.1
                border_alpha: 0.45
            }
        }

        // The drawer's own chrome + grid, painted on top of the glass.
        View{
            width: Fill
            height: Fill
            flow: Down
            padding: Inset{top: 10, left: 22, right: 22, bottom: 8}

            // Grab bar for drag-to-close.
            grab := View{
                width: Fill
                height: 22
                align: Align{x: 0.5, y: 0.5}
                RoundedView{
                    width: 44
                    height: 5
                    show_bg: true
                    draw_bg +: {
                        color: #xffffff30
                        border_radius: 2.5
                    }
                }
            }

            header := View{
                width: Fill
                height: Fit
                flow: Right
                align: Align{y: 0.5}
                padding: Inset{left: 6, right: 6, bottom: 8}
                Label{
                    text: "Apps"
                    draw_text +: {
                        color: #ffffff
                        text_style: theme.font_bold{font_size: 17}
                    }
                }
                View{width: Fill, height: 1}
                sort_button := glass.GlassButton{
                    text: "A–Z"
                    height: 36
                    draw_text +: {
                        text_style: theme.font_bold{font_size: 12}
                    }
                }
            }

            // Filter-as-you-type search field.
            search_row := View{
                width: Fill
                height: Fit
                padding: Inset{left: 2, right: 2, bottom: 10}
                search_input := LauncherTextInput{
                    empty_text: "Search apps"
                }
            }

            list := PortalList{
                width: Fill
                height: Fill
                flow: Down
                drag_scrolling: true

                Row := View{
                    width: Fill
                    height: 96
                    flow: Right
                    cell_0 := mod.widgets.DrawerItem{}
                    cell_1 := mod.widgets.DrawerItem{}
                    cell_2 := mod.widgets.DrawerItem{}
                    cell_3 := mod.widgets.DrawerItem{}
                }
            }
        }
    }
}

/// Number of icon columns in the drawer grid.
const DRAWER_COLS: usize = 4;
/// How long a press must be held (secs) to count as a long press.
const LONG_PRESS_SECS: f64 = 0.5;

/// Actions emitted by the drawer for the app to handle.
#[derive(Clone, Debug, Default)]
pub enum AppDrawerAction {
    /// An app was tapped in the drawer.
    OpenApp { app_id: MiniAppId, from_rect: Rect },
    /// An app was long-pressed in the drawer.
    ShowContextMenu { app_id: MiniAppId, anchor: Rect },
    /// An app was long-pressed and held: dismiss the drawer and hand the finger
    /// to the home pager so the app can be dragged onto the home screen. `area`
    /// is the drawer cell's finger capture; `abs` is where the drag starts.
    DragOutApp { app_id: MiniAppId, area: Area, abs: Vec2d },
    #[default]
    None,
}

/// One tappable/long-pressable cell in the drawer grid. Kept passive except for
/// press tracking, so PortalList's drag-scrolling still works over it: any finger
/// movement past the tap slop makes `was_tap()` false and cancels the long press.
#[derive(Script, ScriptHook, Widget)]
pub struct DrawerItem {
    #[deref]
    view: View,
    /// Which app (by drawer-sorted index) this cell currently shows.
    #[rust]
    app_id: Option<MiniAppId>,
    /// The app whose visuals were last applied, so we skip re-applying every frame.
    #[rust]
    applied_app: Option<MiniAppId>,
    /// The manifest look this cell last applied (name, icon, tint). Compared
    /// alongside the id so an app refined IN PLACE (same id, new identity)
    /// re-applies instead of keeping the stale look forever.
    applied_look: Option<(String, String, u32)>,
    /// Whether `set_app` has run at least once (so the first empty cell still hides).
    #[rust]
    applied: bool,
    #[rust]
    press_timer: Timer,
    #[rust]
    long_press_fired: bool,
}

/// Emitted by a drawer/search grid cell. Reused by the search overlay, which
/// hosts the same `DrawerItem` cells.
#[derive(Clone, Debug, Default)]
pub enum DrawerItemAction {
    Tapped { app_id: MiniAppId, rect: Rect },
    /// `area` is the cell's captured finger area, handed to the pager so the
    /// long-press can flow straight into dragging the app onto the home screen.
    LongPressed { app_id: MiniAppId, rect: Rect, area: Area },
    #[default]
    None,
}

impl Widget for DrawerItem {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        let Some(app_id) = self.app_id.clone() else {
            return;
        };
        let uid = self.widget_uid();

        if self.press_timer.is_event(event).is_some() && !self.long_press_fired {
            self.long_press_fired = true;
            cx.widget_action(
                uid,
                DrawerItemAction::LongPressed {
                    app_id: app_id.clone(),
                    rect: self.view.area().rect(cx),
                    area: self.view.area(),
                },
            );
        }

        match event.hits(cx, self.view.area()) {
            Hit::FingerDown(fe) if fe.device.is_primary_hit() => {
                // Presses under a split-pick's docked pane belong to that app,
                // not this cell (overlay siblings both see the event); fencing
                // here covers the drawer AND the search overlay, which hosts
                // the same cells.
                let blocked = scope.data.get::<AppState>().is_some_and(|s| {
                    s.split_block_rect.size.x > 0.0 && s.split_block_rect.contains(fe.abs)
                });
                if blocked {
                    return;
                }
                self.long_press_fired = false;
                self.press_timer = cx.start_timeout(LONG_PRESS_SECS);
            }
            Hit::FingerMove(fe) => {
                if fe.move_distance() > 6.0 {
                    cx.stop_timer(self.press_timer);
                }
            }
            Hit::FingerHoverIn(_) => {
                cx.set_cursor(MouseCursor::Hand);
            }
            Hit::FingerUp(fe) => {
                cx.stop_timer(self.press_timer);
                // A native (mobile) long press reported here on the *up* event is
                // deliberately ignored. Drag-out — the only thing a drawer long-press
                // does — must hand the still-down finger's capture to the pager; by
                // FingerUp the finger is already released, so acting on it would hand
                // over a dead capture and wedge the pager in a drag it can never end.
                // (On mobile the touch is still captured at this point in the event
                // cycle, so begin_external_drag's abort can't catch it — the only safe
                // place to stop it is here.) The real drag-out fires from press_timer
                // while the finger is held; here we only handle a genuine tap.
                // Same fence as FingerDown: the down-hit captured this area
                // before the fence could return, so the up must re-check.
                let blocked = scope.data.get::<AppState>().is_some_and(|s| {
                    s.split_block_rect.size.x > 0.0 && s.split_block_rect.contains(fe.abs)
                });
                if fe.was_tap() && fe.is_over && !self.long_press_fired && !blocked {
                    cx.widget_action(
                        uid,
                        DrawerItemAction::Tapped {
                            app_id,
                            rect: self.view.area().rect(cx),
                        },
                    );
                }
            }
            _ => (),
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl DrawerItem {
    pub fn set_app(&mut self, cx: &mut Cx, state: &AppState, app_id: Option<MiniAppId>) {
        // A recycled PortalList item is redrawn every frame; only touch the widget
        // tree when the cell's app actually changes, so we don't re-eval the tint
        // shader (and re-dirty the tree) on every frame. "Changes" includes the
        // manifest's look changing under the SAME id (an AI refine renames or
        // restyles in place), so the cached look is part of the key.
        let look = app_id
            .as_ref()
            .and_then(|id| state.registry.get(id))
            .map(|m| (m.name.clone(), m.icon.clone(), m.tint));
        if self.applied && self.applied_app == app_id && self.applied_look == look {
            return;
        }
        self.applied = true;
        self.applied_app = app_id.clone();
        self.applied_look = look;
        self.app_id = app_id.clone();
        match app_id.as_ref().and_then(|id| state.registry.get(id)) {
            Some(manifest) => {
                self.view.set_visible(cx, true);
                self.view.widget(cx, ids!(d_tile)).set_visible(cx, true);
                self.view.label(cx, ids!(d_glyph)).set_text(cx, &manifest.icon);
                self.view.label(cx, ids!(d_name)).set_text(cx, &manifest.name);
                let tint = crate::launcher::home_pager::tile_tint_color(manifest.tint);
                let mut tile = self.view.widget(cx, ids!(d_tile));
                script_apply_eval!(cx, tile, {
                    draw_bg +: { color: #(tint) }
                });
            }
            None => {
                // Keep the empty cell at full width (just hide its content) so it
                // holds its column — otherwise a lone icon in the last row would
                // expand to fill the whole row and drift to the centre instead of
                // staying under its own column.
                self.view.set_visible(cx, true);
                self.view.widget(cx, ids!(d_tile)).set_visible(cx, false);
                self.view.label(cx, ids!(d_glyph)).set_text(cx, "");
                self.view.label(cx, ids!(d_name)).set_text(cx, "");
            }
        }
    }
}

/// The drawer's slide phase (the continuous 0..1 position is tracked separately
/// in `progress`).
#[derive(Clone, Copy, Default, PartialEq)]
enum DrawerAnim {
    #[default]
    Hidden,
    Animating,
    Open,
    /// The user is dragging the drawer open with a finger (driven by the pager).
    Dragging,
}

#[derive(Script, ScriptHook, Widget)]
pub struct AppDrawer {
    #[deref]
    view: View,
    #[rust]
    progress: f64,
    #[rust]
    target: f64,
    #[rust]
    anim: DrawerAnim,
    #[rust]
    next_frame: NextFrame,
    #[rust]
    last_frame_time: f64,
    /// True = most-recently-used order, false = alphabetical.
    #[rust]
    sort_recent: bool,
    /// Current search-filter query (lowercased); empty shows everything.
    #[rust]
    query: String,
    /// Finger-drag on the grab/header area to close.
    #[rust]
    drag_close: Option<f64>,
    #[rust]
    last_rect: Rect,
    /// Overlay draw-list the whole drawer renders into, so the glass draws INLINE
    /// (sampling the backdrop) and the chrome/list paint ON TOP of the lens and stay
    /// crisp. Drawing the content as a plain sibling put it under the lens (fuzzy).
    #[rust]
    overlay_list: Option<DrawList2d>,
}

impl AppDrawer {
    /// The app ids in current drawer order, filtered by the search query.
    fn sorted_ids(&self, state: &AppState) -> Vec<MiniAppId> {
        let q = self.query.trim();
        let mut ids: Vec<MiniAppId> = state
            .registry
            .iter()
            .filter(|m| q.is_empty() || m.name.to_lowercase().contains(q))
            .map(|m| m.id.clone())
            .collect();
        if self.sort_recent {
            ids.sort_by(|a, b| {
                let ra = state.layout.recents.get(a).copied().unwrap_or(0);
                let rb = state.layout.recents.get(b).copied().unwrap_or(0);
                rb.cmp(&ra).then_with(|| {
                    let na = state.registry.get(a).map(|m| m.name.to_lowercase());
                    let nb = state.registry.get(b).map(|m| m.name.to_lowercase());
                    na.cmp(&nb)
                })
            });
        } else {
            ids.sort_by_key(|id| state.registry.get(id).map(|m| m.name.to_lowercase()));
        }
        ids
    }

    fn start_anim(&mut self, cx: &mut Cx, target: f64) {
        self.target = target;
        self.anim = DrawerAnim::Animating;
        self.last_frame_time = 0.0;
        self.next_frame = cx.new_next_frame();
        self.redraw(cx);
    }

    pub fn open(&mut self, cx: &mut Cx) {
        // Start each session with a clean, unfiltered list.
        self.query.clear();
        self.view.text_input(cx, ids!(search_input)).set_text(cx, "");
        // Reset the grid to the top. open() never touched the list, so a stale
        // first_id/first_scroll survived from a prior open/drag/search and the list
        // re-opened mis-normalised (icons floating mid-drawer until the next event —
        // the overlay-wrap suppresses the list's own settle pass).
        self.view
            .portal_list(cx, ids!(list))
            .set_first_id_and_scroll(0, 0.0);
        self.start_anim(cx, 1.0);
    }

    pub fn close(&mut self, cx: &mut Cx) {
        self.start_anim(cx, 0.0);
    }

    /// Drives the drawer's open fraction directly from a finger drag (0 = closed,
    /// 1 = fully open), as the pager forwards an upward swipe in progress.
    pub fn set_drag(&mut self, cx: &mut Cx, progress: f64) {
        if self.anim != DrawerAnim::Dragging {
            // Starting a fresh drag-open: clear any leftover search filter and reset
            // the grid to the top (same stale-scroll reason as open()).
            self.query.clear();
            self.view.text_input(cx, ids!(search_input)).set_text(cx, "");
            self.view
                .portal_list(cx, ids!(list))
                .set_first_id_and_scroll(0, 0.0);
        }
        self.progress = progress.clamp(0.0, 1.0);
        self.anim = DrawerAnim::Dragging;
        self.redraw(cx);
    }

    /// Ends a finger drag: snaps the drawer fully open or closed.
    pub fn settle(&mut self, cx: &mut Cx, open: bool) {
        self.start_anim(cx, if open { 1.0 } else { 0.0 });
    }

    pub fn is_open(&self) -> bool {
        // Only "open" once settled or animating toward open, never mid finger-drag,
        // so the pager keeps receiving the drag until the finger lifts.
        matches!(self.anim, DrawerAnim::Open)
            || (self.anim == DrawerAnim::Animating && self.target > 0.5)
    }
}

impl Widget for AppDrawer {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Slide animation.
        if let Some(ne) = self.next_frame.is_event(event) {
            if self.anim == DrawerAnim::Animating {
                let dt = if self.last_frame_time == 0.0 {
                    1.0 / 60.0
                } else {
                    (ne.time - self.last_frame_time).clamp(0.0, 0.1)
                };
                self.last_frame_time = ne.time;
                let diff = self.target - self.progress;
                if diff.abs() < 0.004 {
                    self.progress = self.target;
                    self.anim = if self.target > 0.5 {
                        DrawerAnim::Open
                    } else {
                        DrawerAnim::Hidden
                    };
                    // The drawer covers glass content; make sure hiding it repaints fully.
                    cx.redraw_all();
                } else {
                    // Slower, unhurried slide IN; quicker slide OUT. The
                    // exponential rate is "how fast the remaining gap closes",
                    // so a smaller number = a longer, calmer glide. Opening is
                    // the moment worth watching; closing just needs to get out
                    // of the way.
                    let rate = if self.target > 0.5 { 3.0 } else { 15.0 };
                    self.progress += diff * (1.0 - (-dt * rate).exp());
                    self.next_frame = cx.new_next_frame();
                }
                self.redraw(cx);
            }
        }

        // Once closing (target 0), the drawer is on its way out and an app may be
        // zooming up over it; stop consuming input immediately so the drawer doesn't
        // steal taps meant for the layer above, even while its slide-out animates.
        if self.anim == DrawerAnim::Hidden || self.target < 0.5 {
            return;
        }

        // A click/tap in the strip ABOVE the drawer's rounded top edge dismisses it
        // (tap-outside-to-close). The drawer is full-bleed on the other three sides,
        // so the region above its top is the only "outside".
        let drawer_top = self.last_rect.pos.y;
        let tapped_above = match event {
            Event::MouseDown(fd) => fd.abs.y < drawer_top,
            Event::TouchUpdate(e) => e
                .touches
                .iter()
                .any(|t| matches!(t.state, TouchState::Start) && t.abs.y < drawer_top),
            _ => false,
        };
        if tapped_above {
            self.close(cx);
            return;
        }

        self.view.handle_event(cx, event, scope);

        // Sort toggle.
        if let Event::Actions(actions) = event {
            if self.view.glass_button(cx, ids!(sort_button)).clicked(actions) {
                self.sort_recent = !self.sort_recent;
                let label = if self.sort_recent { "Recent" } else { "A–Z" };
                self.view.glass_button(cx, ids!(sort_button)).set_text(cx, label);
                self.redraw(cx);
            }

            // Filter-as-you-type: refresh the grid on every keystroke.
            if let Some(text) = self.view.text_input(cx, ids!(search_input)).changed(actions) {
                self.query = text.to_lowercase();
                self.redraw(cx);
            }

            // Bubble up item taps/long-presses as drawer actions.
            let uid = self.widget_uid();
            for action in actions {
                if let Some(item_action) = action.as_widget_action() {
                    match item_action.cast::<DrawerItemAction>() {
                        DrawerItemAction::Tapped { app_id, rect } => {
                            cx.widget_action(uid, AppDrawerAction::OpenApp {
                                app_id,
                                from_rect: rect,
                            });
                        }
                        DrawerItemAction::LongPressed { app_id, rect, area } => {
                            // Long-press+hold a drawer app => drag it onto the home
                            // (Android-style), not a context menu.
                            cx.widget_action(uid, AppDrawerAction::DragOutApp {
                                app_id,
                                area,
                                abs: rect.pos + rect.size * 0.5,
                            });
                        }
                        DrawerItemAction::None => (),
                    }
                }
            }
        }

        // Swipe down anywhere on the drawer to close it: always over the chrome
        // (above the list), and over the list itself only when it's already
        // scrolled to the top, so a downward drag there dismisses instead of
        // fighting the list's own scroll. (Only downward motion closes, so an
        // upward drag over a top-of-list still scrolls into the grid.)
        //
        // The list's cells capture the finger first, so a plain `hits()` on the
        // drawer area never sees a touch that lands on an app icon. `capture_overload`
        // lets the drawer *co-observe* the same finger, so the swipe-to-close works
        // over the grid too — not just the bare chrome above it.
        let list_top = self.view.widget(cx, ids!(list)).area().rect(cx).pos.y;
        let list_at_top = self.view.portal_list(cx, ids!(list)).first_id() == 0;
        match event.hits_with_options(
            cx,
            self.view.area(),
            HitOptions::new().with_capture_overload(true),
        ) {
            Hit::FingerDown(fe) => {
                // Scrolling a split-pick's docked pane (drawn over the
                // drawer) must not double as a swipe-to-close.
                let blocked = scope.data.get::<AppState>().is_some_and(|s| {
                    s.split_block_rect.size.x > 0.0 && s.split_block_rect.contains(fe.abs)
                });
                if !blocked && (fe.abs.y < list_top || list_at_top) {
                    self.drag_close = Some(fe.abs.y);
                }
            }
            Hit::FingerMove(fe) => {
                if let Some(start_y) = self.drag_close {
                    let height = self.last_rect.size.y.max(1.0);
                    let dragged = (fe.abs.y - start_y).max(0.0) / height;
                    self.progress = (1.0 - dragged).clamp(0.0, 1.0);
                    self.redraw(cx);
                }
            }
            Hit::FingerUp(fe) => {
                if let Some(start_y) = self.drag_close.take() {
                    let height = self.last_rect.size.y.max(1.0);
                    let dragged = (fe.abs.y - start_y).max(0.0) / height;
                    if dragged > 0.25 {
                        self.close(cx);
                    } else if self.progress < 0.999 {
                        // Dragged partway down but not far enough to dismiss: snap
                        // back open. `settle` (unlike `open`) keeps the current
                        // search query — a tap or nudge must not wipe what was typed.
                        self.settle(cx, true);
                    }
                }
            }
            _ => (),
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.anim == DrawerAnim::Hidden {
            return DrawStep::done();
        }
        // The drawer fills its slot; slide it in/out by offsetting its abs position.
        let rect = cx.peek_walk_turtle(walk);
        self.last_rect = rect;
        let eased = 1.0 - (1.0 - self.progress).powi(3);
        let offset_y = (1.0 - eased) * rect.size.y;
        let panel_walk = Walk {
            abs_pos: Some(rect.pos + dvec2(0.0, offset_y)),
            margin: Default::default(),
            width: Size::Fixed(rect.size.x),
            height: Size::Fixed(rect.size.y),
            metrics: Default::default(),
        };

        // Populate the grid before drawing.
        let sorted = scope
            .data
            .get::<AppState>()
            .map(|state| self.sorted_ids(state))
            .unwrap_or_default();

        // Draw the whole drawer into its own overlay: inside an overlay the glass
        // surface draws INLINE (still sampling the backdrop snapshot for its
        // refraction) rather than opening its own nested overlay, so the chrome/list
        // painted after it land ON TOP of the lens and stay crisp.
        if self.overlay_list.is_none() {
            self.overlay_list = Some(DrawList2d::new(cx));
        }
        self.overlay_list.as_mut().unwrap().begin_overlay_reuse(cx);
        // Standard PortalList hosting: step the deref view, drive the list's items.
        while let Some(item) = self.view.draw_walk(cx, scope, panel_walk).step() {
            if let Some(mut list) = item.as_portal_list().borrow_mut() {
                let rows = sorted.len().div_ceil(DRAWER_COLS).max(1);
                list.set_item_range(cx, 0, rows);
                while let Some(row_id) = list.next_visible_item(cx) {
                    let row = list.item(cx, row_id, id!(Row));
                    for col in 0 .. DRAWER_COLS {
                        let cell_id = match col {
                            0 => ids!(cell_0),
                            1 => ids!(cell_1),
                            2 => ids!(cell_2),
                            _ => ids!(cell_3),
                        };
                        let idx = row_id * DRAWER_COLS + col;
                        let app_id = sorted.get(idx).cloned();
                        if let Some(mut cell) =
                            row.widget(cx, cell_id).borrow_mut::<DrawerItem>()
                        {
                            let state = scope.data.get::<AppState>().unwrap();
                            cell.set_app(cx, state, app_id);
                        }
                    }
                    row.draw_all_unscoped(cx);
                }
            }
        }
        self.overlay_list.as_mut().unwrap().end(cx);
        DrawStep::done()
    }
}

impl AppDrawerRef {
    pub fn open(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.open(cx);
        }
    }

    pub fn close(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.close(cx);
        }
    }

    pub fn set_drag(&self, cx: &mut Cx, progress: f64) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_drag(cx, progress);
        }
    }

    pub fn settle(&self, cx: &mut Cx, open: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.settle(cx, open);
        }
    }

    pub fn is_open(&self) -> bool {
        self.borrow().is_some_and(|inner| inner.is_open())
    }
}
