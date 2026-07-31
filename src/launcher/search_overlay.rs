//! The iOS-style Spotlight search overlay: swiping down on the home screen (or
//! picking "Search" from the background menu) drops a full-screen search panel
//! over everything, with a text field and a live-filtered grid of matching apps.
//!
//! It reuses the app drawer's `DrawerItem` cells (icon tile + name, with tap and
//! long-press handling), so results behave exactly like drawer entries.

use makepad_widgets::*;

use crate::{
    app::AppState,
    launcher::app_drawer::{DrawerItem, DrawerItemAction},
    mini_apps::registry::MiniAppId,
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.SearchOverlayBase = #(SearchOverlay::register_widget(vm))

    mod.widgets.SearchOverlay = set_type_default() do mod.widgets.SearchOverlayBase{
        width: Fill
        height: Fill
        flow: Down
        show_bg: true
        draw_bg +: { color: #x05070ef2 }
        padding: Inset{
            top: (24.0 + mod.widgets.SAFE_INSET_PAD_TOP),
            bottom: (8.0 + mod.widgets.SAFE_INSET_PAD_BOTTOM),
            left: (16.0 + mod.widgets.SAFE_INSET_PAD_LEFT),
            right: (16.0 + mod.widgets.SAFE_INSET_PAD_RIGHT),
        }

        s_header := View{
            width: Fill
            height: Fit
            flow: Right
            spacing: 10
            align: Align{y: 0.5}
            padding: Inset{bottom: 12}
            s_input := LauncherTextInput{
                empty_text: "Search"
            }
            s_cancel := glass.GlassButton{
                text: "Cancel"
                height: 42
                draw_text +: {
                    text_style: theme.font_bold{font_size: 13}
                }
            }
        }

        s_empty := Label{
            visible: false
            width: Fill
            height: Fit
            align: Align{x: 0.5}
            margin: Inset{top: 40}
            text: "No apps found"
            draw_text +: {
                color: #x8fa6c8aa
                text_style: theme.font_regular{font_size: 14}
            }
        }

        s_list := PortalList{
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

/// Number of icon columns in the results grid.
const COLS: usize = 4;

/// Actions emitted by the search overlay for the app to handle.
#[derive(Clone, Debug, Default)]
pub enum SearchOverlayAction {
    /// A result was tapped; open the app, animating out from `from_rect`.
    OpenApp { app_id: MiniAppId, from_rect: Rect },
    /// The overlay was dismissed (Cancel, Escape, or swipe up).
    Dismissed,
    #[default]
    None,
}

/// The overlay's slide phase.
#[derive(Clone, Copy, Default, PartialEq)]
enum Anim {
    #[default]
    Hidden,
    Animating,
    Open,
}

#[derive(Script, ScriptHook, Widget)]
pub struct SearchOverlay {
    #[deref]
    view: View,
    #[rust]
    progress: f64,
    #[rust]
    target: f64,
    #[rust]
    anim: Anim,
    #[rust]
    next_frame: NextFrame,
    #[rust]
    last_frame_time: f64,
    /// Current search query (lowercased); empty shows every app.
    #[rust]
    query: String,
    /// Set on open; the field is focused on the next event, once it's been drawn.
    #[rust]
    want_focus: bool,
    #[rust]
    last_rect: Rect,
    /// Y where a swipe-to-dismiss started, while one is in progress.
    #[rust]
    drag_close: Option<f64>,
}

impl SearchOverlay {
    /// Apps matching the current query, alphabetically.
    fn results(&self, state: &AppState) -> Vec<MiniAppId> {
        let q = self.query.trim();
        let mut ids: Vec<MiniAppId> = state
            .registry
            .iter()
            .filter(|m| q.is_empty() || m.name.to_lowercase().contains(q))
            .map(|m| m.id.clone())
            .collect();
        ids.sort_by_key(|id| state.registry.get(id).map(|m| m.name.to_lowercase()));
        ids
    }

    fn start_anim(&mut self, cx: &mut Cx, target: f64) {
        self.target = target;
        self.anim = Anim::Animating;
        self.last_frame_time = 0.0;
        self.next_frame = cx.new_next_frame();
        self.redraw(cx);
    }

    pub fn open(&mut self, cx: &mut Cx) {
        self.query.clear();
        self.view.text_input(cx, ids!(s_input)).set_text(cx, "");
        // Focus the field so typing starts immediately, iOS-style; deferred to the
        // next event since the input hasn't been laid out (has no area) yet.
        self.want_focus = true;
        self.start_anim(cx, 1.0);
    }

    pub fn close(&mut self, cx: &mut Cx) {
        self.start_anim(cx, 0.0);
    }

    pub fn is_open(&self) -> bool {
        self.target > 0.5 && self.anim != Anim::Hidden
    }
}

impl Widget for SearchOverlay {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Slide animation.
        if let Some(ne) = self.next_frame.is_event(event) {
            if self.anim == Anim::Animating {
                let dt = if self.last_frame_time == 0.0 {
                    1.0 / 60.0
                } else {
                    (ne.time - self.last_frame_time).clamp(0.0, 0.1)
                };
                self.last_frame_time = ne.time;
                let diff = self.target - self.progress;
                if diff.abs() < 0.004 {
                    self.progress = self.target;
                    self.anim = if self.target > 0.5 { Anim::Open } else { Anim::Hidden };
                    cx.redraw_all();
                } else {
                    // Same rate both ways, slower than it was: this panel
                    // covers the whole screen, and a fast slide reads as a
                    // flash rather than a movement.
                    self.progress += diff * (1.0 - (-dt * 6.0).exp());
                    self.next_frame = cx.new_next_frame();
                }
                self.redraw(cx);
            }
        }

        if self.anim == Anim::Hidden || self.target < 0.5 {
            return;
        }

        // Focus the search field once it's been drawn (has a real area).
        if self.want_focus {
            let area = self.view.widget(cx, ids!(s_input)).area();
            if !area.is_empty() {
                cx.set_key_focus(area);
                self.want_focus = false;
            }
        }

        self.view.handle_event(cx, event, scope);

        let uid = self.widget_uid();
        if let Event::Actions(actions) = event {
            let input = self.view.text_input(cx, ids!(s_input));
            if let Some(text) = input.changed(actions) {
                self.query = text.to_lowercase();
                self.redraw(cx);
            }
            if input.escaped(actions)
                || self.view.glass_button(cx, ids!(s_cancel)).clicked(actions)
            {
                cx.widget_action(uid, SearchOverlayAction::Dismissed);
            }

            for action in actions {
                if let Some(item_action) = action.as_widget_action() {
                    match item_action.cast::<DrawerItemAction>() {
                        DrawerItemAction::Tapped { app_id, rect } => {
                            cx.widget_action(uid, SearchOverlayAction::OpenApp {
                                app_id,
                                from_rect: rect,
                            });
                        }
                        // Long-press in search just opens the app (no rearrange here).
                        DrawerItemAction::LongPressed { app_id, rect, area: _ } => {
                            cx.widget_action(uid, SearchOverlayAction::OpenApp {
                                app_id,
                                from_rect: rect,
                            });
                        }
                        DrawerItemAction::None => (),
                    }
                }
            }
        }

        // Tapping empty space anywhere below the search bar (between or beneath the
        // result icons) dismisses the search. The search field + Cancel sit above the
        // list's top, so they're excluded; a result-cell tap opens its app (which also
        // closes search). capture_overload lets us see the tap even when the results
        // list has captured it for scrolling.
        //
        // A swipe UP dismisses it too — the inverse of the swipe-down that
        // opened it — and follows the finger while it's happening, like the
        // drawer's swipe-to-close. Only started from the chrome above the
        // results, or when the list is already at its top: over a scrollable
        // grid an upward drag means "scroll", and stealing that would make the
        // results unusable.
        let list_top = self.view.widget(cx, ids!(s_list)).area().rect(cx).pos.y;
        let list_at_top = self.view.portal_list(cx, ids!(s_list)).first_id() == 0;
        match event.hits_with_options(
            cx,
            self.view.area(),
            HitOptions::new().with_capture_overload(true),
        ) {
            Hit::FingerDown(fe) => {
                if fe.abs.y < list_top || list_at_top {
                    self.drag_close = Some(fe.abs.y);
                }
            }
            Hit::FingerMove(fe) => {
                if let Some(start_y) = self.drag_close {
                    let height = self.last_rect.size.y.max(1.0);
                    let dragged = (start_y - fe.abs.y).max(0.0) / height;
                    self.progress = (1.0 - dragged).clamp(0.0, 1.0);
                    self.redraw(cx);
                }
            }
            Hit::FingerUp(fe) => {
                let dragged = self.drag_close.take().map_or(0.0, |start_y| {
                    (start_y - fe.abs.y).max(0.0) / self.last_rect.size.y.max(1.0)
                });
                if dragged > 0.25 {
                    cx.widget_action(uid, SearchOverlayAction::Dismissed);
                } else if fe.was_tap() && fe.is_over && fe.abs.y >= list_top {
                    cx.widget_action(uid, SearchOverlayAction::Dismissed);
                } else if self.progress < 0.999 {
                    // Nudged up but not far enough: snap back. `start_anim`
                    // rather than `open` so the typed query survives.
                    self.start_anim(cx, 1.0);
                }
            }
            _ => (),
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.anim == Anim::Hidden {
            return DrawStep::done();
        }
        // Slide down from the top: fully open sits at rest, closing lifts it away.
        let rect = cx.peek_walk_turtle(walk);
        self.last_rect = rect;
        let eased = 1.0 - (1.0 - self.progress).powi(3);
        let offset_y = -(1.0 - eased) * rect.size.y;
        let panel_walk = Walk {
            abs_pos: Some(rect.pos + dvec2(0.0, offset_y)),
            margin: Default::default(),
            width: Size::Fixed(rect.size.x),
            height: Size::Fixed(rect.size.y),
            metrics: Default::default(),
        };

        let results = scope
            .data
            .get::<AppState>()
            .map(|state| self.results(state))
            .unwrap_or_default();
        self.view
            .label(cx, ids!(s_empty))
            .set_visible(cx, results.is_empty());

        while let Some(item) = self.view.draw_walk(cx, scope, panel_walk).step() {
            if let Some(mut list) = item.as_portal_list().borrow_mut() {
                let rows = results.len().div_ceil(COLS).max(1);
                list.set_item_range(cx, 0, rows);
                while let Some(row_id) = list.next_visible_item(cx) {
                    let row = list.item(cx, row_id, id!(Row));
                    for col in 0 .. COLS {
                        let cell_id = match col {
                            0 => ids!(cell_0),
                            1 => ids!(cell_1),
                            2 => ids!(cell_2),
                            _ => ids!(cell_3),
                        };
                        let idx = row_id * COLS + col;
                        let app_id = results.get(idx).cloned();
                        if let Some(mut cell) = row.widget(cx, cell_id).borrow_mut::<DrawerItem>() {
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

impl SearchOverlayRef {
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
