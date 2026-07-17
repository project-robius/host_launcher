//! The bottom dock: a persistent glass bar of favorite app icons, shown on every
//! home page (like the iOS dock / Android favorites tray).
//!
//! It mirrors the home pager's "draw icon child widgets at absolute positions"
//! approach, but with a single fixed row and no paging, editing, or drag: the dock
//! is deliberately a calm, always-there launch strip. Tapping an icon opens the app
//! with the same zoom-from-rect animation as the grid; right-click or long-press
//! opens the app's shortcut menu.

use std::collections::HashMap;

use makepad_widgets::*;

use crate::{
    app::AppState,
    launcher::{home_pager::tile_tint_color, notif_badge::NotifBadgeWidgetRefExt},
    mini_apps::registry::MiniAppId,
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.LauncherDockBase = #(LauncherDock::register_widget(vm))

    mod.widgets.LauncherDock = set_type_default() do mod.widgets.LauncherDockBase{
        width: Fill
        height: 102

        // The full-width liquid-glass rounded rectangle the icons sit on: a real
        // lensing surface (same kit as the menus), refracting the backdrop.
        DockBar := glass.LensSurface{
            width: Fill
            height: Fill
            draw_bg +: {
                corner_radius: 22.0
                tint_color: #xf8fbff
                tint_alpha: 0.03
                border_alpha: 0.7
            }
        }

        // A single icon slot: a tinted glass tile with an emoji glyph, no label
        // (dock icons are label-less, iOS-style).
        DockIcon := View{
            width: Fill
            height: Fill
            flow: Overlay
            align: Align{x: 0.5, y: 0.5}
            // Don't clip (cut off) the notification badge overhanging the tile.
            clip_x: false, clip_y: false
            tile := LauncherIconTile{
                flow: Overlay
                clip_x: false, clip_y: false
                View{
                    width: Fill
                    height: Fill
                    align: Align{x: 0.5, y: 0.5}
                    glyph := LauncherIconGlyph{}
                }
                // Notification count on the top-right corner, iOS-style.
                View{
                    width: Fill
                    height: Fill
                    align: Align{x: 1.0, y: 0.0}
                    clip_x: false, clip_y: false
                    notif := NotifBadge{
                        margin: Inset{top: -5, right: -7}
                    }
                }
            }
        }
    }
}

/// Actions emitted by the dock for the app to handle.
#[derive(Clone, Debug, Default)]
pub enum DockAction {
    /// A dock icon was tapped; open the app, animating out from `from_rect`.
    OpenApp { app_id: MiniAppId, from_rect: Rect },
    /// A dock icon was long-pressed or right-clicked; show its shortcut menu.
    ShowContextMenu { app_id: MiniAppId, anchor: Rect },
    #[default]
    None,
}

/// Finger movement below this (in points) still counts as a tap.
const TAP_SLOP: f64 = 8.0;
/// How long a press must be held (secs) to count as a long press.
const LONG_PRESS_SECS: f64 = 0.5;
/// The size of each dock icon tile.
const ICON_SIZE: f64 = 56.0;

#[derive(Script, WidgetRef, WidgetSet, WidgetRegister)]
pub struct LauncherDock {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,
    #[rust]
    area: Area,

    #[rust]
    templates: HashMap<LiveId, ScriptObjectRef>,
    /// Instantiated icon widgets, one per dock favorite.
    #[rust]
    icons: HashMap<MiniAppId, WidgetRef>,
    /// On-screen rect of each dock icon this frame, for hit-testing.
    #[rust]
    rects: Vec<(MiniAppId, Rect)>,

    /// The instantiated liquid-glass bar drawn behind the icons.
    #[rust]
    bar: Option<WidgetRef>,

    /// The app currently being pressed, and where the press started.
    #[rust]
    pressed: Option<(MiniAppId, Vec2d)>,
    #[rust]
    long_press_timer: Timer,
}

impl ScriptHook for LauncherDock {
    fn on_before_apply(
        &mut self,
        _vm: &mut ScriptVm,
        apply: &Apply,
        _scope: &mut Scope,
        _value: ScriptValue,
    ) {
        if apply.is_reload() {
            self.templates.clear();
        }
    }

    fn on_after_apply(
        &mut self,
        vm: &mut ScriptVm,
        apply: &Apply,
        _scope: &mut Scope,
        value: ScriptValue,
    ) {
        if !apply.is_eval() {
            if let Some(obj) = value.as_object() {
                vm.vec_with(obj, |vm, vec| {
                    for kv in vec {
                        if let Some(id) = kv.key.as_id() {
                            if let Some(template_obj) = kv.value.as_object() {
                                self.templates
                                    .insert(id, vm.bx.heap.new_object_ref(template_obj));
                            }
                        }
                    }
                });
            }
        }
        vm.cx_mut().widget_tree_mark_dirty(self.uid);
    }
}

impl WidgetNode for LauncherDock {
    fn widget_uid(&self) -> WidgetUid {
        self.uid
    }

