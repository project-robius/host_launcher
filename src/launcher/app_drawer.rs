//! The Android-style app drawer: swiping up on the home screen slides a
//! full-height panel over it, listing every installed app in a grid, with a
//! sort button that toggles between alphabetical and most-recently-used order.

use makepad_widgets::*;

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
        flow: Down
        show_bg: true
        draw_bg +: {
            color: #x0a1120ee
            border_color: #xffffff18
            border_size: 1.0
            border_radius: 22.0
        }
        padding: Inset{top: 10, left: 16, right: 16, bottom: 8}

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
            sort_button := ButtonFlat{
                text: "A–Z"
                padding: Inset{top: 6, bottom: 6, left: 12, right: 12}
                draw_text +: {
                    color: #x9dccff
                    text_style: theme.font_bold{font_size: 11}
                }
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
    /// Whether `set_app` has run at least once (so the first empty cell still hides).
    #[rust]
    applied: bool,
    #[rust]
    press_timer: Timer,
    #[rust]
    long_press_fired: bool,
}

#[derive(Clone, Debug, Default)]
enum DrawerItemAction {
    Tapped { app_id: MiniAppId, rect: Rect },
    LongPressed { app_id: MiniAppId, rect: Rect },
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
                },
            );
        }

        match event.hits(cx, self.view.area()) {
            Hit::FingerDown(fe) if fe.device.is_primary_hit() => {
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
                // A native (mobile) long press is reported on the up event.
                if fe.has_long_press_occurred && !self.long_press_fired {
                    self.long_press_fired = true;
                    cx.widget_action(
                        uid,
                        DrawerItemAction::LongPressed {
                            app_id,
                            rect: self.view.area().rect(cx),
                        },
                    );
                } else if fe.was_tap() && fe.is_over && !self.long_press_fired {
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
    fn set_app(&mut self, cx: &mut Cx, state: &AppState, app_id: Option<MiniAppId>) {
        // A recycled PortalList item is redrawn every frame; only touch the widget
        // tree when the cell's app actually changes, so we don't re-eval the tint
        // shader (and re-dirty the tree) on every frame.
        if self.applied && self.applied_app == app_id {
            return;
        }
        self.applied = true;
        self.applied_app = app_id.clone();
        self.app_id = app_id.clone();
        match app_id.as_ref().and_then(|id| state.registry.get(id)) {
            Some(manifest) => {
                self.view.set_visible(cx, true);
                self.view.label(cx, ids!(d_glyph)).set_text(cx, &manifest.icon);
                self.view.label(cx, ids!(d_name)).set_text(cx, &manifest.name);
                let tint = crate::launcher::home_pager::tile_tint_color(manifest.tint);
                let mut tile = self.view.widget(cx, ids!(d_tile));
                script_apply_eval!(cx, tile, {
                    draw_bg +: { color: #(tint) }
                });
            }
            None => {
                self.view.set_visible(cx, false);
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
    /// Finger-drag on the grab/header area to close.
    #[rust]
    drag_close: Option<f64>,
    #[rust]
    last_rect: Rect,
}

impl AppDrawer {
    /// The app ids in current drawer order.
    fn sorted_ids(&self, state: &AppState) -> Vec<MiniAppId> {
        let mut ids: Vec<MiniAppId> = state.registry.iter().map(|m| m.id.clone()).collect();
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
        self.start_anim(cx, 1.0);
    }

    pub fn close(&mut self, cx: &mut Cx) {
        self.start_anim(cx, 0.0);
    }

    pub fn is_open(&self) -> bool {
        self.target > 0.5 && self.anim != DrawerAnim::Hidden
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
                    self.progress += diff * (1.0 - (-dt * 16.0).exp());
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

        self.view.handle_event(cx, event, scope);

        // Sort toggle.
        if let Event::Actions(actions) = event {
            if self.view.button(cx, ids!(sort_button)).clicked(actions) {
                self.sort_recent = !self.sort_recent;
                let label = if self.sort_recent { "Recent" } else { "A–Z" };
                self.view.button(cx, ids!(sort_button)).set_text(cx, label);
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
                        DrawerItemAction::LongPressed { app_id, rect } => {
                            cx.widget_action(uid, AppDrawerAction::ShowContextMenu {
                                app_id,
                                anchor: rect,
                            });
                        }
                        DrawerItemAction::None => (),
                    }
                }
            }
        }

        // Drag down on the grab bar / header to close.
        let grab_area = self.view.widget(cx, ids!(grab)).area();
        match event.hits(cx, grab_area) {
            Hit::FingerDown(fe) => {
                self.drag_close = Some(fe.abs.y);
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
                    } else {
                        self.open(cx);
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

    pub fn is_open(&self) -> bool {
        self.borrow().is_some_and(|inner| inner.is_open())
    }
}