    fn walk(&mut self, _cx: &mut Cx) -> Walk {
        self.walk
    }

    fn area(&self) -> Area {
        self.area
    }

    fn children(&self, visit: &mut dyn FnMut(LiveId, WidgetRef)) {
        if let Some(bar) = &self.bar {
            visit(live_id!(dock_bar), bar.clone());
        }
        for (id, child) in self.icons.iter() {
            visit(LiveId::from_str(id), child.clone());
        }
    }

    fn redraw(&mut self, cx: &mut Cx) {
        self.area.redraw(cx);
    }
}

impl LauncherDock {
    /// Ensures a child icon widget exists for the given app, tinted and glyphed.
    fn ensure_icon(&mut self, cx: &mut Cx, state: &AppState, app_id: &MiniAppId) -> Option<WidgetRef> {
        if let Some(icon) = self.icons.get(app_id) {
            return Some(icon.clone());
        }
        let template = self.templates.get(&live_id!(DockIcon))?;
        let template_value: ScriptValue = template.as_object().into();
        let icon = cx.with_vm(|vm| WidgetRef::script_from_value(vm, template_value));
        let manifest = state.registry.get(app_id)?;
        icon.label(cx, ids!(glyph)).set_text(cx, &manifest.icon);
        let tint = tile_tint_color(manifest.tint);
        let mut tile = icon.widget(cx, ids!(tile));
        script_apply_eval!(cx, tile, {
            draw_bg +: { color: #(tint) }
        });
        cx.widget_tree_insert_child_deep(self.uid, LiveId::from_str(app_id), icon.clone());
        icon.notif_badge(cx, ids!(notif))
            .set_count(state.notifications.get(app_id).copied().unwrap_or(0));
        self.icons.insert(app_id.clone(), icon.clone());
        Some(icon)
    }

    /// Ensures the glass bar child exists (instantiated from its template).
    fn ensure_bar(&mut self, cx: &mut Cx) -> Option<WidgetRef> {
        if let Some(bar) = &self.bar {
            return Some(bar.clone());
        }
        let template = self.templates.get(&live_id!(DockBar))?;
        let template_value: ScriptValue = template.as_object().into();
        let bar = cx.with_vm(|vm| WidgetRef::script_from_value(vm, template_value));
        cx.widget_tree_insert_child_deep(self.uid, live_id!(dock_bar), bar.clone());
        self.bar = Some(bar.clone());
        Some(bar)
    }

    /// Drops child widgets for apps no longer in the dock.
    fn prune_children(&mut self, cx: &mut Cx, dock: &[MiniAppId]) {
        let before = self.icons.len();
        self.icons.retain(|id, _| dock.contains(id));
        if before != self.icons.len() {
            cx.widget_tree_mark_dirty(self.uid);
        }
    }

    /// The absolute on-screen rect of a dock icon, rebasing its draw-relative rect
    /// onto the resolved dock area.
    fn icon_abs_rect(&self, cx: &mut Cx, rel: Rect) -> Rect {
        Rect {
            pos: self.area.rect(cx).pos + rel.pos,
            size: rel.size,
        }
    }

    /// The absolute rect of the given dock icon (falls back to a unit rect).
    fn icon_anchor(&self, cx: &mut Cx, app_id: &MiniAppId) -> Rect {
        self.rects
            .iter()
            .find(|(id, _)| id == app_id)
            .map(|(_, r)| self.icon_abs_rect(cx, *r))
            .unwrap_or(Rect {
                pos: self.area.rect(cx).pos,
                size: dvec2(1.0, 1.0),
            })
    }

    /// The app icon under the given absolute point, if any.
    fn icon_at(&self, cx: &mut Cx, abs: Vec2d) -> Option<MiniAppId> {
        let base = self.area.rect(cx).pos;
        self.rects
            .iter()
            .find(|(_, r)| {
                Rect { pos: base + r.pos, size: r.size }.contains(abs)
            })
            .map(|(id, _)| id.clone())
    }
}

impl Widget for LauncherDock {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Forward to child widgets (glass bar needs events for its lens updates).
        let children: Vec<WidgetRef> = self
            .bar
            .iter()
            .cloned()
            .chain(self.icons.values().cloned())
            .collect();
        for w in children {
            w.handle_event(cx, event, scope);
        }

        if self.long_press_timer.is_event(event).is_some() {
            if let Some((app_id, _)) = self.pressed.clone() {
                self.pressed = None;
                let anchor = self.icon_anchor(cx, &app_id);
                cx.widget_action(self.uid, DockAction::ShowContextMenu { app_id, anchor });
            }
        }

        let hit = event.hits_with_options(cx, self.area, HitOptions::new());
        match hit {
            Hit::FingerDown(fe) => {
                // Right-click opens the shortcut menu directly.
                if fe.device.mouse_button().is_some_and(|b| b.is_secondary()) {
                    if let Some(app_id) = self.icon_at(cx, fe.abs) {
                        let anchor = self.icon_anchor(cx, &app_id);
                        cx.widget_action(self.uid, DockAction::ShowContextMenu { app_id, anchor });
                    }
                    return;
                }
                if !fe.device.is_primary_hit() {
                    return;
                }
                if let Some(app_id) = self.icon_at(cx, fe.abs) {
                    self.pressed = Some((app_id, fe.abs));
                    self.long_press_timer = cx.start_timeout(LONG_PRESS_SECS);
                }
            }
            Hit::FingerMove(fe) => {
                // Moving off the pressed icon cancels the press (no dock reordering).
                if let Some((_, start)) = &self.pressed {
                    if (fe.abs - *start).length() > TAP_SLOP {
                        cx.stop_timer(self.long_press_timer);
                        self.pressed = None;
                    }
                }
            }
            Hit::FingerUp(fe) => {
                cx.stop_timer(self.long_press_timer);
                if let Some((app_id, _)) = self.pressed.take() {
                    if self.icon_at(cx, fe.abs).as_ref() == Some(&app_id) {
                        let rel = self
                            .rects
                            .iter()
                            .find(|(id, _)| id == &app_id)
                            .map(|(_, r)| *r);
                        let from_rect = match rel {
                            Some(r) => self.icon_abs_rect(cx, r),
                            None => Rect { pos: fe.abs, size: dvec2(ICON_SIZE, ICON_SIZE) },
                        };
                        cx.widget_action(self.uid, DockAction::OpenApp { app_id, from_rect });
                    }
                }
            }
            _ => (),
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, Layout::flow_overlay());
        // The turtle rect here is the provisional draw origin (a Fill sibling above
        // the dock defers the real position). Children drawn with abs_pos relative
        // to it still land correctly, but stored hit-rects must be kept relative to
        // this origin and rebased onto `self.area` (the resolved rect) at hit time.
        let rect = cx.turtle().rect();

        let dock: Vec<MiniAppId> = match scope.data.get::<AppState>() {
            Some(state) => state
                .layout
                .dock
                .iter()
                .filter(|id| state.registry.contains(id))
                .cloned()
                .collect(),
            None => {
                cx.end_turtle_with_area(&mut self.area);
                return DrawStep::done();
            }
        };
        self.prune_children(cx, &dock);

        self.rects.clear();
        if !dock.is_empty() {
            let n = dock.len() as f64;
            // The glass pill spans the full dock width; icons spread across it
            // with equal gaps.
            let bar_w = rect.size.x;
            let bar_h = ICON_SIZE + 20.0;
            let bar_x = rect.pos.x + (rect.size.x - bar_w) * 0.5;
            let bar_y = rect.pos.y + (rect.size.y - bar_h) * 0.5;
            if let Some(bar) = self.ensure_bar(cx) {
                bar.draw_walk_all(
                    cx,
                    scope,
                    Walk {
                        abs_pos: Some(dvec2(bar_x, bar_y)),
                        margin: Default::default(),
                        width: Size::Fixed(bar_w),
                        height: Size::Fixed(bar_h),
                        metrics: Default::default(),
                    },
                );
            }

            // Keep the outermost icons clear of the pill's lensing rim, where the
            // refraction would visibly warp them.
            let edge_inset = 30.0_f64.min(bar_w * 0.08);
            let avail = bar_w - 2.0 * edge_inset;
            let gap = if n > 1.0 {
                (avail - n * ICON_SIZE) / (n - 1.0)
            } else {
                0.0
            };
            // Bias the icons slightly downward within the bar so the notification
            // badges overhanging their top-right corners keep clear headroom.
            let icon_y = bar_y + (bar_h - ICON_SIZE) * 0.5 + 2.0;
            for (i, app_id) in dock.iter().enumerate() {
                let icon_x = bar_x + edge_inset + i as f64 * (ICON_SIZE + gap);
                let icon_rect = Rect {
                    pos: dvec2(icon_x, icon_y),
                    size: dvec2(ICON_SIZE, ICON_SIZE),
                };
                // Store the icon rect relative to the draw origin; it's rebased onto
                // the resolved `self.area` when hit-testing (see `icon_at`).
                self.rects.push((
                    app_id.clone(),
                    Rect {
                        pos: icon_rect.pos - rect.pos,
                        size: icon_rect.size,
                    },
                ));
                let icon = {
                    let state = scope.data.get::<AppState>().unwrap();
                    self.ensure_icon(cx, state, app_id)
                };
                if let Some(icon) = icon {
                    icon.draw_walk_all(
                        cx,
                        scope,
                        Walk {
                            abs_pos: Some(icon_rect.pos),
                            margin: Default::default(),
                            width: Size::Fixed(ICON_SIZE),
                            height: Size::Fixed(ICON_SIZE),
                            metrics: Default::default(),
                        },
                    );
                }
            }
        }

        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }
}
